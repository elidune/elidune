//! Lenders and incoming deposit batches.

use super::Repository;
use crate::error::{error_code, AppError, AppResult};
use crate::models::deposit::{
    own_stock_sql, CreateDepositBatch, CreateLender, DepositActiveLoanCopy, DepositBatch,
    DepositBatchCounts, DepositBatchItem, DepositBatchStatus, DepositCloseCopy, DepositCloseReport,
    DepositCopyState, DepositScanOut, Lender, UpdateDepositBatch, UpdateLender,
};
use crate::models::fine::Cents;

const LENDER_COLUMNS: &str =
    "id, name, contact, notes, replacement_rate_cents, archived_at, created_at, updated_at";

const BATCH_COLUMNS: &str = r#"
    id, lender_id, reference, site_id, arrived_on, return_due_on, status,
    closed_at, notes, created_at, updated_at
"#;

fn blank_to_null(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn map_unique(err: sqlx::Error, message: String) -> AppError {
    let code = match &err {
        sqlx::Error::Database(db) => db.code().as_deref().map(ToOwned::to_owned),
        _ => None,
    };
    match code.as_deref() {
        Some("23505") => AppError::Conflict(message),
        Some("23514") => AppError::Validation("returnDueOn must be on or after arrivedOn".into()),
        _ => AppError::Database(Box::new(err)),
    }
}

impl Repository {
    pub async fn lenders_list(&self, include_archived: bool) -> AppResult<Vec<Lender>> {
        let sql = if include_archived {
            format!("SELECT {LENDER_COLUMNS} FROM lenders ORDER BY lower(name), id")
        } else {
            format!(
                "SELECT {LENDER_COLUMNS} FROM lenders WHERE archived_at IS NULL ORDER BY lower(name), id"
            )
        };
        let rows = sqlx::query_as::<_, Lender>(&sql)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn lenders_get(&self, id: i64) -> AppResult<Lender> {
        sqlx::query_as::<_, Lender>(&format!(
            "SELECT {LENDER_COLUMNS} FROM lenders WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Lender {id} not found")))
    }

    pub async fn lenders_create(&self, data: &CreateLender) -> AppResult<Lender> {
        let name = data.name.trim();
        let result = sqlx::query_as::<_, Lender>(&format!(
            r#"
            INSERT INTO lenders (name, contact, notes, replacement_rate_cents)
            VALUES ($1, $2, $3, $4)
            RETURNING {LENDER_COLUMNS}
            "#
        ))
        .bind(name)
        .bind(blank_to_null(data.contact.clone()))
        .bind(blank_to_null(data.notes.clone()))
        .bind(data.replacement_rate_cents)
        .fetch_one(&self.pool)
        .await;

        result.map_err(|err| map_unique(err, "A lender with this name already exists".into()))
    }

    pub async fn lenders_update(&self, id: i64, data: &UpdateLender) -> AppResult<Lender> {
        let existing = self.lenders_get(id).await?;
        let name = match data.name.as_deref() {
            Some(name) => name.trim().to_string(),
            None => existing.name,
        };
        let contact = match &data.contact {
            Some(value) => blank_to_null(Some(value.clone())),
            None => existing.contact,
        };
        let notes = match &data.notes {
            Some(value) => blank_to_null(Some(value.clone())),
            None => existing.notes,
        };
        let replacement_rate_cents = match data.replacement_rate_cents {
            Some(value) => value,
            None => existing.replacement_rate_cents,
        };
        let result = sqlx::query_as::<_, Lender>(&format!(
            r#"
            UPDATE lenders
            SET name = $2, contact = $3, notes = $4, replacement_rate_cents = $5, updated_at = now()
            WHERE id = $1
            RETURNING {LENDER_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(name)
        .bind(contact)
        .bind(notes)
        .bind(replacement_rate_cents)
        .fetch_one(&self.pool)
        .await;

        result.map_err(|err| map_unique(err, "A lender with this name already exists".into()))
    }

    /// Archive a lender. Refused while any of its batches is not closed.
    pub async fn lenders_archive(&self, id: i64) -> AppResult<Lender> {
        let existing = self.lenders_get(id).await?;
        if existing.archived_at.is_some() {
            return Ok(existing);
        }
        let open: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM deposit_batches WHERE lender_id = $1 AND status <> 'closed'",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        if open > 0 {
            return Err(AppError::coded(
                422,
                error_code::LENDER_HAS_ACTIVE_BATCHES,
                "This lender still has a batch that is not closed",
            ));
        }
        let lender = sqlx::query_as::<_, Lender>(&format!(
            r#"
            UPDATE lenders
            SET archived_at = now(), updated_at = now()
            WHERE id = $1
            RETURNING {LENDER_COLUMNS}
            "#
        ))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(lender)
    }

    pub async fn deposit_batches_list(
        &self,
        lender_id: Option<i64>,
        status: Option<DepositBatchStatus>,
    ) -> AppResult<Vec<DepositBatch>> {
        let rows = sqlx::query_as::<_, DepositBatch>(&format!(
            r#"
            SELECT {BATCH_COLUMNS}
            FROM deposit_batches
            WHERE ($1::bigint IS NULL OR lender_id = $1)
              AND ($2::text IS NULL OR status = $2)
            ORDER BY arrived_on DESC, id DESC
            "#
        ))
        .bind(lender_id)
        .bind(status.map(|status| status.as_db_str().to_string()))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn deposit_batches_get(&self, id: i64) -> AppResult<DepositBatch> {
        sqlx::query_as::<_, DepositBatch>(&format!(
            "SELECT {BATCH_COLUMNS} FROM deposit_batches WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Deposit batch {id} not found")))
    }

    pub async fn deposit_batch_counts(&self, id: i64) -> AppResult<DepositBatchCounts> {
        let owned = own_stock_sql("i");
        let sql = format!(
            r#"
            SELECT
                COUNT(*)::bigint AS total,
                COUNT(*) FILTER (WHERE bucket = 'onShelf')::bigint AS on_shelf,
                COUNT(*) FILTER (WHERE bucket = 'onLoan')::bigint AS on_loan,
                COUNT(*) FILTER (WHERE bucket = 'onHold')::bigint AS on_hold,
                COUNT(*) FILTER (WHERE bucket = 'inTransit')::bigint AS in_transit,
                COUNT(*) FILTER (WHERE bucket = 'returned')::bigint AS returned
            FROM (
                SELECT {COPY_BUCKET} AS bucket
                FROM items i
                {COPY_LATERALS}
                WHERE NOT ({owned}) AND i.deposit_batch_id = $1
            ) buckets
            "#
        );
        let row = sqlx::query_as::<_, DepositBatchCounts>(&sql)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn deposit_batches_create(
        &self,
        data: &CreateDepositBatch,
    ) -> AppResult<DepositBatch> {
        let reference = data.reference.trim();
        let result = sqlx::query_as::<_, DepositBatch>(&format!(
            r#"
            INSERT INTO deposit_batches (
                lender_id, reference, site_id, arrived_on, return_due_on, notes
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING {BATCH_COLUMNS}
            "#
        ))
        .bind(data.lender_id)
        .bind(reference)
        .bind(data.site_id)
        .bind(data.arrived_on)
        .bind(data.return_due_on)
        .bind(blank_to_null(data.notes.clone()))
        .fetch_one(&self.pool)
        .await;

        result.map_err(|err| {
            map_unique(
                err,
                "A deposit batch with this reference already exists for that lender".into(),
            )
        })
    }

    /// Update `returnDueOn`, `siteId`, and `notes` while the batch is active.
    /// `siteId` is stored on the batch only. Copies keep their current `source_id`.
    pub async fn deposit_batches_update(
        &self,
        id: i64,
        data: &UpdateDepositBatch,
    ) -> AppResult<DepositBatch> {
        let existing = self.deposit_batches_get(id).await?;
        if existing.status != DepositBatchStatus::Active {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_BATCH_NOT_ACTIVE,
                "Only an active deposit batch can be updated",
            ));
        }
        let return_due_on = data.return_due_on.unwrap_or(existing.return_due_on);
        if return_due_on < existing.arrived_on {
            return Err(AppError::Validation(
                "returnDueOn must be on or after arrivedOn".into(),
            ));
        }
        let site_id = data.site_id.unwrap_or(existing.site_id);
        let notes = match &data.notes {
            Some(value) => blank_to_null(Some(value.clone())),
            None => existing.notes,
        };
        let result = sqlx::query_as::<_, DepositBatch>(&format!(
            r#"
            UPDATE deposit_batches
            SET return_due_on = $2, site_id = $3, notes = $4, updated_at = now()
            WHERE id = $1 AND status = 'active'
            RETURNING {BATCH_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(return_due_on)
        .bind(site_id)
        .bind(notes)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(batch)) => Ok(batch),
            Ok(None) => Err(AppError::coded(
                422,
                error_code::DEPOSIT_BATCH_NOT_ACTIVE,
                "Only an active deposit batch can be updated",
            )),
            Err(err) => Err(map_unique(err, "Could not update the deposit batch".into())),
        }
    }

    pub async fn deposit_batch_items(
        &self,
        id: i64,
        state: Option<DepositCopyState>,
    ) -> AppResult<Vec<DepositBatchItem>> {
        let _ = self.deposit_batches_get(id).await?;
        let owned = own_stock_sql("i");
        let sql = format!(
            r#"
            SELECT i.id AS item_id, i.biblio_id, i.barcode, b.title, bucket.state
            FROM items i
            JOIN biblios b ON b.id = i.biblio_id
            {COPY_LATERALS}
            CROSS JOIN LATERAL (SELECT {COPY_BUCKET} AS state) bucket
            WHERE NOT ({owned}) AND i.deposit_batch_id = $1
              AND ($2::text IS NULL OR bucket.state = $2)
            ORDER BY i.barcode NULLS LAST, i.id
            "#
        );
        let rows = sqlx::query_as::<_, DepositBatchItem>(&sql)
            .bind(id)
            .bind(state.map(|state| state.as_db_str().to_string()))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Deposit copies declared lost or damaged.
    ///
    /// `notified` false is `deposit_loss_notified_at IS NULL`. The row is the
    /// latest replacement or damage fine on that copy. `amount_cents` is that
    /// fine's frozen amount.
    pub async fn deposit_losses_list(
        &self,
        lender_id: Option<i64>,
        notified: bool,
    ) -> AppResult<Vec<crate::models::deposit::DepositLoss>> {
        let sql = format!(
            r#"
            SELECT {DEPOSIT_LOSS_SELECT}
            {DEPOSIT_LOSS_FROM}
            WHERE ($1::bigint IS NULL OR db.lender_id = $1)
              AND (
                  ($2::bool IS FALSE AND i.deposit_loss_notified_at IS NULL)
                  OR ($2::bool IS TRUE AND i.deposit_loss_notified_at IS NOT NULL)
              )
            ORDER BY fine.created_at DESC, i.id
            "#
        );
        let rows = sqlx::query_as::<_, crate::models::deposit::DepositLoss>(&sql)
            .bind(lender_id)
            .bind(notified)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Lock one item and refuse unless it is a deposit copy with a replacement
    /// or damage fine that has not been notified yet.
    ///
    /// Returns the lender id. `deposit_loss_already_notified` does not write
    /// an audit row; the caller must not continue the transaction.
    pub async fn deposit_loss_lock_for_notify(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
    ) -> AppResult<i64> {
        let row = sqlx::query_as::<_, DepositLossLock>(
            r#"
            SELECT i.deposit_batch_id,
                   db.lender_id,
                   i.deposit_loss_notified_at,
                   EXISTS (
                       SELECT 1 FROM fines f
                       WHERE f.item_id = i.id
                         AND f.charge_type IN ('replacement', 'damage')
                   ) AS declared
            FROM items i
            LEFT JOIN deposit_batches db ON db.id = i.deposit_batch_id
            WHERE i.id = $1
            FOR UPDATE OF i
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(row) = row else {
            return Err(AppError::NotFound(format!("Item {item_id} not found")));
        };
        if row.deposit_batch_id.is_none() || !row.declared {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_LOSS_NOT_DECLARED,
                "This copy is not a deposit copy declared lost or damaged",
            ));
        }
        if row.deposit_loss_notified_at.is_some() {
            return Err(AppError::coded(
                409,
                error_code::DEPOSIT_LOSS_ALREADY_NOTIFIED,
                "The lender was already notified about this copy",
            ));
        }
        row.lender_id.ok_or_else(|| {
            AppError::Internal(format!("Deposit copy {item_id} is missing its lender"))
        })
    }

    pub async fn deposit_loss_set_notified(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        notified_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let updated = sqlx::query(
            r#"
            UPDATE items
            SET deposit_loss_notified_at = $2, updated_at = now()
            WHERE id = $1 AND deposit_loss_notified_at IS NULL
            "#,
        )
        .bind(item_id)
        .bind(notified_at)
        .execute(&mut **tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(AppError::coded(
                409,
                error_code::DEPOSIT_LOSS_ALREADY_NOTIFIED,
                "The lender was already notified about this copy",
            ));
        }
        Ok(())
    }

    pub async fn deposit_loss_by_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
    ) -> AppResult<crate::models::deposit::DepositLoss> {
        let sql = format!(
            r#"
            SELECT {DEPOSIT_LOSS_SELECT}
            {DEPOSIT_LOSS_FROM}
            WHERE i.id = $1
            "#
        );
        sqlx::query_as::<_, crate::models::deposit::DepositLoss>(&sql)
            .bind(item_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or_else(|| {
                AppError::Internal(format!(
                    "Declared deposit loss for item {item_id} disappeared"
                ))
            })
    }

    /// Lender replacement rate and the copy price for a deposit item.
    /// `None` when the copy is own stock.
    pub async fn item_deposit_replacement(
        &self,
        item_id: i64,
    ) -> AppResult<Option<DepositReplacement>> {
        let row = sqlx::query_as::<_, DepositReplacement>(
            r#"
            SELECT db.lender_id, l.replacement_rate_cents, i.price AS item_price_cents
            FROM items i
            JOIN deposit_batches db ON db.id = i.deposit_batch_id
            JOIN lenders l ON l.id = db.lender_id
            WHERE i.id = $1
            "#,
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Cap `expiry` at the copy's batch `return_due_on`.
    ///
    /// Own stock is returned unchanged. A batch due today or earlier is
    /// [`error_code::DEPOSIT_RETURN_DUE`]. When `block_closed` is set, an
    /// archived copy or a closed batch is [`error_code::ITEM_NOT_LOANABLE`]
    /// unless the return date is already due (that code wins).
    pub(crate) async fn cap_item_due_date_conn(
        &self,
        conn: &mut sqlx::PgConnection,
        item_id: i64,
        expiry: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
        block_closed: bool,
    ) -> AppResult<DepositDueDecision> {
        let row = sqlx::query_as::<_, DepositDueRow>(
            r#"
            SELECT db.return_due_on, db.status, i.archived_at
            FROM items i
            JOIN deposit_batches db ON db.id = i.deposit_batch_id
            WHERE i.id = $1
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut *conn)
        .await?;
        let Some(row) = row else {
            return Ok(DepositDueDecision {
                expiry_at: expiry,
                capped: false,
                is_deposit: false,
            });
        };
        let zone = self.library_zone_conn(conn).await?;
        let today = crate::services::calendar::local_date(zone, now);
        if block_closed
            && (row.archived_at.is_some() || row.status == DepositBatchStatus::Closed)
            && row.return_due_on > today
        {
            return Err(AppError::coded(
                422,
                error_code::ITEM_NOT_LOANABLE,
                "This deposit copy has been returned to the lender",
            ));
        }
        match crate::services::calendar::cap_loan_due_date(expiry, row.return_due_on, today, zone) {
            Some(capped) => Ok(DepositDueDecision {
                expiry_at: capped.expiry_at,
                capped: capped.capped,
                is_deposit: true,
            }),
            None => Err(AppError::coded(
                422,
                error_code::DEPOSIT_RETURN_DUE,
                "This deposit copy is due back to the lender today or is already overdue",
            )),
        }
    }

    /// `active` → `returning`. Already `returning` is a no-op.
    /// Holds pinned to the batch's copies go back to the title queue.
    pub async fn deposit_batches_start_return(&self, id: i64) -> AppResult<DepositBatch> {
        let mut tx = self.pool.begin().await?;
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM deposit_batches WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(status) = status else {
            return Err(AppError::NotFound(format!("Deposit batch {id} not found")));
        };
        if status == DepositBatchStatus::Closed.as_db_str() {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_BATCH_NOT_ACTIVE,
                "A closed deposit batch cannot start a return",
            ));
        }
        if status == DepositBatchStatus::Active.as_db_str() {
            sqlx::query(
                r#"
                UPDATE deposit_batches
                SET status = 'returning', updated_at = now()
                WHERE id = $1 AND status = 'active'
                "#,
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        let item_ids: Vec<i64> =
            sqlx::query_scalar("SELECT id FROM items WHERE deposit_batch_id = $1")
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;
        self.holds_release_deposit_copies_tx(&mut tx, &item_ids)
            .await?;
        tx.commit().await?;
        self.deposit_batches_get(id).await
    }

    /// Set `deposit_returned_at` on one copy of a `returning` batch.
    /// A second scan keeps the first timestamp.
    pub async fn deposit_batches_scan_out(
        &self,
        id: i64,
        barcode: &str,
    ) -> AppResult<DepositScanOut> {
        let barcode = barcode.trim();
        if barcode.is_empty() {
            return Err(AppError::Validation("barcode is required".into()));
        }
        let mut tx = self.pool.begin().await?;
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM deposit_batches WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(status) = status else {
            return Err(AppError::NotFound(format!("Deposit batch {id} not found")));
        };
        if status != DepositBatchStatus::Returning.as_db_str() {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_BATCH_NOT_RETURNING,
                "Copies can be scanned out only while the batch is returning",
            ));
        }
        let row = sqlx::query_as::<_, ScanOutRow>(
            r#"
            SELECT id, barcode, deposit_returned_at
            FROM items
            WHERE deposit_batch_id = $1 AND barcode = $2
            FOR UPDATE
            "#,
        )
        .bind(id)
        .bind(barcode)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            return Err(AppError::NotFound(
                "This barcode is not a copy of this deposit batch".into(),
            ));
        };
        let on_loan: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE item_id = $1 AND returned_at IS NULL)",
        )
        .bind(row.id)
        .fetch_one(&mut *tx)
        .await?;
        if on_loan {
            return Err(AppError::deposit_item_on_loan(row.id, row.barcode.clone()));
        }
        if let Some(returned_at) = row.deposit_returned_at {
            tx.commit().await?;
            return Ok(DepositScanOut {
                item_id: row.id,
                barcode: row.barcode.unwrap_or_else(|| barcode.to_string()),
                deposit_returned_at: returned_at,
                already_scanned: true,
            });
        }
        let returned_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
            r#"
            UPDATE items
            SET deposit_returned_at = now(), updated_at = now()
            WHERE id = $1
            RETURNING deposit_returned_at
            "#,
        )
        .bind(row.id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(DepositScanOut {
            item_id: row.id,
            barcode: row.barcode.unwrap_or_else(|| barcode.to_string()),
            deposit_returned_at: returned_at,
            already_scanned: false,
        })
    }

    /// `returning` → `closed` once every copy is returned or declared lost.
    /// An open loan refuses the close and leaves the batch `returning`.
    /// Does not delete items. A closed batch returns the same report again.
    pub async fn deposit_batches_close(&self, id: i64) -> AppResult<DepositCloseReport> {
        let mut tx = self.pool.begin().await?;
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM deposit_batches WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(status) = status else {
            return Err(AppError::NotFound(format!("Deposit batch {id} not found")));
        };
        if status == DepositBatchStatus::Active.as_db_str() {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_BATCH_NOT_RETURNING,
                "Start the return before closing the batch",
            ));
        }
        if status == DepositBatchStatus::Returning.as_db_str() {
            sqlx::query("SELECT id FROM items WHERE deposit_batch_id = $1 FOR UPDATE")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            let on_loan = sqlx::query_as::<_, DepositActiveLoanCopy>(
                r#"
                SELECT i.id AS item_id, i.barcode
                FROM items i
                JOIN loans l ON l.item_id = i.id AND l.returned_at IS NULL
                WHERE i.deposit_batch_id = $1
                ORDER BY i.barcode NULLS LAST, i.id
                "#,
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            if !on_loan.is_empty() {
                return Err(AppError::deposit_batch_has_active_loans(on_loan));
            }
            sqlx::query(
                r#"
                UPDATE items i
                SET archived_at = now(), updated_at = now()
                WHERE i.deposit_batch_id = $1
                  AND i.archived_at IS NULL
                  AND NOT EXISTS (
                      SELECT 1 FROM loans l
                      WHERE l.item_id = i.id AND l.returned_at IS NULL
                  )
                "#,
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                UPDATE deposit_batches
                SET status = 'closed', closed_at = now(), updated_at = now()
                WHERE id = $1 AND status = 'returning'
                "#,
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        let report = self.deposit_close_report_tx(&mut tx, id).await?;
        tx.commit().await?;
        Ok(report)
    }

    async fn deposit_close_report_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: i64,
    ) -> AppResult<DepositCloseReport> {
        let rows = sqlx::query_as::<_, ClassifiedCopy>(
            r#"
            SELECT i.id AS item_id, i.barcode, b.title,
                CASE
                    WHEN EXISTS (
                        SELECT 1 FROM loans l
                        WHERE l.item_id = i.id AND l.returned_at IS NULL
                    ) THEN 'onLoan'
                    WHEN i.deposit_returned_at IS NOT NULL THEN 'returned'
                    ELSE 'missing'
                END AS bucket
            FROM items i
            JOIN biblios b ON b.id = i.biblio_id
            WHERE i.deposit_batch_id = $1
            ORDER BY i.barcode NULLS LAST, i.id
            "#,
        )
        .bind(id)
        .fetch_all(&mut **tx)
        .await?;
        let mut report = DepositCloseReport::default();
        for row in rows {
            let copy = DepositCloseCopy {
                item_id: row.item_id,
                barcode: row.barcode,
                title: row.title,
            };
            match row.bucket.as_str() {
                "returned" => report.returned.push(copy),
                "missing" => report.missing.push(copy),
                _ => {}
            }
        }
        Ok(report)
    }

    pub async fn sources_exists(&self, id: i64) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id = $1)")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(exists)
    }
}

