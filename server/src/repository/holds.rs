//! Hold domain methods on Repository

use async_trait::async_trait;
use chrono::Utc;
use snowflaked::Generator;
use sqlx::Postgres;

use std::collections::{HashMap, HashSet};

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        biblio::BiblioShort,
        hold::{CreateHold, Hold, HoldDetails, HoldStatus, DEFAULT_MAX_ACTIVE_HOLDS},
        item::ItemShort,
        user::{UserShort, UserShortRow},
    },
};

#[async_trait]
pub trait HoldsRepository: Send + Sync {
    /// All holds, newest first, with total count (for pagination).
    /// When `active_only`, only `pending` and `ready` rows.
    async fn holds_list_all(
        &self,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)>;
    /// Holds for one user (paginated), same ordering/filters as [`HoldsRepository::holds_list_all`].
    async fn holds_list_for_user_paginated(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)>;
    async fn holds_list_for_item(&self, item_id: i64) -> AppResult<Vec<HoldDetails>>;
    async fn holds_list_for_user(&self, user_id: i64) -> AppResult<Vec<HoldDetails>>;
    async fn holds_get_by_id(&self, id: i64) -> AppResult<Hold>;
    async fn holds_create(&self, data: &CreateHold) -> AppResult<Hold>;
    async fn holds_mark_ready(&self, id: i64, expiry_days: i32) -> AppResult<Hold>;
    async fn holds_cancel(&self, id: i64) -> AppResult<Hold>;
    async fn holds_expire_overdue(&self) -> AppResult<Vec<i64>>;
    async fn holds_count_for_item(&self, item_id: i64) -> AppResult<i64>;
    async fn holds_count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64>;
    async fn holds_has_active_for_user_item(&self, user_id: i64, item_id: i64) -> AppResult<bool>;
    async fn holds_get_next_pending(&self, item_id: i64) -> AppResult<Option<Hold>>;
    async fn holds_fulfill(&self, id: i64) -> AppResult<Hold>;
    /// First `pending` hold for the item becomes `ready` with `expires_at` set.
    async fn holds_notify_next(&self, item_id: i64, expiry_days: i32) -> AppResult<Option<Hold>>;
    /// Count of `pending`/`ready` holds for this patron (copy-level).
    async fn holds_count_active_for_user(&self, user_id: i64) -> AppResult<i64>;
    /// Effective cap: public-type override when set, else global default.
    async fn holds_get_max_active_for_user(&self, user_id: i64) -> AppResult<i16>;
    async fn holds_get_global_max_active(&self) -> AppResult<i16>;
    async fn holds_set_global_max_active(&self, max_active_holds: i16) -> AppResult<i16>;
}

