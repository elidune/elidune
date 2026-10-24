//! Biblios repository — items operations.

use std::collections::HashMap;

use chrono::Utc;

use super::super::Repository;
use crate::models::item::ItemShort;
use crate::{
    error::{error_code, AppError, AppResult},
    models::item::{note_stored_value, note_write_value, Item},
};

use super::ItemShortRow;
use crate::models::deposit::{item_deposit_join, item_deposit_json};
use crate::models::item::WeedingStatus;

/// One round-trip of laterals for loan, transit, and the exclusive hold on a copy.
///
/// `copy_state.availability` applies this precedence (first match wins):
/// `onLoan`, `inTransit`, `onHold` (`to_pick`/`ready`), `toSearch`
/// (`circulation_status` 4), `unavailable` (not borrowable, lost, damaged,
/// claimed returned, declared missing, withdrawn, deposit returned to lender),
/// else `available`.
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
    SELECT h.user_id, u.barcode, u.firstname, u.lastname
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
        WHEN i.deposit_returned_at IS NOT NULL THEN 'unavailable'
        WHEN i.borrowable = FALSE
          OR i.weeding_status = 'withdrawn'
          OR COALESCE(i.circulation_status, 0) IN (1, 2, 3, 5)
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
                       WHEN copy_state.availability = 'onHold' THEN NULLIF(btrim(assigned_hold.barcode), '')
                   END AS hold_user_barcode,
                   CASE
                       WHEN copy_state.availability = 'onHold' THEN assigned_hold.firstname
                   END AS hold_user_firstname,
                   CASE
                       WHEN copy_state.availability = 'onHold' THEN assigned_hold.lastname
                   END AS hold_user_lastname
"#;

fn item_sql(where_and_order: &str) -> String {
    let deposit_join = item_deposit_join("i");
    let deposit_json = item_deposit_json("i");
    format!(
        r#"
            SELECT i.id, i.biblio_id, i.site_id, i.barcode, i.lender_barcode, i.call_number, i.volume_designation,
                   i.place, i.borrowable, i.circulation_status, i.public_note, i.internal_note, i.price,
                   i.price_deferred,
                   i.weeding_status, i.weeding_reason,
                   i.created_at, i.updated_at, i.archived_at,
                   so.name as site_name,
                   (active_loan.id IS NOT NULL) as borrowed,
                   active_loan.id as loan_id,
                   active_loan.expiry_at as due_date,
                   (
                       EXISTS (
                           SELECT 1 FROM holds h
                           WHERE h.item_id = i.id
                             AND h.status IN ('to_pick', 'ready')
                       )
                       OR EXISTS (
                           SELECT 1 FROM item_transits t
                           WHERE t.item_id = i.id
                             AND t.hold_id IS NOT NULL
                             AND t.status IN ('requested', 'in_transit')
                       )
                   ) AS on_hold,
                   {ITEM_AVAILABILITY_COLUMNS},
                   {deposit_json} AS deposit
            FROM items i
            LEFT JOIN sites so ON i.site_id = so.id
            {ITEM_STATE_JOINS}
            {deposit_join}
            {where_and_order}
        "#
    )
}

fn item_short_sql(where_and_order: &str) -> String {
    let deposit_join = item_deposit_join("i");
    let deposit_json = item_deposit_json("i");
    format!(
        r#"
            SELECT i.biblio_id, i.id, i.barcode, i.call_number, i.borrowable,
                   i.circulation_status,
                   i.weeding_status,
                   so.name as site_name,
                   (active_loan.id IS NOT NULL) as borrowed,
                   {ITEM_AVAILABILITY_COLUMNS},
                   {deposit_json} AS deposit
            FROM items i
            LEFT JOIN sites so ON i.site_id = so.id
            {ITEM_STATE_JOINS}
            {deposit_join}
            {where_and_order}
        "#
    )
}

impl Repository {
    /// Library-local calendar date of an open loan, for the public catalog only.
    async fn fill_expected_return_dates(&self, items: &mut [Item]) -> AppResult<()> {
        let needs_date = items.iter().any(|item| item.availability == crate::models::item::ItemAvailability::OnLoan && item.due_date.is_some());
        if !needs_date {
            return Ok(());
        }
        let zone = self.library_zone().await?;
        let now = chrono::Utc::now();
        for item in items.iter_mut() {
            item.expected_return_date = if item.availability == crate::models::item::ItemAvailability::OnLoan {
                item.due_date.and_then(|due| crate::services::calendar::public_expected_return_date(zone, due, now))
            } else {
                None
            };
        }
        Ok(())
    }

