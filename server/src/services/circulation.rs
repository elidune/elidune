//! Staff circulation exceptions: lost, damaged, claimed-returned.
//!
//! Item status is the state machine. The loan is archived or left open
//! according to the action. Billing is a lost/damaged *transition effect*
//! (never encoded as status). Claimed-returned is investigation only and
//! cannot bill — the patron may have returned the copy.

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    error::{AppError, AppResult},
    models::{
        circulation::{CirculationAction, ClaimsResolveOutcome},
        dto::circulation::{
            CirculationExceptionOutcome, CirculationExceptionResponse, ClaimsReturnedQueueItem, LostFineAction, LostFoundDisposition, LostFoundResolution, MarkClaimedReturnedRequest,
            MarkDamagedRequest, MarkLostRequest, ResolveClaimsReturnedRequest, ResolveLostFoundRequest,
        },
        fine::{Cents, FineChargeType},
    },
    repository::{circulation::CirculationApplyParams, fines::LostFineSettlement, Repository},
    services::{
        audit::{self, AuditLogMeta, AuditService},
        email::EmailService,
        event_bus::EventBus,
    },
};

struct DepositLossAudit<'a> {
    deposit: Option<&'a crate::repository::deposits::DepositReplacement>,
    amount: Option<Cents>,
    kind: &'a str,
    loan_id: i64,
    item_id: i64,
    actor: Option<i64>,
    client_ip: Option<String>,
}

#[derive(Clone)]
pub struct CirculationService {
    repository: Arc<Repository>,
    audit: AuditService,
    events: EventBus,
    email: EmailService,
}

impl CirculationService {
    pub fn new(repository: Arc<Repository>, audit: AuditService, events: EventBus, email: EmailService) -> Self {
        Self { repository, audit, events, email }
    }

    pub async fn mark_lost(&self, loan_id: i64, req: MarkLostRequest, actor: Option<i64>, client_ip: Option<String>) -> AppResult<CirculationExceptionResponse> {
        let loan = self.repository.loans_get_by_id(loan_id).await?;
        let deposit = self.repository.item_deposit_replacement(loan.item_id).await?;
        let amount = if let Some(ref deposit) = deposit {
            Some(resolve_deposit_bill(deposit, req.amount)?)
        } else if req.bill {
            Some(self.resolve_bill_amount(loan.item_id, req.amount, req.use_item_price, "Replacement amount").await?)
        } else {
            None
        };
        let applied = self
            .repository
            .circulation_apply(CirculationApplyParams {
                loan_id,
                action: CirculationAction::MarkLost,
                disposition: None,
                bill_amount: amount,
                charge_type: amount.map(|_| FineChargeType::Replacement),
                notes: req.notes.as_deref(),
            })
            .await?;
        let response = to_response(CirculationExceptionOutcome::Lost, &applied);
        self.audit_exception(
            audit::event::ITEM_MARKED_LOST,
            actor,
            client_ip.clone(),
            &response,
            serde_json::json!({
                "loanClosed": response.loan_closed,
                "billed": response.charge.is_some(),
                "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
                "chargeType": response.charge.as_ref().map(|c| c.charge_type),
                "amount": response.charge.as_ref().map(|c| c.amount.to_euro_string()),
            }),
        );
        self.audit_deposit_loss(DepositLossAudit {
            deposit: deposit.as_ref(),
            amount,
            kind: "lost",
            loan_id,
            item_id: response.item_id,
            actor,
            client_ip,
        });
        Ok(response)
    }

