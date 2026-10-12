//! Biblios repository — items operations.

use std::collections::HashMap;

use chrono::Utc;

use super::super::Repository;
use crate::models::item::ItemShort;
use crate::{
    error::{error_code, AppError, AppResult},
    models::item::Item,
};

use super::ItemShortRow;
use crate::models::item::WeedingStatus;

/// One round-trip of laterals for loan, transit, and the exclusive hold on a copy.
///
/// `copy_state.availability` applies this precedence (first match wins):
/// `onLoan`, `inTransit`, `onHold` (`to_pick`/`ready`), `toSearch`
/// (`circulation_status` 4), `unavailable` (not borrowable, lost, damaged,
/// claimed returned, withdrawn), else `available`.
const ITEM_STATE_JOINS: &str = r#"
LEFT JOIN LATERAL (
    SELECT l.id, l.expiry_at
    FROM loans l
    WHERE l.item_id = i.id AND l.returned_at IS NULL
    ORDER BY l.id DESC
    LIMIT 1
) active_loan ON TRUE
LEFT JOIN LATERAL (
    SELECT t.id
    FROM item_transits t
    WHERE t.item_id = i.id AND t.status IN ('requested', 'in_transit')
    LIMIT 1
) active_transit ON TRUE
LEFT JOIN LATERAL (
    SELECT
        h.user_id,
        COALESCE(
            NULLIF(btrim(concat_ws(' ', u.firstname, u.lastname)), ''),
            h.user_id::text
        ) AS user_name
    FROM holds h
    LEFT JOIN users u ON u.id = h.user_id
    WHERE h.item_id = i.id
      AND h.status IN ('to_pick', 'ready')
    ORDER BY CASE h.status WHEN 'ready' THEN 0 ELSE 1 END,
             h.position ASC,
             h.created_at ASC
    LIMIT 1
) assigned_hold ON TRUE
LEFT JOIN LATERAL (
    SELECT CASE
        WHEN active_loan.id IS NOT NULL THEN 'onLoan'
        WHEN active_transit.id IS NOT NULL THEN 'inTransit'
        WHEN assigned_hold.user_id IS NOT NULL THEN 'onHold'
        WHEN COALESCE(i.circulation_status, 0) = 4 THEN 'toSearch'
        WHEN i.borrowable = FALSE
          OR i.weeding_status = 'withdrawn'
          OR COALESCE(i.circulation_status, 0) IN (1, 2, 3)
        THEN 'unavailable'
        ELSE 'available'
    END AS availability
) copy_state ON TRUE
"#;

const ITEM_AVAILABILITY_COLUMNS: &str = r#"
                   copy_state.availability AS availability,
                   (
                       copy_state.availability IN ('available', 'onLoan')
                       AND i.weeding_status <> 'withdrawn'
                   ) AS can_hold_item,
                   CASE
                       WHEN copy_state.availability = 'onHold' THEN assigned_hold.user_id
                   END AS hold_user_id,
                   CASE
                       WHEN copy_state.availability = 'onHold' THEN assigned_hold.user_name
                   END AS hold_user_name
"#;

fn item_sql(where_and_order: &str) -> String {
    format!(
        r#"
            SELECT i.id, i.biblio_id, i.source_id, i.barcode, i.call_number, i.volume_designation,
                   i.place, i.borrowable, i.circulation_status, i.notes, i.price,
                   i.weeding_status, i.weeding_reason,
                   i.created_at, i.updated_at, i.archived_at,
                   so.name as source_name,
                   (active_loan.id IS NOT NULL) as borrowed,
                   active_loan.id as loan_id,
                   active_loan.expiry_at as due_date,
                   EXISTS (
                       SELECT 1 FROM holds h
                       WHERE h.status IN ('pending', 'to_pick', 'ready')
                         AND (
                             h.item_id = i.id
                             OR (h.item_id IS NULL AND h.biblio_id = i.biblio_id)
                         )
                   ) AS on_hold,
                   {ITEM_AVAILABILITY_COLUMNS}
            FROM items i
            LEFT JOIN sources so ON i.source_id = so.id
            {ITEM_STATE_JOINS}
            {where_and_order}
        "#
    )
}

