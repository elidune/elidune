//! Item transit domain methods on Repository.

use async_trait::async_trait;
use snowflaked::Generator;
use sqlx::Postgres;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        hold::{Hold, HoldStatus},
        transit::{CreateTransit, ItemTransit, TransitStatus},
    },
};

#[async_trait]
pub trait TransitsRepository: Send + Sync {
    async fn transits_get_by_id(&self, id: i64) -> AppResult<ItemTransit>;
    async fn transits_get_active_for_item(&self, item_id: i64) -> AppResult<Option<ItemTransit>>;
    async fn transits_get_active_for_hold(&self, hold_id: i64) -> AppResult<Option<ItemTransit>>;
    async fn transits_list(
        &self,
        status: Option<TransitStatus>,
        item_id: Option<i64>,
        hold_id: Option<i64>,
        to_source_id: Option<i64>,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ItemTransit>, i64)>;
}

#[async_trait::async_trait]
impl TransitsRepository for Repository {
    async fn transits_get_by_id(&self, id: i64) -> AppResult<ItemTransit> {
        Repository::transits_get_by_id(self, id).await
    }
    async fn transits_get_active_for_item(&self, item_id: i64) -> AppResult<Option<ItemTransit>> {
        Repository::transits_get_active_for_item(self, item_id).await
    }
    async fn transits_get_active_for_hold(&self, hold_id: i64) -> AppResult<Option<ItemTransit>> {
        Repository::transits_get_active_for_hold(self, hold_id).await
    }
    async fn transits_list(
        &self,
        status: Option<TransitStatus>,
        item_id: Option<i64>,
        hold_id: Option<i64>,
        to_source_id: Option<i64>,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ItemTransit>, i64)> {
        Repository::transits_list(self, status, item_id, hold_id, to_source_id, page, per_page)
            .await
    }
}

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(2)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate::<i64>()
}

#[derive(sqlx::FromRow)]
struct ItemSiteRow {
    biblio_id: i64,
    source_id: Option<i64>,
    on_loan: bool,
}