    pub async fn mark_damaged(&self, loan_id: i64, req: MarkDamagedRequest, actor: Option<i64>, client_ip: Option<String>) -> AppResult<CirculationExceptionResponse> {
        let loan = self.repository.loans_get_by_id(loan_id).await?;
        let deposit = self.repository.item_deposit_replacement(loan.item_id).await?;
        let should_bill = req.bill || req.amount.is_some();
        let amount = if let Some(ref deposit) = deposit {
            Some(resolve_deposit_bill(deposit, req.amount)?)
        } else if should_bill {
            Some(self.resolve_bill_amount(loan.item_id, req.amount, true, "Damage fee amount").await?)
        } else {
            None
        };
        let applied = self
            .repository
            .circulation_apply(CirculationApplyParams {
                loan_id,
                action: CirculationAction::MarkDamaged,
                disposition: Some(req.disposition),
                bill_amount: amount,
                charge_type: amount.map(|_| FineChargeType::Damage),
                notes: req.notes.as_deref(),
            })
            .await?;
        let response = to_response(CirculationExceptionOutcome::Damaged, &applied);
        self.audit_exception(
            audit::event::ITEM_MARKED_DAMAGED,
            actor,
            client_ip.clone(),
            &response,
            serde_json::json!({
                "disposition": req.disposition,
                "loanClosed": response.loan_closed,
                "billed": response.charge.is_some(),
                "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
                "chargeType": response.charge.as_ref().map(|c| c.charge_type),
                "amount": response.charge.as_ref().map(|c| c.amount.to_euro_string()),
            }),
        );
        self.audit_deposit_loss(DepositLossAudit {
            deposit: deposit.as_ref(),
            amount,
            kind: "damaged",
            loan_id,
            item_id: response.item_id,
            actor,
            client_ip,
        });
        Ok(response)
    }

    pub async fn mark_claimed_returned(&self, loan_id: i64, req: MarkClaimedReturnedRequest, actor: Option<i64>, client_ip: Option<String>) -> AppResult<CirculationExceptionResponse> {
        reject_disallowed_bill(CirculationAction::ClaimReturned, req.bill)?;
        let applied = self
            .repository
            .circulation_apply(CirculationApplyParams {
                loan_id,
                action: CirculationAction::ClaimReturned,
                disposition: None,
                bill_amount: None,
                charge_type: None,
                notes: req.notes.as_deref(),
            })
            .await?;
        if applied.loan_closed {
            return Err(AppError::Internal("claimed-returned must not close the loan".to_string()));
        }
        let response = to_response(CirculationExceptionOutcome::ClaimedReturned, &applied);
        self.audit_exception(
            audit::event::ITEM_CLAIMED_RETURNED,
            actor,
            client_ip,
            &response,
            serde_json::json!({
                "loanClosed": false,
                "queue": "claimsReturned",
            }),
        );
        Ok(response)
    }

    pub async fn resolve_claims_returned(&self, loan_id: i64, req: ResolveClaimsReturnedRequest, actor: Option<i64>, client_ip: Option<String>) -> AppResult<CirculationExceptionResponse> {
        if !req.inventory_checked {
            return Err(AppError::BusinessRule(
                "Claims-returned cannot be cleared without an inventory check (set inventoryChecked=true)".to_string(),
            ));
        }

        let (action, outcome) = match req.outcome {
            ClaimsResolveOutcome::Found => (CirculationAction::ResolveFound, CirculationExceptionOutcome::ClaimsResolvedFound),
            ClaimsResolveOutcome::NotFound => (CirculationAction::ResolveNotFound, CirculationExceptionOutcome::ClaimsResolvedNotFound),
        };
        reject_disallowed_bill(action, req.bill)?;

        let mut pending = self
            .repository
            .circulation_apply_pending(CirculationApplyParams {
                loan_id,
                action,
                disposition: None,
                bill_amount: None,
                charge_type: None,
                notes: req.notes.as_deref(),
            })
            .await?;

        let response = to_response(outcome, &pending.apply);
        // Same transaction as the status change, committed before the response,
        // so a reader cannot observe the resolution without this audit row.
        self.audit
            .log_in_tx(
                &mut pending.tx,
                audit::event::ITEM_CLAIMS_RETURNED_RESOLVED,
                actor,
                Some("item"),
                Some(response.item_id),
                client_ip,
                Some(exception_audit_payload(
                    &response,
                    serde_json::json!({
                        "resolveOutcome": req.outcome,
                        "inventoryChecked": true,
                        "loanClosed": response.loan_closed,
                        "itemStatus": response.item_status,
                        "billed": response.charge.is_some(),
                        "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
                    }),
                )),
                AuditLogMeta::success(),
            )
            .await?;

        let applied = pending.apply;
        pending.tx.commit().await?;

        if let Some(ref hold) = applied.readied_hold {
            if let Some(item_id) = hold.item_id {
                self.events.hold_ready(hold.id, hold.user_id, item_id);
            }
        }
        if applied.loan_closed {
            self.events.loan_returned(applied.loan_id, applied.user_id, applied.item_id);
        }

        Ok(response)
    }

