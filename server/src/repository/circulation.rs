//! Item circulation-exception persistence (lost / damaged / claimed-returned).

use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::Row;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        circulation::{CirculationAction, CirculationStatus, DamageDisposition},
        dto::circulation::{ClaimsReturnedQueueItem, LostFoundDisposition},
        fine::{Fine, FineChargeType},
        hold::Hold,
        loan::{Loan, LoanClosureReason},
    },
    repository::fines::LostFineSettlement,
};

/// Inputs for a single atomic exception apply.
pub struct CirculationApplyParams<'a> {
    pub loan_id: i64,
    pub action: CirculationAction,
    pub disposition: Option<DamageDisposition>,
    pub bill_amount: Option<Decimal>,
    pub charge_type: Option<FineChargeType>,
    pub notes: Option<&'a str>,
}

/// Result of [`Repository::circulation_apply`].
pub struct CirculationApplyResult {
    pub loan_id: i64,
    pub item_id: i64,
    pub user_id: i64,
    pub item_status: CirculationStatus,
    pub borrowable: bool,
    pub loan_closed: bool,
    pub charge: Option<Fine>,
    pub readied_hold: Option<Hold>,
}

/// Uncommitted circulation exception.
///
/// Insert audit rows on [`Self::tx`], then commit. Dropping this value rolls
/// the apply back.
pub struct CirculationApplyPending {
    pub tx: sqlx::Transaction<'static, sqlx::Postgres>,
    pub apply: CirculationApplyResult,
}

impl Repository {
    /// Apply an item-status transition and optional loan close + charge in one transaction.
    pub async fn circulation_apply(
        &self,
        params: CirculationApplyParams<'_>,
    ) -> AppResult<CirculationApplyResult> {
        let pending = self.circulation_apply_pending(params).await?;
        pending.tx.commit().await?;
        Ok(pending.apply)
    }

    /// Same work as [`Self::circulation_apply`], with the transaction left open.
    ///
    /// Callers that must record an audit row atomically with the status change
    /// insert it before commit. Dropping [`CirculationApplyPending`] rolls the
    /// apply back.
    pub async fn circulation_apply_pending(
        &self,
        params: CirculationApplyParams<'_>,
    ) -> AppResult<CirculationApplyPending> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;