impl Repository {
    #[tracing::instrument(skip(self), err)]
    pub async fn transits_get_by_id(&self, id: i64) -> AppResult<ItemTransit> {
        sqlx::query_as::<_, ItemTransit>("SELECT * FROM item_transits WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Transit {id} not found")))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn transits_get_active_for_item(
        &self,
        item_id: i64,
    ) -> AppResult<Option<ItemTransit>> {
        sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE item_id = $1 AND status IN ('requested', 'in_transit')
            "#,
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn transits_get_active_for_hold(
        &self,
        hold_id: i64,
    ) -> AppResult<Option<ItemTransit>> {
        sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE hold_id = $1 AND status IN ('requested', 'in_transit')
            "#,
        )
        .bind(hold_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn transits_list(
        &self,
        status: Option<TransitStatus>,
        item_id: Option<i64>,
        hold_id: Option<i64>,
        to_source_id: Option<i64>,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<ItemTransit>, i64)> {
        let offset = (page - 1).max(0) * per_page;
        let status_filter = status.map(|s| s.as_str().to_string());
        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM item_transits
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::bigint IS NULL OR item_id = $2)
              AND ($3::bigint IS NULL OR hold_id = $3)
              AND ($4::bigint IS NULL OR to_source_id = $4)
            "#,
        )
        .bind(&status_filter)
        .bind(item_id)
        .bind(hold_id)
        .bind(to_source_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE ($1::text IS NULL OR status = $1)
              AND ($2::bigint IS NULL OR item_id = $2)
              AND ($3::bigint IS NULL OR hold_id = $3)
              AND ($4::bigint IS NULL OR to_source_id = $4)
            ORDER BY created_at DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(&status_filter)
        .bind(item_id)
        .bind(hold_id)
        .bind(to_source_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok((rows, total))
    }

    pub async fn transits_active_for_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
    ) -> AppResult<Option<ItemTransit>> {
        sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE item_id = $1 AND status IN ('requested', 'in_transit')
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(Into::into)
    }

    pub async fn transits_cancel_active_for_item_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
        cancelled_by: Option<i64>,
    ) -> AppResult<u64> {
        let r = sqlx::query(
            r#"
            UPDATE item_transits
            SET status = 'cancelled', cancelled_at = NOW(), cancelled_by = $2
            WHERE item_id = $1 AND status IN ('requested', 'in_transit')
            "#,
        )
        .bind(item_id)
        .bind(cancelled_by)
        .execute(&mut **tx)
        .await?;
        Ok(r.rows_affected())
    }

    /// Staff-driven request (and optional ship) of a copy toward a hold's pickup site.
    #[tracing::instrument(skip(self), err)]
    pub async fn transits_request(
        &self,
        hold_id: i64,
        data: &CreateTransit,
        actor_id: i64,
    ) -> AppResult<(ItemTransit, Hold)> {
        let mut tx = self.pool.begin().await?;
        let hold = sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE id = $1 FOR UPDATE")
            .bind(hold_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Hold {hold_id} not found")))?;

        if !matches!(hold.status, HoldStatus::Pending) {
            return Err(AppError::BusinessRule(
                "Transit can only start from a pending hold".to_string(),
            ));
        }

        let existing = sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE hold_id = $1 AND status IN ('requested', 'in_transit')
            FOR UPDATE
            "#,
        )
        .bind(hold_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            return Err(AppError::Conflict(format!(
                "Hold already has an active transit ({})",
                existing.status.as_str()
            )));
        }

        let item_id = data.item_id.or(hold.item_id).ok_or_else(|| {
            AppError::Validation(
                "itemId is required to start transit for a title-level hold".into(),
            )
        })?;

        let item = Self::transits_lock_item(&mut tx, item_id).await?;
        if item.biblio_id != hold.biblio_id {
            return Err(AppError::Validation(
                "itemId does not belong to this hold's bibliographic record".to_string(),
            ));
        }
        if item.on_loan {
            return Err(AppError::BusinessRule(
                "Item is currently on loan and cannot be placed in transit".to_string(),
            ));
        }

        let item_active = sqlx::query_as::<_, ItemTransit>(
            r#"
            SELECT * FROM item_transits
            WHERE item_id = $1 AND status IN ('requested', 'in_transit')
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;
        if item_active.is_some() {
            return Err(AppError::Conflict(
                "Item already has an active transit".to_string(),
            ));
        }

        let from_source_id = data.from_source_id.or(item.source_id).ok_or_else(|| {
            AppError::Validation(
                "fromSourceId is required when the item has no current site".into(),
            )
        })?;
        let to_source_id = data.to_source_id.or(hold.pickup_site_id).ok_or_else(|| {
            AppError::Validation(
                "toSourceId or hold pickupSiteId is required to start transit".into(),
            )
        })?;
        self.transits_ensure_source(&mut tx, from_source_id).await?;
        self.transits_ensure_source(&mut tx, to_source_id).await?;
        if from_source_id == to_source_id {
            return Err(AppError::BusinessRule(
                "Pickup site matches the item's current site — mark the hold ready instead of transiting".to_string(),
            ));
        }

        if hold.item_id.is_none() {
            sqlx::query("UPDATE holds SET item_id = $1, pickup_site_id = COALESCE(pickup_site_id, $2) WHERE id = $3")
                .bind(item_id)
                .bind(to_source_id)
                .bind(hold_id)
                .execute(&mut *tx)
                .await?;
        } else if hold.pickup_site_id.is_none() {
            sqlx::query("UPDATE holds SET pickup_site_id = $1 WHERE id = $2")
                .bind(to_source_id)
                .bind(hold_id)
                .execute(&mut *tx)
                .await?;
        }

        let transit = self
            .transits_insert_tx(
                &mut tx,
                item_id,
                Some(hold_id),
                from_source_id,
                to_source_id,
                if data.ship {
                    TransitStatus::InTransit
                } else {
                    TransitStatus::Requested
                },
                data.notes.as_deref(),
                if data.ship { Some(actor_id) } else { None },
                None,
            )
            .await?;

        let hold = sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE id = $1")
            .bind(hold_id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok((transit, hold))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn transits_ship(
        &self,
        transit_id: i64,
        actor_id: i64,
        notes: Option<&str>,
    ) -> AppResult<ItemTransit> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, ItemTransit>(
            "SELECT * FROM item_transits WHERE id = $1 FOR UPDATE",
        )
        .bind(transit_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Transit {transit_id} not found")))?;

        if current.status != TransitStatus::Requested {
            return Err(AppError::BusinessRule(format!(
                "Transit must be requested to ship (current: {})",
                current.status.as_str()
            )));
        }

        let item = Self::transits_lock_item(&mut tx, current.item_id).await?;
        if item.on_loan {
            return Err(AppError::BusinessRule(
                "Item is currently on loan and cannot be shipped".to_string(),
            ));
        }

        let notes = notes.or(current.notes.as_deref());
        let transit = sqlx::query_as::<_, ItemTransit>(
            r#"
            UPDATE item_transits
            SET status = 'in_transit',
                shipped_at = NOW(),
                shipped_by = $2,
                notes = COALESCE($3, notes)
            WHERE id = $1 AND status = 'requested'
            RETURNING *
            "#,
        )
        .bind(transit_id)
        .bind(actor_id)
        .bind(notes)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Conflict("Transit was updated concurrently".to_string()))?;

        tx.commit().await?;
        Ok(transit)
    }

    /// Receive at the pickup site: move the copy, mark the linked hold ready.
    #[tracing::instrument(skip(self), err)]
    pub async fn transits_receive(
        &self,
        transit_id: i64,
        actor_id: i64,
        notes: Option<&str>,
    ) -> AppResult<(ItemTransit, Option<Hold>)> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, ItemTransit>(
            "SELECT * FROM item_transits WHERE id = $1 FOR UPDATE",
        )
        .bind(transit_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Transit {transit_id} not found")))?;

        if current.status != TransitStatus::InTransit {
            return Err(AppError::BusinessRule(format!(
                "Transit must be in transit to receive (current: {})",
                current.status.as_str()
            )));
        }

        let _item = Self::transits_lock_item(&mut tx, current.item_id).await?;

        sqlx::query("UPDATE items SET source_id = $1, updated_at = NOW() WHERE id = $2")
            .bind(current.to_source_id)
            .bind(current.item_id)
            .execute(&mut *tx)
            .await?;

        let notes = notes.or(current.notes.as_deref());
        let transit = sqlx::query_as::<_, ItemTransit>(
            r#"
            UPDATE item_transits
            SET status = 'received',
                received_at = NOW(),
                received_by = $2,
                notes = COALESCE($3, notes)
            WHERE id = $1 AND status = 'in_transit'
            RETURNING *
            "#,
        )
        .bind(transit_id)
        .bind(actor_id)
        .bind(notes)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Conflict("Transit was updated concurrently".to_string()))?;

        let hold = if let Some(hold_id) = current.hold_id {
            let hold = sqlx::query_as::<_, Hold>("SELECT * FROM holds WHERE id = $1 FOR UPDATE")
                .bind(hold_id)
                .fetch_optional(&mut *tx)
                .await?;
            match hold {
                Some(h) if h.status == HoldStatus::Pending => {
                    let expiry_days = self.hold_ready_expiry_days();
                    let ready = sqlx::query_as::<_, Hold>(
                        r#"
                        UPDATE holds
                        SET item_id = COALESCE(item_id, $2),
                            status = 'ready',
                            notified_at = NOW(),
                            expires_at = NOW() + ($3::int * INTERVAL '1 day')
                        WHERE id = $1 AND status = 'pending'
                        RETURNING *
                        "#,
                    )
                    .bind(hold_id)
                    .bind(current.item_id)
                    .bind(expiry_days)
                    .fetch_optional(&mut *tx)
                    .await?;
                    ready
                }
                other => other,
            }
        } else {
            None
        };

        tx.commit().await?;
        Ok((transit, hold))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn transits_cancel(
        &self,
        transit_id: i64,
        actor_id: i64,
        reverse: bool,
        notes: Option<&str>,
    ) -> AppResult<(ItemTransit, Option<ItemTransit>)> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query_as::<_, ItemTransit>(
            "SELECT * FROM item_transits WHERE id = $1 FOR UPDATE",
        )
        .bind(transit_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Transit {transit_id} not found")))?;

        if !current.status.is_active() {
            return Err(AppError::BusinessRule(format!(
                "Transit is not active (current: {})",
                current.status.as_str()
            )));
        }

        let notes = notes.or(current.notes.as_deref());
        let cancelled = sqlx::query_as::<_, ItemTransit>(
            r#"
            UPDATE item_transits
            SET status = 'cancelled',
                cancelled_at = NOW(),
                cancelled_by = $2,
                notes = COALESCE($3, notes)
            WHERE id = $1 AND status IN ('requested', 'in_transit')
            RETURNING *
            "#,
        )
        .bind(transit_id)
        .bind(actor_id)
        .bind(notes)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::Conflict("Transit was updated concurrently".to_string()))?;

        let reverse_row = if reverse && current.status == TransitStatus::InTransit {
            Some(
                self.transits_insert_tx(
                    &mut tx,
                    current.item_id,
                    None,
                    current.to_source_id,
                    current.from_source_id,
                    TransitStatus::Requested,
                    Some("Reverse transit after cancel"),
                    None,
                    Some(current.id),
                )
                .await?,
            )
        } else {
            None
        };

        tx.commit().await?;
        Ok((cancelled, reverse_row))
    }

    /// Allocate a copy to the next hold, or start transit when the pickup site differs.
    ///
    /// Returns the hold only when it became `ready` (same-site). Cross-site
    /// allocation creates a `requested` transit and returns `None`.
    pub async fn transits_maybe_request_for_notify_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        hold: &Hold,
        item_id: i64,
        item_source_id: Option<i64>,
    ) -> AppResult<bool> {
        let Some(pickup) = hold.pickup_site_id else {
            return Ok(false);
        };
        let Some(current) = item_source_id else {
            return Ok(false);
        };
        if pickup == current {
            return Ok(false);
        }

        let existing: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM item_transits
                WHERE item_id = $1 AND status IN ('requested', 'in_transit')
            )
            "#,
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        if existing {
            return Ok(true);
        }

        self.transits_insert_tx(
            tx,
            item_id,
            Some(hold.id),
            current,
            pickup,
            TransitStatus::Requested,
            Some("Auto-requested after copy became available"),
            None,
            None,
        )
        .await?;
        Ok(true)
    }

    pub(crate) async fn transits_insert_for_hold_cancel_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        transit: &ItemTransit,
    ) -> AppResult<ItemTransit> {
        self.transits_insert_tx(
            tx,
            transit.item_id,
            None,
            transit.to_source_id,
            transit.from_source_id,
            TransitStatus::Requested,
            Some("Reverse transit after hold cancel"),
            None,
            Some(transit.id),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn transits_insert_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
        hold_id: Option<i64>,
        from_source_id: i64,
        to_source_id: i64,
        status: TransitStatus,
        notes: Option<&str>,
        shipped_by: Option<i64>,
        reversed_from_id: Option<i64>,
    ) -> AppResult<ItemTransit> {
        let id = next_id();
        let shipped_at = if status == TransitStatus::InTransit {
            Some(chrono::Utc::now())
        } else {
            None
        };
        sqlx::query_as::<_, ItemTransit>(
            r#"
            INSERT INTO item_transits (
                id, item_id, hold_id, from_source_id, to_source_id, status, notes,
                shipped_at, shipped_by, reversed_from_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(item_id)
        .bind(hold_id)
        .bind(from_source_id)
        .bind(to_source_id)
        .bind(status.as_str())
        .bind(notes)
        .bind(shipped_at)
        .bind(shipped_by)
        .bind(reversed_from_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(Into::into)
    }

    async fn transits_ensure_source(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        source_id: i64,
    ) -> AppResult<()> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM sources WHERE id = $1 AND COALESCE(is_archive, 0) = 0",
        )
        .bind(source_id)
        .fetch_optional(&mut **tx)
        .await?;
        if found.is_none() {
            return Err(AppError::Validation(format!(
                "Pickup site {source_id} not found or archived"
            )));
        }
        Ok(())
    }

    async fn transits_lock_item(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        item_id: i64,
    ) -> AppResult<ItemSiteRow> {
        sqlx::query_as::<_, ItemSiteRow>(
            r#"
            SELECT it.biblio_id, it.source_id,
                   EXISTS(
                       SELECT 1 FROM loans l
                       WHERE l.item_id = it.id AND l.returned_at IS NULL
                   ) AS on_loan
            FROM items it
            WHERE it.id = $1 AND it.archived_at IS NULL
            FOR UPDATE OF it
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Item not found".to_string()))
    }
}