    /// Put a found lost copy back into circulation, mark it damaged, or leave it lost.
    ///
    /// `circulate` settles the one replacement amount. Elidune does not track a
    /// separate non-refundable processing fee, so a collected charge is refunded
    /// in full and an uncollected remainder is cancelled.
    /// `damaged` leaves that charge unchanged: no cancel, no refund, no amount
    /// or status write. Partial relief is an explicit waive (`fine.waived`) with
    /// a reason, never an automatic side effect of this action.
    pub async fn resolve_lost_found(&self, item_id: i64, req: ResolveLostFoundRequest, actor: Option<i64>, client_ip: Option<String>) -> AppResult<LostFoundResolution> {
        let mut pending = self.repository.circulation_resolve_lost_found(item_id, req.disposition, req.notes.as_deref()).await?;

        let unchanged_fine_id = if req.disposition == LostFoundDisposition::Damaged {
            self.repository.fines_linked_replacement_id_tx(&mut pending.tx, item_id).await?
        } else {
            None
        };

        // Audit rows are immutable. `damaged` writes none of the settlement events.
        if req.disposition != LostFoundDisposition::Damaged {
            if let Some(settlement) = pending.apply.settlement.clone() {
                self.audit_settlement_tx(&mut pending.tx, actor, client_ip.clone(), item_id, req.disposition, &settlement).await?;
            }
        }
        pending.tx.commit().await?;
        let applied = pending.apply;

        if let Some(ref hold) = applied.readied_hold {
            self.notify_resolved_hold(hold, applied.title.as_deref(), applied.barcode.as_deref(), actor, client_ip.clone()).await;
        }

        let (fine_action, fine_id, refunded_amount, cancelled_amount) = if req.disposition == LostFoundDisposition::Damaged {
            (LostFineAction::Unchanged, unchanged_fine_id, 0, 0)
        } else {
            match applied.settlement {
                Some(ref settlement) => (settlement.action, Some(settlement.fine_id), settlement.refunded_amount.get(), settlement.cancelled_amount.get()),
                None => (LostFineAction::None, None, 0, 0),
            }
        };

        let response = LostFoundResolution {
            item_id: applied.item_id,
            item_status: applied.item_status,
            borrowable: applied.borrowable,
            disposition: req.disposition,
            fine_action,
            fine_id,
            refunded_amount,
            cancelled_amount,
            hold: applied.readied_hold.clone(),
        };

        let event = if req.disposition == LostFoundDisposition::Leave {
            audit::event::ITEM_LOST_FOUND_LEFT
        } else {
            audit::event::ITEM_LOST_FOUND_RESOLVED
        };
        self.audit.log(
            event,
            actor,
            Some("item"),
            Some(item_id),
            client_ip.clone(),
            Some(serde_json::json!({
                "disposition": req.disposition,
                "itemStatus": response.item_status,
                "borrowable": response.borrowable,
                "fineAction": response.fine_action,
                "fineId": response.fine_id.map(|id| id.to_string()),
                "refundedAmount": response.refunded_amount,
                "cancelledAmount": response.cancelled_amount,
                "notes": req.notes,
            })),
            AuditLogMeta::success(),
        );
        Ok(response)
    }

    pub async fn list_claims_returned(&self, page: i64, per_page: i64) -> AppResult<(Vec<ClaimsReturnedQueueItem>, i64)> {
        self.repository.circulation_list_claims_returned(page, per_page).await
    }