fn item_short_sql(where_and_order: &str) -> String {
    format!(
        r#"
            SELECT i.biblio_id, i.id, i.barcode, i.call_number, i.borrowable,
                   i.weeding_status,
                   so.name as source_name,
                   (active_loan.id IS NOT NULL) as borrowed,
                   {ITEM_AVAILABILITY_COLUMNS}
            FROM items i
            LEFT JOIN sources so ON i.source_id = so.id
            {ITEM_STATE_JOINS}
            {where_and_order}
        "#
    )
}

impl Repository {
    /// Get items (physical copies) for a biblio (excludes archived items)
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_get_items(&self, biblio_id: i64) -> AppResult<Vec<Item>> {
        let items = sqlx::query_as::<_, Item>(&item_sql(
            "WHERE i.biblio_id = $1 AND i.archived_at IS NULL ORDER BY i.barcode",
        ))
        .bind(biblio_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    /// Get one active item by id (same row shape as [`biblios_get_items`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_active_by_id(&self, item_id: i64) -> AppResult<Item> {
        sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1 AND i.archived_at IS NULL"))
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))
    }

    /// Get one active item by barcode (same row shape as [`items_get_active_by_id`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_active_by_barcode(&self, barcode: &str) -> AppResult<Item> {
        sqlx::query_as::<_, Item>(&item_sql("WHERE i.barcode = $1 AND i.archived_at IS NULL"))
            .bind(barcode)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item with barcode {barcode} not found")))
    }

    /// Get ItemShort for many biblios (excludes archived). Used to attach items to BiblioShort lists.
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_get_items_short_by_biblio_ids(
        &self,
        biblio_ids: &[i64],
    ) -> AppResult<HashMap<i64, Vec<ItemShort>>> {
        if biblio_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<ItemShortRow> = sqlx::query_as(&item_short_sql(
            "WHERE i.biblio_id = ANY($1) AND i.archived_at IS NULL ORDER BY i.biblio_id, i.barcode",
        ))
        .bind(biblio_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut map: HashMap<i64, Vec<ItemShort>> = HashMap::new();
        for row in rows {
            map.entry(row.biblio_id)
                .or_default()
                .push(ItemShort::from(row));
        }
        Ok(map)
    }

    /// Create an item (physical copy) for a biblio
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_create_item(&self, biblio_id: i64, item: &Item) -> AppResult<Item> {
        let now = Utc::now();
        let mut new_item = item.clone();
        let source_id = if let Some(id) = item.source_id {
            Some(id)
        } else if let Some(ref name) = item.source_name {
            Some(self.sources_find_or_create_by_name(name).await?)
        } else if item.borrowable {
            // Incomplete received copies must not inherit the default site.
            match self.sources_get_default().await? {
                Some(default) => Some(default.id),
                None => None,
            }
        } else {
            None
        };
        new_item.source_id = source_id;

        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO items (
                biblio_id, barcode, call_number, volume_designation, place, borrowable, notes, price, source_id, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            RETURNING id
            "#,
        )
        .bind(biblio_id)
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&item.notes)
        .bind(&item.price)
        .bind(source_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        new_item.id = Some(id);
        Ok(new_item)
    }

    /// Upsert an item (physical copy)
    #[tracing::instrument(skip(self), err)]
    pub async fn upsert_item<'a>(&self, item: &'a mut Item) -> AppResult<&'a mut Item> {
        let now = Utc::now();
        item.updated_at = Some(now);
        if let Some(id) = item.id {
            sqlx::query(
                r#"
                UPDATE items SET
                    biblio_id = $1,
                    barcode = $2,
                    call_number = $3,
                    volume_designation = $4,
                    place = $5,
                    borrowable = $6,
                    notes = $7,
                    price = $8,
                    source_id = $9,
                    updated_at = $10,
                    archived_at = $11
                WHERE id = $12
                "#,
            )
            .bind(item.biblio_id)
            .bind(&item.barcode)
            .bind(&item.call_number)
            .bind(&item.volume_designation)
            .bind(item.place)
            .bind(item.borrowable)
            .bind(&item.notes)
            .bind(&item.price)
            .bind(item.source_id)
            .bind(item.updated_at)
            .bind(item.archived_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        } else {
            if let Some(ref barcode) = item.barcode {
                let existing_id =
                    sqlx::query_scalar::<_, i64>("SELECT id FROM items WHERE barcode = $1")
                        .bind(barcode)
                        .fetch_optional(&self.pool)
                        .await?;
                item.id = existing_id;
            }

            if let Some(id) = item.id {
                sqlx::query(
                    r#"
                    UPDATE items SET
                        biblio_id = $1,
                        barcode = $2,
                        call_number = $3,
                        volume_designation = $4,
                        place = $5,
                        borrowable = $6,
                        notes = $7,
                        price = $8,
                        source_id = $9,
                        updated_at = $10,
                        archived_at = $11
                    WHERE id = $12
                    "#,
                )
                .bind(item.biblio_id)
                .bind(&item.barcode)
                .bind(&item.call_number)
                .bind(&item.volume_designation)
                .bind(item.place)
                .bind(item.borrowable)
                .bind(&item.notes)
                .bind(&item.price)
                .bind(item.source_id)
                .bind(item.updated_at)
                .bind(item.archived_at)
                .bind(id)
                .execute(&self.pool)
                .await?;
            } else {
                let id = sqlx::query_scalar::<_, i64>(
                    r#"
                    INSERT INTO items (
                        biblio_id, barcode, call_number, volume_designation,
                        place, borrowable, notes, price, source_id, created_at, updated_at, archived_at
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10, $11)
                    RETURNING id
                    "#,
                )
                .bind(item.biblio_id)
                .bind(&item.barcode)
                .bind(&item.call_number)
                .bind(&item.volume_designation)
                .bind(item.place)
                .bind(item.borrowable)
                .bind(&item.notes)
                .bind(&item.price)
                .bind(item.source_id)
                .bind(item.updated_at)
                .bind(item.archived_at)
                .fetch_one(&self.pool)
                .await?;

                item.id = Some(id);
            }
        }

        Ok(item)
    }

    /// Update an item (physical copy)
    #[tracing::instrument(skip(self), err)]
    pub async fn items_update<'a>(&self, item: &'a mut Item) -> AppResult<&'a mut Item> {
        let now = Utc::now();
        item.updated_at = Some(now);
        sqlx::query(
            r#"
            UPDATE items SET
                barcode = COALESCE($1, barcode),
                call_number = COALESCE($2, call_number),
                volume_designation = COALESCE($3, volume_designation),
                place = COALESCE($4, place),
                borrowable = COALESCE($5, borrowable),
                notes = COALESCE($6, notes),
                price = COALESCE($7, price),
                source_id = COALESCE($8, source_id),
                updated_at = $9,
                archived_at = $10
            WHERE id = $11
            "#,
        )
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&item.notes)
        .bind(&item.price)
        .bind(item.source_id)
        .bind(item.updated_at)
        .bind(item.archived_at)
        .bind(item.id.unwrap_or(0))
        .execute(&self.pool)
        .await?;

        Ok(item)
    }

    /// Delete an item (physical copy — soft delete, sets archived_at).
    ///
    /// `force` does not bypass circulation history. A copy with any loan, hold, or fine
    /// must be weeded instead.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_delete(&self, id: i64, force: bool) -> AppResult<()> {
        let _ = force;
        let now = Utc::now();

        let (has_loan, has_hold, has_fine): (bool, bool, bool) = sqlx::query_as(
            r#"
            SELECT
                (
                    EXISTS (SELECT 1 FROM loans WHERE item_id = $1)
                    OR EXISTS (SELECT 1 FROM loans_archives WHERE item_id = $1)
                ),
                EXISTS (SELECT 1 FROM holds WHERE item_id = $1),
                (
                    EXISTS (
                        SELECT 1 FROM fines f
                        JOIN loans l ON l.id = f.loan_id
                        WHERE l.item_id = $1
                    )
                    OR EXISTS (
                        SELECT 1 FROM fines f
                        JOIN loans_archives la ON la.id = f.loan_id
                        WHERE la.item_id = $1
                    )
                )
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if has_loan || has_hold || has_fine {
            let mut kinds = Vec::new();
            if has_loan {
                kinds.push("loan");
            }
            if has_hold {
                kinds.push("hold");
            }
            if has_fine {
                kinds.push("fine");
            }
            return Err(AppError::coded(
                422,
                error_code::ITEM_HAS_HISTORY_USE_WEEDING,
                format!(
                    "This copy has {} history and cannot be deleted. Weed it instead.",
                    kinds.join(", ")
                ),
            ));
        }

        sqlx::query("UPDATE items SET archived_at = $1, updated_at = $1, barcode = CONCAT('ARCH_', id::text, '_', COALESCE(barcode, '')) WHERE id = $2 AND archived_at IS NULL")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Check if item barcode already exists
    #[tracing::instrument(skip(self), err)]
    pub async fn items_barcode_exists(
        &self,
        barcode: &str,
        exclude_item_id: Option<i64>,
    ) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_item_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE barcode = $1 AND id != $2)")
                .bind(barcode)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE barcode = $1)")
                .bind(barcode)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists)
    }

    /// Get item id and archived_at by barcode
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_by_barcode(&self, barcode: &str) -> AppResult<Option<(i64, bool)>> {
        let row: Option<(i64, Option<chrono::DateTime<Utc>>)> =
            sqlx::query_as("SELECT id, archived_at FROM items WHERE barcode = $1")
                .bind(barcode)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id, archived_at)| (id, archived_at.is_some())))
    }

    /// Reactivate an archived item and update its fields.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_reactivate(
        &self,
        item_id: i64,
        biblio_id: i64,
        item: &Item,
    ) -> AppResult<Item> {
        let now = Utc::now();
        let source_id = if let Some(id) = item.source_id {
            Some(id)
        } else if let Some(ref name) = item.source_name {
            Some(self.sources_find_or_create_by_name(name).await?)
        } else if let Some(default) = self.sources_get_default().await? {
            Some(default.id)
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE items SET
                biblio_id = $1, barcode = $2, call_number = $3, volume_designation = $4,
                place = $5, borrowable = $6,
                notes = $7, price = $8, source_id = $9,
                archived_at = NULL,
                updated_at = $10
            WHERE id = $11
            "#,
        )
        .bind(biblio_id)
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&item.notes)
        .bind(&item.price)
        .bind(source_id)
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1"))
            .bind(item_id)
            .fetch_one(&self.pool)
            .await
            .map_err(Into::into)
    }
    /// Find an existing item by barcode and return its short representation.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_find_short_by_barcode(
        &self,
        barcode: &str,
        exclude_item_id: Option<i64>,
    ) -> AppResult<Option<ItemShort>> {
        let row: Option<ItemShortRow> = if let Some(eid) = exclude_item_id {
            sqlx::query_as(&item_short_sql(
                "WHERE i.barcode = $1 AND i.id != $2 AND i.archived_at IS NULL LIMIT 1",
            ))
            .bind(barcode)
            .bind(eid)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as(&item_short_sql(
                "WHERE i.barcode = $1 AND i.archived_at IS NULL LIMIT 1",
            ))
            .bind(barcode)
            .fetch_optional(&self.pool)
            .await?
        };
        Ok(row.map(ItemShort::from))
    }

    /// Set candidate or on-shelf weeding status.
    /// Withdrawal is a separate event (`POST /items/{id}/withdrawals`).
    /// Does not touch `archived_at` or circulation exceptions.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_set_weeding(
        &self,
        item_id: i64,
        status: WeedingStatus,
        reason: Option<String>,
    ) -> AppResult<Item> {
        let reason = reason
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        if reason.as_ref().is_some_and(|s| s.chars().count() > 200) {
            return Err(AppError::Validation(
                "Weeding reason must be at most 200 characters".to_string(),
            ));
        }

        if status == WeedingStatus::Withdrawn {
            return Err(AppError::coded(
                422,
                error_code::WITHDRAWN_VIA_WITHDRAWAL,
                "Withdraw a copy with POST /api/v1/items/{id}/withdrawals",
            ));
        }

        let mut tx = self.pool.begin().await?;
        let current: Option<WeedingStatus> = sqlx::query_scalar(
            "SELECT weeding_status FROM items WHERE id = $1 AND archived_at IS NULL FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Err(AppError::NotFound(format!("Item {item_id} not found")));
        };
        let active_withdrawal: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item_withdrawals WHERE item_id = $1 AND cancelled_at IS NULL)",
        )
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await?;
        if current == WeedingStatus::Withdrawn || active_withdrawal {
            return Err(AppError::coded(
                422,
                error_code::WITHDRAWAL_USE_CANCEL,
                "Cancel the active withdrawal before changing weeding status",
            ));
        }

        sqlx::query(
            r#"
            UPDATE items
            SET weeding_status = $2,
                weeding_reason = $3,
                updated_at = NOW()
            WHERE id = $1 AND archived_at IS NULL
            "#,
        )
        .bind(item_id)
        .bind(status)
        .bind(&reason)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        self.items_get_active_by_id(item_id).await
    }
}