#[async_trait::async_trait]
impl HoldsRepository for Repository {
    async fn holds_list_all(
        &self,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        Repository::holds_list_all(self, page, per_page, active_only).await
    }
    async fn holds_list_for_user_paginated(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        Repository::holds_list_for_user_paginated(self, user_id, page, per_page, active_only).await
    }
    async fn holds_list_for_item(&self, item_id: i64) -> AppResult<Vec<HoldDetails>> {
        Repository::holds_list_for_item(self, item_id).await
    }
    async fn holds_list_for_user(&self, user_id: i64) -> AppResult<Vec<HoldDetails>> {
        Repository::holds_list_for_user(self, user_id).await
    }
    async fn holds_get_by_id(&self, id: i64) -> AppResult<Hold> {
        Repository::holds_get_by_id(self, id).await
    }
    async fn holds_create(&self, data: &CreateHold) -> AppResult<Hold> {
        Repository::holds_create(self, data).await
    }
    async fn holds_mark_ready(&self, id: i64, expiry_days: i32) -> AppResult<Hold> {
        Repository::holds_mark_ready(self, id, expiry_days).await
    }
    async fn holds_cancel(&self, id: i64) -> AppResult<Hold> {
        Repository::holds_cancel(self, id).await
    }
    async fn holds_expire_overdue(&self) -> AppResult<Vec<i64>> {
        Repository::holds_expire_overdue(self).await
    }
    async fn holds_count_for_item(&self, item_id: i64) -> AppResult<i64> {
        Repository::holds_count_for_item(self, item_id).await
    }
    async fn holds_count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64> {
        Repository::holds_count_active_for_biblio(self, biblio_id).await
    }
    async fn holds_has_active_for_user_item(&self, user_id: i64, item_id: i64) -> AppResult<bool> {
        Repository::holds_has_active_for_user_item(self, user_id, item_id).await
    }
    async fn holds_get_next_pending(&self, item_id: i64) -> AppResult<Option<Hold>> {
        Repository::holds_get_next_pending(self, item_id).await
    }
    async fn holds_fulfill(&self, id: i64) -> AppResult<Hold> {
        Repository::holds_fulfill(self, id).await
    }
    async fn holds_notify_next(&self, item_id: i64, expiry_days: i32) -> AppResult<Option<Hold>> {
        Repository::holds_notify_next(self, item_id, expiry_days).await
    }
    async fn holds_count_active_for_user(&self, user_id: i64) -> AppResult<i64> {
        Repository::holds_count_active_for_user(self, user_id).await
    }
    async fn holds_get_max_active_for_user(&self, user_id: i64) -> AppResult<i16> {
        Repository::holds_get_max_active_for_user(self, user_id).await
    }
    async fn holds_get_global_max_active(&self) -> AppResult<i16> {
        Repository::holds_get_global_max_active(self).await
    }
    async fn holds_set_global_max_active(&self, max_active_holds: i16) -> AppResult<i16> {
        Repository::holds_set_global_max_active(self, max_active_holds).await
    }
}

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(1)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate::<i64>()
}

impl Repository {
    /// Batch-load `(biblio_id, ItemShort)` per hold `item_id` for list enrichment.
    async fn holds_item_biblio_map(
        &self,
        item_ids: &[i64],
    ) -> AppResult<HashMap<i64, (i64, ItemShort)>> {
        if item_ids.is_empty() {
            return Ok(HashMap::new());
        }
        #[derive(sqlx::FromRow)]
        struct HoldItemShortRow {
            id: i64,
            biblio_id: i64,
            barcode: Option<String>,
            call_number: Option<String>,
            borrowable: bool,
            source_name: Option<String>,
            borrowed: bool,
        }
        let rows: Vec<HoldItemShortRow> = sqlx::query_as(
            r#"
            SELECT it.id, it.biblio_id, it.barcode, it.call_number, it.borrowable,
                   so.name AS source_name,
                   EXISTS(
                       SELECT 1 FROM loans l
                       WHERE l.item_id = it.id AND l.returned_at IS NULL
                   ) AS borrowed
            FROM items it
            LEFT JOIN sources so ON it.source_id = so.id
            WHERE it.id = ANY($1)
            "#,
        )
        .bind(item_ids)
        .fetch_all(&self.pool)
        .await?;
        let mut m = HashMap::with_capacity(rows.len());
        for r in rows {
            let id = r.id;
            let item = ItemShort {
                id: r.id,
                barcode: r.barcode,
                call_number: r.call_number,
                borrowable: r.borrowable,
                source_name: r.source_name,
                borrowed: r.borrowed,
            };
            m.insert(id, (r.biblio_id, item));
        }
        Ok(m)
    }