    fn audit_deposit_loss(&self, audit_row: DepositLossAudit<'_>) {
        let Some(deposit) = audit_row.deposit else {
            return;
        };
        self.audit.log(
            audit::event::DEPOSIT_ITEM_LOST,
            audit_row.actor,
            Some("item"),
            Some(audit_row.item_id),
            audit_row.client_ip,
            Some(serde_json::json!({
                "lenderId": deposit.lender_id.to_string(),
                "itemId": audit_row.item_id.to_string(),
                "loanId": audit_row.loan_id.to_string(),
                "amountCents": audit_row.amount.map(|cents| cents.0).unwrap_or(0),
                "kind": audit_row.kind,
            })),
            AuditLogMeta::success(),
        );
    }

    async fn resolve_bill_amount(&self, item_id: i64, explicit: Option<Decimal>, use_item_price: bool, explicit_label: &str) -> AppResult<Cents> {
        if let Some(amount) = explicit {
            let cents = Cents::try_from_decimal(amount).map_err(|err| AppError::Validation(err.to_string()))?;
            if cents.0 == 0 {
                return Err(AppError::Validation(format!("{explicit_label} must be positive")));
            }
            return Ok(cents);
        }
        if use_item_price {
            if let Some(price) = self.repository.circulation_item_price(item_id).await? {
                return Ok(price);
            }
        }
        if let Some(rule) = self.repository.fines_default_replacement_amount().await? {
            return Ok(rule);
        }
        Err(AppError::BusinessRule(
            "Enter the amount to bill: this copy has no price and there is no default replacement rule".to_string(),
        ))
    }

    async fn notify_resolved_hold(&self, hold: &crate::models::hold::Hold, title: Option<&str>, barcode: Option<&str>, actor: Option<i64>, client_ip: Option<String>) {
        if let Some(item_id) = hold.item_id {
            self.events.hold_ready(hold.id, hold.user_id, item_id);
        }
        self.audit.log(
            audit::event::HOLD_READY,
            actor,
            Some("hold"),
            Some(hold.id),
            client_ip.clone(),
            Some(serde_json::json!({
                "user_id": hold.user_id,
                "item_id": hold.item_id,
                "expires_at": hold.expires_at,
                "trigger": "lost_found",
            })),
            AuditLogMeta::success(),
        );
        let contact = self.repository.users_hold_ready_contact(hold.user_id).await.ok().flatten();
        let zone = match self.repository.library_timezone_name().await {
            Ok(name) => crate::services::calendar::parse_timezone(&name).unwrap_or_else(|_| crate::services::calendar::default_zone()),
            Err(_) => crate::services::calendar::default_zone(),
        };
        if let Err(e) = crate::hold_email::send_hold_ready_simple(&self.email, contact, hold, title.unwrap_or("(unknown title)"), barcode, zone).await {
            tracing::warn!(
                target: "circulation",
                error = %e,
                hold_id = hold.id,
                "Failed to queue hold ready email after lost copy was returned to circulation"
            );
        }
    }

