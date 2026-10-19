//! Lenders and incoming deposit batches.

use super::Repository;
use crate::error::{error_code, AppError, AppResult};
use crate::models::deposit::{
    own_stock_sql, CreateDepositBatch, CreateLender, DepositBatch, DepositBatchCounts,
    DepositBatchItem, DepositBatchStatus, DepositCopyState, Lender, UpdateDepositBatch,
    UpdateLender,
};

const LENDER_COLUMNS: &str = "id, name, contact, notes, archived_at, created_at, updated_at";

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
            INSERT INTO lenders (name, contact, notes)
            VALUES ($1, $2, $3)
            RETURNING {LENDER_COLUMNS}
            "#
        ))
        .bind(name)
        .bind(blank_to_null(data.contact.clone()))
        .bind(blank_to_null(data.notes.clone()))
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
        let result = sqlx::query_as::<_, Lender>(&format!(
            r#"
            UPDATE lenders
            SET name = $2, contact = $3, notes = $4, updated_at = now()
            WHERE id = $1
            RETURNING {LENDER_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(name)
        .bind(contact)
        .bind(notes)
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

    pub async fn sources_exists(&self, id: i64) -> AppResult<bool> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id = $1)")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(exists)
    }
}

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
    WHEN i.deposit_returned_at IS NOT NULL THEN 'returned'
    WHEN active_loan.id IS NOT NULL THEN 'onLoan'
    WHEN active_transit.id IS NOT NULL THEN 'inTransit'
    WHEN assigned_hold.user_id IS NOT NULL THEN 'onHold'
    WHEN i.archived_at IS NOT NULL OR COALESCE(i.circulation_status, 0) = 5 THEN 'missing'
    WHEN i.borrowable = FALSE
      OR i.weeding_status = 'withdrawn'
      OR COALESCE(i.circulation_status, 0) IN (1, 2, 3, 4)
    THEN 'unavailable'
    ELSE 'onShelf'
END
"#;