    /// Batch-load [`UserShort`] for hold list enrichment.
    async fn holds_user_short_map(&self, ids: &[i64]) -> AppResult<HashMap<i64, UserShort>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<UserShortRow> = sqlx::query_as(
            r#"
            SELECT u.id, u.firstname, u.lastname, u.account_type, u.public_type,
                   (SELECT COUNT(*)::bigint FROM loans l WHERE l.user_id = u.id AND l.returned_at IS NULL) AS nb_loans,
                   (SELECT COUNT(*)::bigint FROM loans l WHERE l.user_id = u.id AND l.returned_at IS NULL AND l.expiry_at < NOW()) AS nb_late_loans,
                   u.status, u.created_at, u.expiry_at
            FROM users u
            WHERE u.id = ANY($1)
            "#,
        )
        .bind(ids)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let u = UserShort::from(r);
                (u.id, u)
            })
            .collect())
    }

    /// Expand [`Hold`] rows into [`HoldDetails`] with biblio (single copy) and user snapshots.
    pub async fn holds_holds_to_details(&self, holds: Vec<Hold>) -> AppResult<Vec<HoldDetails>> {
        if holds.is_empty() {
            return Ok(vec![]);
        }
        let item_ids: Vec<i64> = holds
            .iter()
            .map(|h| h.item_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let user_ids: Vec<i64> = holds
            .iter()
            .map(|h| h.user_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let item_biblio_map = self.holds_item_biblio_map(&item_ids).await?;
        let biblio_ids: Vec<i64> = item_biblio_map
            .values()
            .map(|(bid, _)| *bid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let biblio_meta = self
            .biblios_get_short_metadata_map_by_biblio_ids(&biblio_ids)
            .await?;
        let users_map = self.holds_user_short_map(&user_ids).await?;

        let mut out = Vec::with_capacity(holds.len());
        for h in holds {
            let (biblio_id, item_short) = item_biblio_map.get(&h.item_id).ok_or_else(|| {
                AppError::Internal(format!("Item {} not found for hold {}", h.item_id, h.id))
            })?;
            let mut biblio: BiblioShort = biblio_meta.get(biblio_id).cloned().ok_or_else(|| {
                AppError::Internal(format!("Biblio {} not found for hold {}", biblio_id, h.id))
            })?;
            biblio.items = vec![item_short.clone()];
            let user = users_map.get(&h.user_id).cloned();
            out.push(HoldDetails {
                id: h.id,
                biblio,
                user,
                created_at: h.created_at,
                notified_at: h.notified_at,
                expires_at: h.expires_at,
                status: h.status,
                position: h.position,
                notes: h.notes,
            });
        }
        Ok(out)
    }

    /// List every hold row (staff / reporting). Ordered by `created_at` ascending.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_list_all(
        &self,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        let (total, rows) = if active_only {
            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*)::bigint FROM holds WHERE status IN ('pending','ready')",
            )
            .fetch_one(&self.pool)
            .await?;
            let offset = (page - 1).max(0) * per_page;
            let rows = sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE status IN ('pending','ready') ORDER BY created_at ASC LIMIT $1 OFFSET $2")
                .bind(per_page)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
            (total, rows)
        } else {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds")
                .fetch_one(&self.pool)
                .await?;
            let offset = (page - 1).max(0) * per_page;
            let rows = sqlx::query_as::<_, Hold>(
                "SELECT * FROM holds ORDER BY created_at ASC LIMIT $1 OFFSET $2",
            )
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        };
        let details = self.holds_holds_to_details(rows).await?;
        Ok((details, total))
    }

    /// Paginated holds for a single user (same filters/order as [`Repository::holds_list_all`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_list_for_user_paginated(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        let (total, rows) = if active_only {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds WHERE user_id = $1 AND status IN ('pending','ready')")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
            let offset = (page - 1).max(0) * per_page;
            let rows = sqlx::query_as::<_, Hold>(
                "SELECT * FROM holds WHERE user_id = $1 AND status IN ('pending','ready') \
                 ORDER BY created_at ASC LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        } else {
            let total: i64 =
                sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_one(&self.pool)
                    .await?;
            let offset = (page - 1).max(0) * per_page;
            let rows = sqlx::query_as::<_, Hold>(
                "SELECT * FROM holds WHERE user_id = $1 ORDER BY created_at ASC LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        };
        let details = self.holds_holds_to_details(rows).await?;
        Ok((details, total))
    }

    /// First pending hold for this item becomes `ready` (after a loan return frees the copy).
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_notify_next(
        &self,
        item_id: i64,
        expiry_days: i32,
    ) -> AppResult<Option<Hold>> {
        let mut tx = self.pool.begin().await?;
        let next = self
            .holds_notify_next_tx(&mut tx, item_id, expiry_days)
            .await?;
        tx.commit().await?;
        Ok(next)
    }

    /// Same as [`holds_notify_next`] but within an open transaction (atomic with loan return).
    ///
    /// Locks the item's active queue in the same order as checkout so cancel, expire,
    /// return, and borrow serialize. Does not promote if a `ready` hold already exists.
    #[tracing::instrument(skip(self, tx), err)]
    pub async fn holds_notify_next_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
        expiry_days: i32,
    ) -> AppResult<Option<Hold>> {
        let locked: Vec<Hold> = sqlx::query_as(
            r#"
            SELECT * FROM holds
            WHERE item_id = $1 AND status IN ('pending', 'ready')
            ORDER BY CASE status WHEN 'ready' THEN 0 ELSE 1 END, position ASC
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_all(&mut **tx)
        .await?;

        if locked.iter().any(|h| h.status == HoldStatus::Ready) {
            return Ok(None);
        }

        let Some(next) = locked.into_iter().find(|h| h.status == HoldStatus::Pending) else {
            return Ok(None);
        };

        let expires_at = Utc::now() + chrono::Duration::days(expiry_days as i64);
        let updated = sqlx::query_as::<_, Hold>(
            r#"UPDATE holds
               SET status = 'ready', notified_at = NOW(), expires_at = $2
               WHERE id = $1 AND status = 'pending'
               RETURNING *"#,
        )
        .bind(next.id)
        .bind(expires_at)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Pending hold {} not found", next.id)))?;
        Ok(Some(updated))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_list_for_item(&self, item_id: i64) -> AppResult<Vec<HoldDetails>> {
        let rows = sqlx::query_as::<_, Hold>(
            "SELECT * FROM holds WHERE item_id = $1 AND status IN ('pending','ready')
             ORDER BY position ASC",
        )
        .bind(item_id)
        .fetch_all(&self.pool)
        .await?;
        self.holds_holds_to_details(rows).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_list_for_user(&self, user_id: i64) -> AppResult<Vec<HoldDetails>> {
        let rows = sqlx::query_as::<_, Hold>(
            "SELECT * FROM holds WHERE user_id = $1 ORDER BY created_at ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        self.holds_holds_to_details(rows).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_get_by_id(&self, id: i64) -> AppResult<Hold> {
        sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Hold {id} not found")))
    }

    /// Place a hold on a physical copy.
    ///
    /// Serializes on the item row (`FOR UPDATE`) so concurrent inserts cannot
    /// assign the same `MAX(position)+1`. Relies on
    /// `idx_holds_one_active_per_user_item` so the same patron cannot hold the
    /// same copy twice while status is `pending` or `ready`.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_create(&self, data: &CreateHold) -> AppResult<Hold> {
        let id = next_id();
        let mut tx = self.pool.begin().await?;

        // Lock the copy so empty-queue first holds also serialize.
        sqlx::query_scalar::<_, i64>("SELECT id FROM items WHERE id = $1 FOR UPDATE")
            .bind(data.item_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("Item not found".to_string()))?;

        let already_active: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM holds
                WHERE user_id = $1 AND item_id = $2 AND status IN ('pending','ready')
            )
            "#,
        )
        .bind(data.user_id)
        .bind(data.item_id)
        .fetch_one(&mut *tx)
        .await?;
        if already_active {
            return Err(AppError::Conflict(
                "User already has an active hold for this item".to_string(),
            ));
        }

        let row = match sqlx::query_as::<_, Hold>(
            r#"
            INSERT INTO holds (id, user_id, item_id, position, notes)
            VALUES (
                $1, $2, $3,
                COALESCE((SELECT MAX(position) FROM holds
                          WHERE item_id = $3 AND status IN ('pending','ready')), 0) + 1,
                $4
            )
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(data.user_id)
        .bind(data.item_id)
        .bind(&data.notes)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(row) => row,
            Err(e) if is_active_hold_unique_violation(&e) => {
                return Err(AppError::Conflict(
                    "User already has an active hold for this item".to_string(),
                ));
            }
            Err(e) => return Err(e.into()),
        };

        tx.commit().await?;
        Ok(row)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_mark_ready(&self, id: i64, expiry_days: i32) -> AppResult<Hold> {
        let expires_at = Utc::now() + chrono::Duration::days(expiry_days as i64);
        sqlx::query_as::<_, Hold>(
            r#"UPDATE holds
               SET status = 'ready', notified_at = NOW(), expires_at = $2
               WHERE id = $1 AND status = 'pending'
               RETURNING *"#,
        )
        .bind(id)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Pending hold {id} not found")))
    }

    /// Cancel a hold. If it was `ready`, promote the next pending patron in the same transaction.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_cancel(&self, id: i64) -> AppResult<Hold> {
        let mut tx = self.pool.begin().await?;

        let current = sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Hold {id} not found")))?;

        let cancelled = sqlx::query_as::<_, Hold>(
            "UPDATE holds SET status = 'cancelled' WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

        if current.status == HoldStatus::Ready {
            self.holds_notify_next_tx(&mut tx, current.item_id, self.hold_ready_expiry_days())
                .await?;
        }

        tx.commit().await?;
        Ok(cancelled)
    }

    /// Expire overdue `ready` holds and, per affected item, promote the next pending patron.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_expire_overdue(&self) -> AppResult<Vec<i64>> {
        let mut tx = self.pool.begin().await?;

        let expired: Vec<Hold> = sqlx::query_as(
            "UPDATE holds SET status = 'expired'
             WHERE status = 'ready' AND expires_at < NOW()
             RETURNING *",
        )
        .fetch_all(&mut *tx)
        .await?;

        let expiry_days = self.hold_ready_expiry_days();
        let mut notified_items = HashSet::new();
        for hold in &expired {
            if notified_items.insert(hold.item_id) {
                self.holds_notify_next_tx(&mut tx, hold.item_id, expiry_days)
                    .await?;
            }
        }

        tx.commit().await?;
        Ok(expired.into_iter().map(|h| h.id).collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_count_for_item(&self, item_id: i64) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM holds WHERE item_id = $1 AND status IN ('pending','ready')",
        )
        .bind(item_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_get_next_pending(&self, item_id: i64) -> AppResult<Option<Hold>> {
        let row = sqlx::query_as::<_, Hold>(
            "SELECT * FROM holds WHERE item_id = $1 AND status = 'pending'
             ORDER BY position ASC LIMIT 1",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_fulfill(&self, id: i64) -> AppResult<Hold> {
        sqlx::query_as::<_, Hold>("UPDATE holds SET status = 'fulfilled' WHERE id = $1 RETURNING *")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Hold {id} not found")))
    }

    /// Patron allowed to borrow this copy next: `ready` first, else first `pending` by queue position.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_eligible_borrower_for_item(&self, item_id: i64) -> AppResult<Option<i64>> {
        let ready: Option<i64> = sqlx::query_scalar("SELECT user_id FROM holds WHERE item_id = $1 AND status = 'ready' ORDER BY position ASC LIMIT 1")
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?;
        if ready.is_some() {
            return Ok(ready);
        }
        let pending: Option<i64> = sqlx::query_scalar("SELECT user_id FROM holds WHERE item_id = $1 AND status = 'pending' ORDER BY position ASC LIMIT 1")
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(pending)
    }

    /// Whether another patron has a `pending` or `ready` hold on this copy.
    /// Locks active hold rows so renew serializes with checkout and hold updates.
    #[tracing::instrument(skip(self, tx), err)]
    pub async fn holds_has_waiting_patron_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
        borrower_user_id: i64,
    ) -> AppResult<bool> {
        let user_ids: Vec<i64> = sqlx::query_scalar(
            r#"
            SELECT user_id FROM holds
            WHERE item_id = $1 AND status IN ('pending', 'ready')
            ORDER BY CASE status WHEN 'ready' THEN 0 ELSE 1 END, position ASC
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_all(&mut **tx)
        .await?;
        Ok(user_ids.iter().any(|&uid| uid != borrower_user_id))
    }

    /// Same as [`holds_eligible_borrower_for_item`] inside an open transaction.
    /// Locks active hold rows on this copy so checkout and hold updates serialize.
    #[tracing::instrument(skip(self, tx), err)]
    pub async fn holds_eligible_borrower_for_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
    ) -> AppResult<Option<i64>> {
        let user_ids: Vec<i64> = sqlx::query_scalar(
            r#"
            SELECT user_id FROM holds
            WHERE item_id = $1 AND status IN ('pending', 'ready')
            ORDER BY CASE status WHEN 'ready' THEN 0 ELSE 1 END, position ASC
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_all(&mut **tx)
        .await?;
        Ok(user_ids.into_iter().next())
    }

    /// Mark the patron’s active hold on this copy as fulfilled (after a normal checkout).
    #[tracing::instrument(skip(self, tx), err)]
    pub async fn holds_fulfill_active_for_user_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        user_id: i64,
        item_id: i64,
    ) -> AppResult<Option<i64>> {
        let hold_id: Option<i64> = sqlx::query_scalar("UPDATE holds SET status = 'fulfilled' WHERE user_id = $1 AND item_id = $2 AND status IN ('pending','ready') RETURNING id")
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(hold_id)
    }

    /// Cancel every active hold on this copy (used when staff checks out with `force` or removes the item).
    #[tracing::instrument(skip(self, tx), err)]
    pub async fn holds_cancel_active_for_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
    ) -> AppResult<u64> {
        let r = sqlx::query("UPDATE holds SET status = 'cancelled' WHERE item_id = $1 AND status IN ('pending','ready')")
            .bind(item_id)
            .execute(&mut **tx)
            .await?;
        Ok(r.rows_affected())
    }

    /// Cancel active holds on one copy (e.g. item withdrawn from circulation).
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_cancel_active_for_item(&self, item_id: i64) -> AppResult<u64> {
        let r = sqlx::query("UPDATE holds SET status = 'cancelled' WHERE item_id = $1 AND status IN ('pending','ready')")
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }

    /// Whether the user already has a `pending` or `ready` hold on this copy.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_has_active_for_user_item(
        &self,
        user_id: i64,
        item_id: i64,
    ) -> AppResult<bool> {
        let b: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM holds
                WHERE user_id = $1 AND item_id = $2 AND status IN ('pending','ready')
            )
            "#,
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(b)
    }

    /// Count active holds across all copies of a bibliographic record.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM holds h
            INNER JOIN items i ON i.id = h.item_id
            WHERE i.biblio_id = $1 AND h.status IN ('pending','ready')
            "#,
        )
        .bind(biblio_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Count of this patron's `pending`/`ready` holds (copy-level).
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_count_active_for_user(&self, user_id: i64) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM holds WHERE user_id = $1 AND status IN ('pending','ready')",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Effective cap: public-type override when set, else global default.
    #[tracing::instrument(skip(self), err)]
    pub async fn holds_get_max_active_for_user(&self, user_id: i64) -> AppResult<i16> {
        let max: Option<i16> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(
                (
                    SELECT pt.max_active_holds
                    FROM users u
                    JOIN public_types pt ON pt.id = u.public_type
                    WHERE u.id = $1 AND pt.max_active_holds IS NOT NULL
                ),
                (SELECT max_active_holds FROM circulation_settings WHERE id = 1),
                $2
            )
            "#,
        )
        .bind(user_id)
        .bind(DEFAULT_MAX_ACTIVE_HOLDS)
        .fetch_one(&self.pool)
        .await?;
        Ok(max.unwrap_or(DEFAULT_MAX_ACTIVE_HOLDS))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_get_global_max_active(&self) -> AppResult<i16> {
        let max: Option<i16> =
            sqlx::query_scalar("SELECT max_active_holds FROM circulation_settings WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?;
        Ok(max.unwrap_or(DEFAULT_MAX_ACTIVE_HOLDS))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn holds_set_global_max_active(&self, max_active_holds: i16) -> AppResult<i16> {
        let max: i16 = sqlx::query_scalar(
            r#"
            INSERT INTO circulation_settings (id, max_active_holds)
            VALUES (1, $1)
            ON CONFLICT (id) DO UPDATE SET max_active_holds = EXCLUDED.max_active_holds
            RETURNING max_active_holds
            "#,
        )
        .bind(max_active_holds)
        .fetch_one(&self.pool)
        .await?;
        Ok(max)
    }
}

/// `idx_holds_one_active_per_user_item` — last line of defense if two inserts race.
fn is_active_hold_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("23505")
                && db.constraint() == Some("idx_holds_one_active_per_user_item")
        }
        _ => false,
    }
}