const DEPOSIT_LOSS_SELECT: &str = r#"
    i.id AS item_id,
    i.barcode,
    b.title,
    db.lender_id,
    l.name AS lender_name,
    db.id AS batch_id,
    db.reference,
    CASE fine.charge_type WHEN 'damage' THEN 'damaged' ELSE 'lost' END AS kind,
    fine.amount AS amount_cents,
    fine.id AS fine_id,
    fine.created_at AS declared_at,
    i.deposit_loss_notified_at
"#;

const DEPOSIT_LOSS_FROM: &str = r#"
    FROM items i
    JOIN deposit_batches db ON db.id = i.deposit_batch_id
    JOIN lenders l ON l.id = db.lender_id
    JOIN biblios b ON b.id = i.biblio_id
    JOIN LATERAL (
        SELECT f.id, f.amount, f.charge_type, f.created_at
        FROM fines f
        WHERE f.item_id = i.id
          AND f.charge_type IN ('replacement', 'damage')
        ORDER BY f.created_at DESC, f.id DESC
        LIMIT 1
    ) fine ON TRUE
"#;

/// Shared laterals for batch copy counts and the item list.
const COPY_LATERALS: &str = r#"
LEFT JOIN LATERAL (
    SELECT l.id
    FROM loans l
    WHERE l.item_id = i.id AND l.returned_at IS NULL
    LIMIT 1
) active_loan ON TRUE
LEFT JOIN LATERAL (
    SELECT t.id
    FROM item_transits t
    WHERE t.item_id = i.id AND t.status IN ('requested', 'in_transit')
    LIMIT 1
) active_transit ON TRUE
LEFT JOIN LATERAL (
    SELECT h.user_id
    FROM holds h
    WHERE h.item_id = i.id AND h.status IN ('to_pick', 'ready')
    LIMIT 1
) assigned_hold ON TRUE
"#;