    /// Get items (physical copies) for a biblio (excludes archived items)
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_get_items(&self, biblio_id: i64) -> AppResult<Vec<Item>> {
        let mut items = sqlx::query_as::<_, Item>(&item_sql("WHERE i.biblio_id = $1 AND i.archived_at IS NULL ORDER BY i.barcode"))
            .bind(biblio_id)
            .fetch_all(&self.pool)
            .await?;
        self.fill_expected_return_dates(&mut items).await?;
        Ok(items)
    }

    /// Get one active item by id (same row shape as [`biblios_get_items`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_active_by_id(&self, item_id: i64) -> AppResult<Item> {
        let mut item = sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1 AND i.archived_at IS NULL"))
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;
        self.fill_expected_return_dates(std::slice::from_mut(&mut item)).await?;
        Ok(item)
    }

    /// Get one active item by barcode (same row shape as [`items_get_active_by_id`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_active_by_barcode(&self, barcode: &str) -> AppResult<Item> {
        let mut item = sqlx::query_as::<_, Item>(&item_sql("WHERE i.barcode = $1 AND i.archived_at IS NULL"))
            .bind(barcode)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item with barcode {barcode} not found")))?;
        self.fill_expected_return_dates(std::slice::from_mut(&mut item)).await?;
        Ok(item)
    }

    /// Get ItemShort for many biblios (excludes archived). Used to attach items to BiblioShort lists.
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_get_items_short_by_biblio_ids(&self, biblio_ids: &[i64]) -> AppResult<HashMap<i64, Vec<ItemShort>>> {
        if biblio_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<ItemShortRow> = sqlx::query_as(&item_short_sql("WHERE i.biblio_id = ANY($1) AND i.archived_at IS NULL ORDER BY i.biblio_id, i.barcode"))
            .bind(biblio_ids)
            .fetch_all(&self.pool)
            .await?;

        let mut map: HashMap<i64, Vec<ItemShort>> = HashMap::new();
        for row in rows {
            map.entry(row.biblio_id).or_default().push(ItemShort::from(row));
        }
        Ok(map)
    }

    /// Create an item (physical copy) for a biblio
    #[tracing::instrument(skip(self), err)]
    pub async fn biblios_create_item(&self, biblio_id: i64, item: &Item) -> AppResult<Item> {
        let now = Utc::now();
        let mut new_item = item.clone();
        let site_id = if let Some(id) = item.site_id {
            Some(id)
        } else if let Some(ref name) = item.site_name {
            Some(self.sites_find_or_create_by_name(name).await?)
        } else if item.borrowable {
            // Incomplete received copies must not inherit the default site.
            match self.sites_get_default().await? {
                Some(default) => Some(default.id),
                None => None,
            }
        } else {
            None
        };
        new_item.site_id = site_id;

        let public_note = note_stored_value(item.public_note.clone());
        let internal_note = note_stored_value(item.internal_note.clone());
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO items (
                biblio_id, barcode, lender_barcode, call_number, volume_designation, place, borrowable,
                public_note, internal_note, price, site_id, deposit_batch_id, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NULLIF(btrim($8), ''), NULLIF(btrim($9), ''), $10, $11, $12, $13, $13)
            RETURNING id
            "#,
        )
        .bind(biblio_id)
        .bind(&item.barcode)
        .bind(&item.lender_barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&public_note)
        .bind(&internal_note)
        .bind(item.price)
        .bind(site_id)
        .bind(item.deposit_batch_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        new_item.id = Some(id);
        new_item.public_note = public_note;
        new_item.internal_note = internal_note;
        Ok(new_item)
    }

    /// Upsert an item (physical copy)
    #[tracing::instrument(skip(self), err)]
    pub async fn upsert_item<'a>(&self, item: &'a mut Item) -> AppResult<&'a mut Item> {
        let now = Utc::now();
        item.updated_at = Some(now);
        let public_note = note_stored_value(item.public_note.clone());
        let internal_note = note_stored_value(item.internal_note.clone());
        item.public_note = public_note.clone();
        item.internal_note = internal_note.clone();
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
                    public_note = NULLIF(btrim($7), ''),
                    internal_note = NULLIF(btrim($8), ''),
                    price = $9,
                    site_id = $10,
                    updated_at = $11,
                    archived_at = $12
                WHERE id = $13
                "#,
            )
            .bind(item.biblio_id)
            .bind(&item.barcode)
            .bind(&item.call_number)
            .bind(&item.volume_designation)
            .bind(item.place)
            .bind(item.borrowable)
            .bind(&public_note)
            .bind(&internal_note)
            .bind(item.price)
            .bind(item.site_id)
            .bind(item.updated_at)
            .bind(item.archived_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        } else {
            if let Some(ref barcode) = item.barcode {
                let existing_id = sqlx::query_scalar::<_, i64>("SELECT id FROM items WHERE barcode = $1 AND archived_at IS NULL")
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
                        public_note = NULLIF(btrim($7), ''),
                        internal_note = NULLIF(btrim($8), ''),
                        price = $9,
                        site_id = $10,
                        updated_at = $11,
                        archived_at = $12
                    WHERE id = $13
                    "#,
                )
                .bind(item.biblio_id)
                .bind(&item.barcode)
                .bind(&item.call_number)
                .bind(&item.volume_designation)
                .bind(item.place)
                .bind(item.borrowable)
                .bind(&public_note)
                .bind(&internal_note)
                .bind(item.price)
                .bind(item.site_id)
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
                        place, borrowable, public_note, internal_note, price, site_id,
                        deposit_batch_id, created_at, updated_at, archived_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6,
                        NULLIF(btrim($7), ''), NULLIF(btrim($8), ''),
                        $9, $10, $11, $12, $12, $13
                    )
                    RETURNING id
                    "#,
                )
                .bind(item.biblio_id)
                .bind(&item.barcode)
                .bind(&item.call_number)
                .bind(&item.volume_designation)
                .bind(item.place)
                .bind(item.borrowable)
                .bind(&public_note)
                .bind(&internal_note)
                .bind(item.price)
                .bind(item.site_id)
                .bind(item.deposit_batch_id)
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
        let public_note = note_write_value(item.public_note.clone());
        let internal_note = note_write_value(item.internal_note.clone());
        item.public_note = public_note.clone().filter(|text| !text.is_empty());
        item.internal_note = internal_note.clone().filter(|text| !text.is_empty());
        sqlx::query(
            r#"
            UPDATE items SET
                barcode = COALESCE($1, barcode),
                call_number = COALESCE($2, call_number),
                volume_designation = COALESCE($3, volume_designation),
                place = COALESCE($4, place),
                borrowable = COALESCE($5, borrowable),
                public_note = CASE
                    WHEN $6::text IS NULL THEN public_note
                    ELSE NULLIF(btrim($6), '')
                END,
                internal_note = CASE
                    WHEN $7::text IS NULL THEN internal_note
                    ELSE NULLIF(btrim($7), '')
                END,
                price = COALESCE($8, price),
                site_id = COALESCE($9, site_id),
                updated_at = $10,
                archived_at = $11
            WHERE id = $12
            "#,
        )
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&public_note)
        .bind(&internal_note)
        .bind(item.price)
        .bind(item.site_id)
        .bind(item.updated_at)
        .bind(item.archived_at)
        .bind(item.id.unwrap_or(0))
        .execute(&self.pool)
        .await?;

        let stored: (Option<String>, Option<String>) = sqlx::query_as("SELECT public_note, internal_note FROM items WHERE id = $1")
            .bind(item.id.unwrap_or(0))
            .fetch_one(&self.pool)
            .await?;
        item.public_note = stored.0;
        item.internal_note = stored.1;

        Ok(item)
    }

    /// Delete an item (physical copy — soft delete, sets `archived_at`).
    ///
    /// **Own stock:** for cataloguing mistakes only. Open circulation, unpaid fines, or past
    /// loan history require weeding or withdrawal instead. `force` does not bypass that.
    ///
    /// **Deposit copies:** allowed while the batch is `active` after explicit confirmation
    /// (`force=true`). The first attempt returns `deposit_item_delete_needs_confirmation`.
    /// Not a substitute for scan-out — batches in `returning` must use the deposit return workflow.
    /// Past loans in `loans_archives` do not block deposit deletes.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_delete(&self, id: i64, force: bool) -> AppResult<()> {
        let now = Utc::now();

        let mut tx = self.pool.begin().await?;

        let locked: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT i.deposit_batch_id, db.status
            FROM items i
            LEFT JOIN deposit_batches db ON db.id = i.deposit_batch_id
            WHERE i.id = $1 AND i.archived_at IS NULL
            FOR UPDATE OF i
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((deposit_batch_id, batch_status)) = locked else {
            return Err(AppError::NotFound(format!("Item {id} not found")));
        };

        if deposit_batch_id.is_some() {
            self.items_delete_deposit_copy_tx(&mut tx, id, batch_status.as_deref(), now, force)
                .await?;
        } else {
            let _ = force;
            self.items_delete_own_stock_tx(&mut tx, id, now).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn items_delete_deposit_copy_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: i64,
        batch_status: Option<&str>,
        now: chrono::DateTime<chrono::Utc>,
        force: bool,
    ) -> AppResult<()> {
        match batch_status {
            Some("returning") => {
                return Err(AppError::coded(
                    422,
                    error_code::DEPOSIT_ITEM_DELETE_USE_BATCH_RETURN,
                    "This copy belongs to a deposit batch that is being returned to the lender. \
                     Use scan-out in the deposit batch to record that it was sent back; \
                     catalog delete is not available during batch return.",
                ));
            }
            Some("closed") => {
                return Err(AppError::coded(
                    422,
                    error_code::DEPOSIT_ITEM_DELETE_USE_BATCH_RETURN,
                    "This copy belongs to a closed deposit batch and is no longer on the active catalog.",
                ));
            }
            Some("active") | None => {}
            Some(other) => {
                return Err(AppError::Internal(format!("Unknown deposit batch status: {other}")));
            }
        }

        let on_loan: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE item_id = $1 AND returned_at IS NULL)",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if on_loan {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_ITEM_ON_LOAN,
                "This deposit copy is still on loan. Check it in at the desk before removing it from the catalog.",
            ));
        }

        if self.item_has_unpaid_fine_tx(tx, id).await? {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_ITEM_DELETE_UNPAID_FINE,
                "This deposit copy has an unpaid fine. Settle or waive the fine before removing it from the catalog.",
            ));
        }

        if !force {
            return Err(AppError::coded(
                409,
                error_code::DEPOSIT_ITEM_DELETE_NEEDS_CONFIRMATION,
                "This copy belongs to a lender deposit. Removing it from the catalog does not send it back to the \
                 lender — use scan-out in the deposit batch for that. Confirm with force=true only to fix a cataloguing \
                 mistake (the copy will count as missing when the batch is closed unless it was scanned out first).",
            ));
        }

        self.holds_release_deposit_copies_tx(tx, &[id]).await?;

        sqlx::query("UPDATE items SET archived_at = $1, updated_at = $1 WHERE id = $2 AND archived_at IS NULL")
            .bind(now)
            .bind(id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn items_delete_own_stock_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: i64,
        now: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        let on_loan: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE item_id = $1 AND returned_at IS NULL)",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if on_loan {
            return Err(AppError::coded(
                422,
                error_code::ITEM_ON_LOAN,
                "This copy is still on loan. Check it in before removing it from the catalog.",
            ));
        }

        let in_transit: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item_transits WHERE item_id = $1 AND status IN ('requested', 'in_transit'))",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if in_transit {
            return Err(AppError::coded(
                422,
                error_code::ITEM_IN_TRANSIT,
                "This copy is in transit. Finish or cancel the transit before removing it from the catalog.",
            ));
        }

        let on_hold: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM holds WHERE item_id = $1 AND status IN ('pending', 'to_pick', 'ready'))",
        )
        .bind(id)
        .fetch_one(&mut **tx)
        .await?;
        if on_hold {
            return Err(AppError::coded(
                422,
                error_code::ITEM_ON_HOLD,
                "This copy is reserved for a reader. Cancel the hold or assign another copy before removing it from the catalog.",
            ));
        }

        if self.item_has_unpaid_fine_tx(tx, id).await? {
            return Err(AppError::coded(
                422,
                error_code::ITEM_DELETE_UNPAID_FINE,
                "This copy has an unpaid fine. Settle or waive the fine before removing it from the catalog.",
            ));
        }

        let has_loan_history: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM loans_archives WHERE item_id = $1)")
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
        if has_loan_history {
            return Err(AppError::coded(
                422,
                error_code::ITEM_HAS_HISTORY_USE_WEEDING,
                "This library-owned copy was loaned before and cannot be deleted from the catalog. \
                 Weed or withdraw it instead to keep a proper disposal record.",
            ));
        }

        sqlx::query("UPDATE items SET archived_at = $1, updated_at = $1 WHERE id = $2 AND archived_at IS NULL")
            .bind(now)
            .bind(id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn item_has_unpaid_fine_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
    ) -> AppResult<bool> {
        let open: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM fines f
                WHERE f.status IN ('pending', 'partial')
                  AND (
                      f.item_id = $1
                      OR EXISTS (SELECT 1 FROM loans l WHERE l.id = f.loan_id AND l.item_id = $1)
                      OR EXISTS (
                          SELECT 1 FROM loans_archives la
                          WHERE la.id = f.loan_id AND la.item_id = $1
                      )
                  )
            )
            "#,
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok(open)
    }

    /// Check if item barcode already exists
    #[tracing::instrument(skip(self), err)]
    pub async fn items_barcode_exists(&self, barcode: &str, exclude_item_id: Option<i64>) -> AppResult<bool> {
        self.items_barcode_exists_active(barcode, exclude_item_id).await
    }

    /// True when a non-archived copy uses this barcode.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_barcode_exists_active(&self, barcode: &str, exclude_item_id: Option<i64>) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_item_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE barcode = $1 AND id != $2 AND archived_at IS NULL)")
                .bind(barcode)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE barcode = $1 AND archived_at IS NULL)")
                .bind(barcode)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists)
    }

    /// Active copy id, or none. Second tuple value is always `false` (kept for callers).
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_by_barcode(&self, barcode: &str) -> AppResult<Option<(i64, bool)>> {
        Ok(self.items_get_active_id_by_barcode(barcode).await?.map(|id| (id, false)))
    }

    /// Get active copy id by barcode.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_get_active_id_by_barcode(&self, barcode: &str) -> AppResult<Option<i64>> {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM items WHERE barcode = $1 AND archived_at IS NULL LIMIT 1")
            .bind(barcode)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    /// Reactivate an archived item and update its fields.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_reactivate(&self, item_id: i64, biblio_id: i64, item: &Item) -> AppResult<Item> {
        if let Some(ref barcode) = item.barcode {
            if self.items_barcode_exists_active(barcode, Some(item_id)).await? {
                return Err(AppError::coded(
                    422,
                    error_code::ITEM_REACTIVATION_BARCODE_TAKEN,
                    "This copy's barcode is already used by an active copy",
                ));
            }
        }
        let now = Utc::now();
        let site_id = if let Some(id) = item.site_id {
            Some(id)
        } else if let Some(ref name) = item.site_name {
            Some(self.sites_find_or_create_by_name(name).await?)
        } else if let Some(default) = self.sites_get_default().await? {
            Some(default.id)
        } else {
            None
        };

        let public_note = note_stored_value(item.public_note.clone());
        let internal_note = note_stored_value(item.internal_note.clone());
        sqlx::query(
            r#"
            UPDATE items SET
                biblio_id = $1, barcode = $2, call_number = $3, volume_designation = $4,
                place = $5, borrowable = $6,
                public_note = NULLIF(btrim($7), ''),
                internal_note = NULLIF(btrim($8), ''),
                price = $9, site_id = $10,
                archived_at = NULL,
                updated_at = $11
            WHERE id = $12
            "#,
        )
        .bind(biblio_id)
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&public_note)
        .bind(&internal_note)
        .bind(item.price)
        .bind(site_id)
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await?;

        let mut item = sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1")).bind(item_id).fetch_one(&self.pool).await?;
        self.fill_expected_return_dates(std::slice::from_mut(&mut item)).await?;
        Ok(item)
    }

    /// Re-attach a returned archived deposit copy to a new batch.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(skip(self, item), err)]
    pub async fn items_reactivate_deposit_copy(&self, item_id: i64, biblio_id: i64, deposit_batch_id: i64, site_id: i64, barcode: &str, lender_barcode: &str, item: &Item) -> AppResult<Item> {
        if self.items_barcode_exists_active(barcode, Some(item_id)).await? {
            return Err(AppError::coded(
                422,
                error_code::ITEM_REACTIVATION_BARCODE_TAKEN,
                "This copy's barcode is already used by an active copy",
            ));
        }
        let now = Utc::now();
        let public_note = note_stored_value(item.public_note.clone());
        let internal_note = note_stored_value(item.internal_note.clone());
        sqlx::query(
            r#"
            UPDATE items SET
                biblio_id = $1,
                barcode = $2,
                lender_barcode = $3,
                call_number = $4,
                volume_designation = $5,
                place = $6,
                borrowable = $7,
                public_note = NULLIF(btrim($8), ''),
                internal_note = NULLIF(btrim($9), ''),
                price = $10,
                site_id = $11,
                deposit_batch_id = $12,
                deposit_returned_at = NULL,
                circulation_status = 0,
                archived_at = NULL,
                updated_at = $13
            WHERE id = $14 AND archived_at IS NOT NULL
            "#,
        )
        .bind(biblio_id)
        .bind(barcode)
        .bind(lender_barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&public_note)
        .bind(&internal_note)
        .bind(item.price)
        .bind(site_id)
        .bind(deposit_batch_id)
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await?;

        let mut item = sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1")).bind(item_id).fetch_one(&self.pool).await?;
        self.fill_expected_return_dates(std::slice::from_mut(&mut item)).await?;
        Ok(item)
    }

    /// Replace fields on an active copy during deposit MARC import (overwrite duplicate).
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(skip(self, item), err)]
    pub async fn items_deposit_overwrite_from_import(&self, item_id: i64, biblio_id: i64, deposit_batch_id: i64, site_id: i64, lender_barcode: &str, item: &Item) -> AppResult<Item> {
        let now = Utc::now();
        let public_note = note_stored_value(item.public_note.clone());
        let internal_note = note_stored_value(item.internal_note.clone());
        sqlx::query(
            r#"
            UPDATE items SET
                biblio_id = $1,
                lender_barcode = $2,
                call_number = $3,
                volume_designation = $4,
                place = $5,
                borrowable = $6,
                public_note = NULLIF(btrim($7), ''),
                internal_note = NULLIF(btrim($8), ''),
                price = $9,
                site_id = $10,
                deposit_batch_id = $11,
                deposit_returned_at = NULL,
                updated_at = $12
            WHERE id = $13 AND archived_at IS NULL
            "#,
        )
        .bind(biblio_id)
        .bind(lender_barcode)
        .bind(&item.call_number)
        .bind(&item.volume_designation)
        .bind(item.place)
        .bind(item.borrowable)
        .bind(&public_note)
        .bind(&internal_note)
        .bind(item.price)
        .bind(site_id)
        .bind(deposit_batch_id)
        .bind(now)
        .bind(item_id)
        .execute(&self.pool)
        .await?;

        let mut item = sqlx::query_as::<_, Item>(&item_sql("WHERE i.id = $1")).bind(item_id).fetch_one(&self.pool).await?;
        self.fill_expected_return_dates(std::slice::from_mut(&mut item)).await?;
        Ok(item)
    }

    /// Find an existing item by barcode and return its short representation.
    #[tracing::instrument(skip(self), err)]
    pub async fn items_find_short_by_barcode(&self, barcode: &str, exclude_item_id: Option<i64>) -> AppResult<Option<ItemShort>> {
        let row: Option<ItemShortRow> = if let Some(eid) = exclude_item_id {
            sqlx::query_as(&item_short_sql("WHERE i.barcode = $1 AND i.id != $2 AND i.archived_at IS NULL LIMIT 1"))
                .bind(barcode)
                .bind(eid)
                .fetch_optional(&self.pool)
                .await?
        } else {
            sqlx::query_as(&item_short_sql("WHERE i.barcode = $1 AND i.archived_at IS NULL LIMIT 1"))
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
    pub async fn items_set_weeding(&self, item_id: i64, status: WeedingStatus, reason: Option<String>) -> AppResult<Item> {
        let reason = reason.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if reason.as_ref().is_some_and(|s| s.chars().count() > 200) {
            return Err(AppError::Validation("Weeding reason must be at most 200 characters".to_string()));
        }

        if status == WeedingStatus::Withdrawn {
            return Err(AppError::coded(422, error_code::WITHDRAWN_VIA_WITHDRAWAL, "Withdraw a copy with POST /api/v1/items/{id}/withdrawals"));
        }

        let mut tx = self.pool.begin().await?;
        let current: Option<(WeedingStatus, Option<i64>)> = sqlx::query_as("SELECT weeding_status, deposit_batch_id FROM items WHERE id = $1 AND archived_at IS NULL FOR UPDATE")
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((current, deposit_batch_id)) = current else {
            return Err(AppError::NotFound(format!("Item {item_id} not found")));
        };
        if deposit_batch_id.is_some() {
            return Err(AppError::coded(422, error_code::DEPOSIT_ITEM_NOT_OWNED, "A deposit copy is not part of the library's own stock"));
        }
        let active_withdrawal: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM item_withdrawals WHERE item_id = $1 AND cancelled_at IS NULL)")
            .bind(item_id)
            .fetch_one(&mut *tx)
            .await?;
        if current == WeedingStatus::Withdrawn || active_withdrawal {
            return Err(AppError::coded(422, error_code::WITHDRAWAL_USE_CANCEL, "Cancel the active withdrawal before changing weeding status"));
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