        let loan = sqlx::query_as::<_, Loan>(
            "SELECT * FROM loans WHERE id = $1 AND returned_at IS NULL FOR UPDATE",
        )
        .bind(params.loan_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Active loan {} not found", params.loan_id)))?;

        let item_row = sqlx::query(
            r#"
            SELECT id, circulation_status, borrowable, price, internal_note
            FROM items
            WHERE id = $1 AND archived_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(loan.item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {} not found", loan.item_id)))?;

        let current = CirculationStatus::from_db(item_row.get("circulation_status"));
        let next = current
            .transition(params.action)
            .map_err(|msg| AppError::BusinessRule(msg.to_string()))?;

        let close_loan = params.action.closes_loan(params.disposition);
        let notify_holds = params.action.notify_holds();
        let borrowable = next.borrowable_after();

        let item_notes = merge_notes(item_row.get("internal_note"), params.notes);

        sqlx::query(
            r#"
            UPDATE items SET
                circulation_status = $2,
                borrowable = $3,
                internal_note = $4,
                updated_at = $5
            WHERE id = $1
            "#,
        )
        .bind(loan.item_id)
        .bind(next.as_db())
        .bind(borrowable)
        .bind(&item_notes)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        if let Some(note) = params.notes {
            if !note.trim().is_empty() {
                sqlx::query("UPDATE loans SET notes = $2 WHERE id = $1")
                    .bind(loan.id)
                    .bind(merge_notes(loan.notes.clone(), Some(note)))
                    .execute(&mut *tx)
                    .await?;
            }
        }

        let charge =
            if let (Some(amount), Some(charge_type)) = (params.bill_amount, params.charge_type) {
                if amount <= Decimal::ZERO {
                    return Err(AppError::Validation(
                        "Charge amount must be positive".to_string(),
                    ));
                }
                Some(
                    self.fines_create_charge_tx(
                        &mut tx,
                        loan.id,
                        loan.user_id,
                        loan.item_id,
                        amount,
                        charge_type,
                        params.notes,
                    )
                    .await?,
                )
            } else {
                None
            };

        let mut readied_hold = None;
        if close_loan {
            let closure_reason = match params.action {
                CirculationAction::MarkLost | CirculationAction::ResolveNotFound => {
                    LoanClosureReason::Lost
                }
                _ => LoanClosureReason::Returned,
            };
            self.loans_archive_and_delete_tx(&mut tx, &loan, now, closure_reason)
                .await?;
            if notify_holds {
                readied_hold = self
                    .holds_notify_next_tx(&mut tx, loan.item_id, self.hold_ready_expiry_days())
                    .await?;
            }
        }

        Ok(CirculationApplyPending {
            tx,
            apply: CirculationApplyResult {
                loan_id: loan.id,
                item_id: loan.item_id,
                user_id: loan.user_id,
                item_status: next,
                borrowable,
                loan_closed: close_loan,
                charge,
                readied_hold,
            },
        })
    }

    /// Active loans whose item is in the claims-returned queue.
    pub async fn circulation_list_claims_returned(
        &self,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ClaimsReturnedQueueItem>, i64)> {
        let offset = (page - 1).max(0) * per_page;
        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM loans l
            JOIN items it ON it.id = l.item_id
            WHERE l.returned_at IS NULL
              AND it.circulation_status = $1
              AND it.archived_at IS NULL
            "#,
        )
        .bind(CirculationStatus::CLAIMED_RETURNED)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query(
            r#"
            SELECT
                l.id AS loan_id,
                l.item_id,
                l.user_id,
                l.date AS loan_start,
                l.expiry_at,
                COALESCE(l.notes, it.internal_note) AS notes,
                it.barcode,
                it.updated_at AS item_updated_at,
                b.title
            FROM loans l
            JOIN items it ON it.id = l.item_id
            JOIN biblios b ON b.id = it.biblio_id
            WHERE l.returned_at IS NULL
              AND it.circulation_status = $1
              AND it.archived_at IS NULL
            ORDER BY it.updated_at DESC NULLS LAST, l.id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(CirculationStatus::CLAIMED_RETURNED)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let items = rows
            .iter()
            .map(|row| ClaimsReturnedQueueItem {
                loan_id: row.get("loan_id"),
                item_id: row.get("item_id"),
                user_id: row.get("user_id"),
                barcode: row.get("barcode"),
                title: row.get("title"),
                loan_start: row.get("loan_start"),
                expiry_at: row.get("expiry_at"),
                item_updated_at: row.get("item_updated_at"),
                notes: row.get("notes"),
            })
            .collect();

        Ok((items, total))
    }

    /// Item price used as a replacement bill when staff omit an explicit amount.
    pub async fn circulation_item_price(&self, item_id: i64) -> AppResult<Option<Decimal>> {
        let price: Option<String> =
            sqlx::query_scalar("SELECT price FROM items WHERE id = $1 AND archived_at IS NULL")
                .bind(item_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        Ok(parse_item_price(price.as_deref()))
    }

    /// Accept a lost copy back, or record that staff left it lost.
    /// Lost status is not changed on `leave`. The replacement charge is
    /// cancelled or refunded only for `circulate`. `damaged` updates the item
    /// and leaves the charge as it is.
    ///
    /// The transaction stays open so the caller can write fine audit rows
    /// before commit. Dropping [`LostFoundPending`] rolls the resolution back.
    pub async fn circulation_resolve_lost_found(
        &self,
        item_id: i64,
        disposition: LostFoundDisposition,
        notes: Option<&str>,
    ) -> AppResult<LostFoundPending> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;

        let item_row = sqlx::query(
            r#"
            SELECT i.circulation_status, i.borrowable, i.barcode, b.title
            FROM items i
            JOIN biblios b ON b.id = i.biblio_id
            WHERE i.id = $1 AND i.archived_at IS NULL
            FOR UPDATE OF i
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        let current = CirculationStatus::from_db(item_row.get("circulation_status"));
        if current != CirculationStatus::Lost {
            return Err(AppError::BusinessRule(
                "Item is not marked lost".to_string(),
            ));
        }
        let title: Option<String> = item_row.get("title");
        let barcode: Option<String> = item_row.get("barcode");

        if disposition == LostFoundDisposition::Leave {
            return Ok(LostFoundPending {
                tx,
                apply: LostFoundApply {
                    item_id,
                    item_status: CirculationStatus::Lost,
                    borrowable: item_row.get("borrowable"),
                    settlement: None,
                    readied_hold: None,
                    title,
                    barcode,
                },
            });
        }

        let action = match disposition {
            LostFoundDisposition::Circulate => CirculationAction::AcceptFound,
            LostFoundDisposition::Damaged => CirculationAction::AcceptFoundDamaged,
            LostFoundDisposition::Leave => CirculationAction::AcceptFound,
        };
        let next = current
            .transition(action)
            .map_err(|msg| AppError::BusinessRule(msg.to_string()))?;
        let borrowable = next.borrowable_after();

        sqlx::query(
            r#"
            UPDATE items SET
                circulation_status = $2,
                borrowable = $3,
                updated_at = $4
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .bind(next.as_db())
        .bind(borrowable)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Finding the copy does not reopen the loan. It stays closed as lost.
        if matches!(
            disposition,
            LostFoundDisposition::Circulate | LostFoundDisposition::Damaged
        ) {
            self.loans_mark_lost_copy_found_tx(&mut tx, item_id, now)
                .await?;
        }

        // `damaged` must not cancel, refund, or waive. Relief is an explicit waive.
        let settlement = if disposition == LostFoundDisposition::Damaged {
            None
        } else {
            self.fines_settle_replacement_tx(&mut tx, item_id, notes)
                .await?
        };

        let readied_hold = if action.notify_holds() {
            self.holds_notify_next_tx(&mut tx, item_id, self.hold_ready_expiry_days())
                .await?
        } else {
            None
        };

        Ok(LostFoundPending {
            tx,
            apply: LostFoundApply {
                item_id,
                item_status: next,
                borrowable,
                settlement,
                readied_hold,
                title,
                barcode,
            },
        })
    }
}

/// Uncommitted lost-found resolution. Commit after fine audit rows are written.
pub struct LostFoundPending {
    pub tx: sqlx::Transaction<'static, sqlx::Postgres>,
    pub apply: LostFoundApply,
}

/// Persistence result for resolving a lost copy that was found.
pub struct LostFoundApply {
    pub item_id: i64,
    pub item_status: CirculationStatus,
    pub borrowable: bool,
    pub settlement: Option<LostFineSettlement>,
    pub readied_hold: Option<Hold>,
    pub title: Option<String>,
    pub barcode: Option<String>,
}

fn merge_notes(existing: Option<String>, extra: Option<&str>) -> Option<String> {
    match (existing, extra.map(str::trim).filter(|s| !s.is_empty())) {
        (Some(cur), Some(add)) if !cur.contains(add) => Some(format!("{cur}\n{add}")),
        (None, Some(add)) => Some(add.to_string()),
        (cur, _) => cur,
    }
}

pub(crate) fn parse_item_price(price: Option<&str>) -> Option<Decimal> {
    crate::models::item::parse_stored_item_price(price).filter(|amount| *amount > Decimal::ZERO)
}

#[cfg(test)]
mod tests {
    use super::parse_item_price;
    use rust_decimal::Decimal;

    #[test]
    fn parse_item_price_accepts_plain_decimals() {
        assert_eq!(parse_item_price(Some("12.50")), Some(Decimal::new(1250, 2)));
        assert_eq!(parse_item_price(Some("0")), None);
        assert_eq!(parse_item_price(Some("")), None);
        assert_eq!(parse_item_price(None), None);
    }
}