const COPY_BUCKET: &str = r#"
CASE
    WHEN active_loan.id IS NOT NULL THEN 'onLoan'
    WHEN i.deposit_returned_at IS NOT NULL THEN 'returned'
    WHEN i.archived_at IS NOT NULL OR COALESCE(i.circulation_status, 0) = 5 THEN 'missing'
    WHEN active_transit.id IS NOT NULL THEN 'inTransit'
    WHEN assigned_hold.user_id IS NOT NULL THEN 'onHold'
    WHEN i.borrowable = FALSE
      OR i.weeding_status = 'withdrawn'
      OR COALESCE(i.circulation_status, 0) IN (1, 2, 3, 4)
    THEN 'unavailable'
    ELSE 'onShelf'
END
"#;

/// Facts used to cap a loan or renewal due date.
pub(crate) struct DepositDueDecision {
    pub expiry_at: chrono::DateTime<chrono::Utc>,
    pub capped: bool,
    pub is_deposit: bool,
}

/// Lender rate and copy price used when a deposit copy is lost or damaged.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DepositReplacement {
    pub lender_id: i64,
    pub replacement_rate_cents: Option<Cents>,
    /// `items.price` in cents. Zero and null are not a billable slip price.
    pub item_price_cents: Option<Cents>,
}

#[derive(Debug, sqlx::FromRow)]
struct DepositDueRow {
    return_due_on: chrono::NaiveDate,
    status: DepositBatchStatus,
    archived_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct DepositLossLock {
    deposit_batch_id: Option<i64>,
    lender_id: Option<i64>,
    deposit_loss_notified_at: Option<chrono::DateTime<chrono::Utc>>,
    declared: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct ScanOutRow {
    id: i64,
    barcode: Option<String>,
    deposit_returned_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
struct ClassifiedCopy {
    item_id: i64,
    barcode: Option<String>,
    title: Option<String>,
    bucket: String,
}