    async fn audit_settlement_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        actor: Option<i64>,
        client_ip: Option<String>,
        item_id: i64,
        disposition: LostFoundDisposition,
        settlement: &LostFineSettlement,
    ) -> AppResult<()> {
        let refunded_amount = settlement.refunded_amount.get();
        let cancelled_amount = settlement.cancelled_amount.get();
        if settlement.refunded_amount.0 > 0 {
            self.audit
                .log_in_tx(
                    tx,
                    audit::event::FINE_REFUNDED,
                    actor,
                    Some("fine"),
                    Some(settlement.fine_id),
                    client_ip.clone(),
                    Some(serde_json::json!({
                        "itemId": item_id.to_string(),
                        "userId": settlement.user_id.to_string(),
                        "refundedAmount": refunded_amount,
                        "disposition": disposition,
                    })),
                    AuditLogMeta::success(),
                )
                .await?;
        }
        if settlement.cancelled_amount.0 > 0 || settlement.refunded_amount.0 == 0 {
            self.audit
                .log_in_tx(
                    tx,
                    audit::event::FINE_CANCELLED,
                    actor,
                    Some("fine"),
                    Some(settlement.fine_id),
                    client_ip,
                    Some(serde_json::json!({
                        "itemId": item_id.to_string(),
                        "userId": settlement.user_id.to_string(),
                        "cancelledAmount": cancelled_amount,
                        "disposition": disposition,
                    })),
                    AuditLogMeta::success(),
                )
                .await?;
        }
        Ok(())
    }

    fn audit_exception(&self, event: &'static str, actor: Option<i64>, client_ip: Option<String>, response: &CirculationExceptionResponse, extra: serde_json::Value) {
        let payload = exception_audit_payload(response, extra);
        self.audit.log(event, actor, Some("item"), Some(response.item_id), client_ip, Some(payload), AuditLogMeta::success());
        if let Some(ref charge) = response.charge {
            self.audit.log(
                audit::event::FINE_CREATED,
                actor,
                Some("fine"),
                Some(charge.id),
                None,
                Some(serde_json::json!({
                    "loanId": response.loan_id.to_string(),
                    "itemId": response.item_id.to_string(),
                    "userId": response.user_id.to_string(),
                    "chargeType": charge.charge_type,
                    "amount": charge.amount.to_euro_string(),
                    "trigger": event,
                })),
                AuditLogMeta::success(),
            );
        }
    }
}

/// Lender `replacementRateCents` wins, including when staff also send an amount or `0`.
/// Otherwise a positive copy price (the lender's deposit slip) wins over the staff amount.
/// A null or zero price does not count. A missing or non-positive staff amount is then
/// `deposit_replacement_amount_required`. The library default replacement
/// rule is never used. The returned cents are written once onto the fine row.
fn resolve_deposit_bill(deposit: &crate::repository::deposits::DepositReplacement, explicit: Option<Decimal>) -> AppResult<Cents> {
    if let Some(rate) = deposit.replacement_rate_cents {
        if rate.0 > 0 {
            return Ok(rate);
        }
    }
    if let Some(price) = deposit.item_price_cents {
        if price.0 > 0 {
            return Ok(price);
        }
    }
    let cents = explicit
        .map(Cents::try_from_decimal)
        .transpose()
        .map_err(|err| AppError::Validation(err.to_string()))?
        .filter(|cents| cents.0 > 0);
    cents.ok_or_else(|| {
        AppError::coded(
            422,
            crate::error::error_code::DEPOSIT_REPLACEMENT_AMOUNT_REQUIRED,
            "Enter the lender replacement amount for this deposit copy",
        )
    })
}

fn exception_audit_payload(response: &CirculationExceptionResponse, mut extra: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = extra.as_object_mut() {
        obj.insert("loanId".into(), serde_json::json!(response.loan_id.to_string()));
        obj.insert("itemId".into(), serde_json::json!(response.item_id.to_string()));
        obj.insert("userId".into(), serde_json::json!(response.user_id.to_string()));
        obj.insert("itemStatus".into(), serde_json::to_value(response.item_status).unwrap_or(serde_json::Value::Null));
        obj.insert("borrowable".into(), serde_json::json!(response.borrowable));
    }
    extra
}

fn reject_disallowed_bill(action: CirculationAction, bill: bool) -> AppResult<()> {
    if bill && !action.allows_billing() {
        return Err(AppError::BusinessRule(
            "Billing is only allowed for lost or damaged transitions; claimed-returned is an investigation queue and cannot bill. Escalate with POST /loans/{id}/lost if a replacement charge is needed"
                .to_string(),
        ));
    }
    Ok(())
}

fn to_response(outcome: CirculationExceptionOutcome, applied: &crate::repository::circulation::CirculationApplyResult) -> CirculationExceptionResponse {
    CirculationExceptionResponse {
        outcome,
        item_status: applied.item_status,
        borrowable: applied.borrowable,
        loan_closed: applied.loan_closed,
        loan_id: applied.loan_id,
        item_id: applied.item_id,
        user_id: applied.user_id,
        charge: applied.charge.clone(),
    }
}

