//! Staff circulation exceptions: lost, damaged, claimed-returned.
//!
//! Item status is the state machine. The loan is archived or left open
//! according to the action; billing reuses the fines table with a charge type.

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    error::{AppError, AppResult},
    models::{
        circulation::{CirculationAction, ClaimsResolveOutcome},
        dto::circulation::{
            CirculationExceptionOutcome, CirculationExceptionResponse, ClaimsReturnedQueueItem,
            MarkClaimedReturnedRequest, MarkDamagedRequest, MarkLostRequest,
            ResolveClaimsReturnedRequest,
        },
        fine::FineChargeType,
    },
    repository::{circulation::CirculationApplyParams, Repository},
    services::{
        audit::{self, AuditLogMeta, AuditService},
        event_bus::EventBus,
    },
};

#[derive(Clone)]
pub struct CirculationService {
    repository: Arc<Repository>,
    audit: AuditService,
    events: EventBus,
}

impl CirculationService {
    pub fn new(repository: Arc<Repository>, audit: AuditService, events: EventBus) -> Self {
        Self {
            repository,
            audit,
            events,
        }
    }

    pub async fn mark_lost(
        &self,
        loan_id: i64,
        req: MarkLostRequest,
        actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<CirculationExceptionResponse> {
        let loan = self.repository.loans_get_by_id(loan_id).await?;
        let amount = if req.bill {
            Some(
                self.resolve_replacement_amount(loan.item_id, req.amount, req.use_item_price)
                    .await?,
            )
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
            client_ip,
            &response,
            serde_json::json!({
                "loanClosed": response.loan_closed,
                "billed": response.charge.is_some(),
                "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
                "chargeType": response.charge.as_ref().map(|c| c.charge_type),
                "amount": response.charge.as_ref().map(|c| c.amount),
            }),
        );
        Ok(response)
    }

    pub async fn mark_damaged(
        &self,
        loan_id: i64,
        req: MarkDamagedRequest,
        actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<CirculationExceptionResponse> {
        let should_bill = req.bill || req.amount.is_some();
        let amount = if should_bill {
            let amount = req.amount.ok_or_else(|| {
                AppError::Validation("Damage fee amount is required when billing".to_string())
            })?;
            if amount <= Decimal::ZERO {
                return Err(AppError::Validation(
                    "Damage fee amount must be positive".to_string(),
                ));
            }
            Some(amount)
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
            client_ip,
            &response,
            serde_json::json!({
                "disposition": req.disposition,
                "loanClosed": response.loan_closed,
                "billed": response.charge.is_some(),
                "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
                "chargeType": response.charge.as_ref().map(|c| c.charge_type),
                "amount": response.charge.as_ref().map(|c| c.amount),
            }),
        );
        Ok(response)
    }

    pub async fn mark_claimed_returned(
        &self,
        loan_id: i64,
        req: MarkClaimedReturnedRequest,
        actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<CirculationExceptionResponse> {
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
            return Err(AppError::Internal(
                "claimed-returned must not close the loan".to_string(),
            ));
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

    pub async fn resolve_claims_returned(
        &self,
        loan_id: i64,
        req: ResolveClaimsReturnedRequest,
        actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<CirculationExceptionResponse> {
        if !req.inventory_checked {
            return Err(AppError::BusinessRule(
                "Claims-returned cannot be cleared without an inventory check (set inventoryChecked=true)"
                    .to_string(),
            ));
        }

        let (action, outcome, charge_type) = match req.outcome {
            ClaimsResolveOutcome::Found => (
                CirculationAction::ResolveFound,
                CirculationExceptionOutcome::ClaimsResolvedFound,
                None,
            ),
            ClaimsResolveOutcome::NotFound => (
                CirculationAction::ResolveNotFound,
                CirculationExceptionOutcome::ClaimsResolvedNotFound,
                Some(FineChargeType::Replacement),
            ),
        };

        let amount = if req.outcome == ClaimsResolveOutcome::NotFound && req.bill {
            let loan = self.repository.loans_get_by_id(loan_id).await?;
            Some(
                self.resolve_replacement_amount(loan.item_id, req.amount, req.use_item_price)
                    .await?,
            )
        } else {
            None
        };

        let applied = self
            .repository
            .circulation_apply(CirculationApplyParams {
                loan_id,
                action,
                disposition: None,
                bill_amount: amount,
                charge_type: amount.and(charge_type),
                notes: req.notes.as_deref(),
            })
            .await?;

        if let Some(ref hold) = applied.readied_hold {
            self.events.hold_ready(hold.id, hold.user_id, hold.item_id);
        }
        if applied.loan_closed {
            self.events
                .loan_returned(applied.loan_id, applied.user_id, applied.item_id);
        }

        let response = to_response(outcome, &applied);
        self.audit_exception(
            audit::event::ITEM_CLAIMS_RETURNED_RESOLVED,
            actor,
            client_ip,
            &response,
            serde_json::json!({
                "resolveOutcome": req.outcome,
                "inventoryChecked": true,
                "loanClosed": response.loan_closed,
                "itemStatus": response.item_status,
                "billed": response.charge.is_some(),
                "chargeId": response.charge.as_ref().map(|c| c.id.to_string()),
            }),
        );
        Ok(response)
    }

    pub async fn list_claims_returned(
        &self,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ClaimsReturnedQueueItem>, i64)> {
        self.repository
            .circulation_list_claims_returned(page, per_page)
            .await
    }

    async fn resolve_replacement_amount(
        &self,
        item_id: i64,
        explicit: Option<Decimal>,
        use_item_price: bool,
    ) -> AppResult<Decimal> {
        if let Some(amount) = explicit {
            if amount <= Decimal::ZERO {
                return Err(AppError::Validation(
                    "Replacement amount must be positive".to_string(),
                ));
            }
            return Ok(amount);
        }
        if use_item_price {
            if let Some(price) = self.repository.circulation_item_price(item_id).await? {
                return Ok(price);
            }
        }
        Err(AppError::Validation(
            "Replacement bill requires an amount or a positive item price".to_string(),
        ))
    }

    fn audit_exception(
        &self,
        event: &'static str,
        actor: Option<i64>,
        client_ip: Option<String>,
        response: &CirculationExceptionResponse,
        extra: serde_json::Value,
    ) {
        let mut payload = extra;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "loanId".into(),
                serde_json::json!(response.loan_id.to_string()),
            );
            obj.insert(
                "itemId".into(),
                serde_json::json!(response.item_id.to_string()),
            );
            obj.insert(
                "userId".into(),
                serde_json::json!(response.user_id.to_string()),
            );
            obj.insert(
                "itemStatus".into(),
                serde_json::to_value(response.item_status).unwrap_or(serde_json::Value::Null),
            );
            obj.insert("borrowable".into(), serde_json::json!(response.borrowable));
        }
        self.audit.log(
            event,
            actor,
            Some("item"),
            Some(response.item_id),
            client_ip,
            Some(payload),
            AuditLogMeta::success(),
        );
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
                    "amount": charge.amount,
                    "trigger": event,
                })),
                AuditLogMeta::success(),
            );
        }
    }
}

fn to_response(
    outcome: CirculationExceptionOutcome,
    applied: &crate::repository::circulation::CirculationApplyResult,
) -> CirculationExceptionResponse {
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
    use crate::models::circulation::CirculationStatus;

    #[test]
    fn lost_and_damaged_are_non_borrowable() {
        assert!(CirculationStatus::Lost.blocks_circulation());
        assert!(CirculationStatus::Damaged.blocks_circulation());
        assert!(CirculationStatus::ClaimedReturned.blocks_circulation());
        assert!(!CirculationStatus::Available.blocks_circulation());
    }
}