#[cfg(test)]
mod tests {
    use crate::models::circulation::{CirculationAction, CirculationStatus};

    #[test]
    fn settlement_refunds_only_money_already_collected() {
        use crate::models::dto::circulation::LostFineAction;
        use crate::models::fine::Cents;
        use crate::repository::fines::settle_replacement_charge;

        let unpaid = settle_replacement_charge(Cents(1500), Cents::ZERO);
        assert_eq!(unpaid.action, LostFineAction::Cancelled);
        assert_eq!(unpaid.refunded_amount.0, 0);
        assert_eq!(unpaid.cancelled_amount, Cents(1500));

        let paid = settle_replacement_charge(Cents(1500), Cents(1500));
        assert_eq!(paid.action, LostFineAction::Refunded);
        assert_eq!(paid.refunded_amount, Cents(1500));
        assert_eq!(paid.cancelled_amount.0, 0);

        let partial = settle_replacement_charge(Cents(1500), Cents(400));
        assert_eq!(partial.action, LostFineAction::Partial);
        assert_eq!(partial.refunded_amount, Cents(400));
        assert_eq!(partial.cancelled_amount, Cents(1100));
    }

    #[test]
    fn damaged_fine_action_serializes_as_unchanged() {
        use crate::models::dto::circulation::LostFineAction;

        let action = LostFineAction::Unchanged;
        assert_eq!(serde_json::to_value(action).expect("fine action json"), serde_json::json!("unchanged"));
    }

    #[test]
    fn deposit_bill_uses_rate_then_positive_price_then_staff_amount() {
        use super::resolve_deposit_bill;
        use crate::models::fine::Cents;
        use crate::repository::deposits::DepositReplacement;
        use rust_decimal::Decimal;

        let rated = DepositReplacement {
            lender_id: 1,
            replacement_rate_cents: Some(Cents(1500)),
            item_price_cents: Some(Cents(2000)),
        };
        assert_eq!(resolve_deposit_bill(&rated, Some(Decimal::new(850, 2))).unwrap(), Cents(1500));
        assert_eq!(resolve_deposit_bill(&rated, Some(Decimal::ZERO)).unwrap(), Cents(1500));

        let priced = DepositReplacement {
            lender_id: 1,
            replacement_rate_cents: None,
            item_price_cents: Some(Cents(2000)),
        };
        assert_eq!(resolve_deposit_bill(&priced, Some(Decimal::new(850, 2))).unwrap(), Cents(2000));
        assert_eq!(resolve_deposit_bill(&priced, None).unwrap(), Cents(2000));

        let free = DepositReplacement {
            lender_id: 1,
            replacement_rate_cents: None,
            item_price_cents: Some(Cents(0)),
        };
        assert!(resolve_deposit_bill(&free, None).is_err());
        assert!(resolve_deposit_bill(&free, Some(Decimal::ZERO)).is_err());
        assert_eq!(resolve_deposit_bill(&free, Some(Decimal::new(850, 2))).unwrap(), Cents(850));

        let unknown = DepositReplacement {
            lender_id: 1,
            replacement_rate_cents: None,
            item_price_cents: None,
        };
        assert!(resolve_deposit_bill(&unknown, None).is_err());
        assert_eq!(resolve_deposit_bill(&unknown, Some(Decimal::new(850, 2))).unwrap(), Cents(850));
    }

    #[test]
    fn lost_and_damaged_are_non_borrowable() {
        assert!(CirculationStatus::Lost.blocks_circulation());
        assert!(CirculationStatus::Damaged.blocks_circulation());
        assert!(CirculationStatus::ClaimedReturned.blocks_circulation());
        assert!(CirculationStatus::ToSearch.blocks_circulation());
        assert_eq!(CirculationStatus::ToSearch.transition(CirculationAction::MarkLost).unwrap(), CirculationStatus::Lost);
        assert!(!CirculationStatus::Available.blocks_circulation());
    }
}
