//! Sites domain methods on Repository

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Postgres};

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::site::Site,
};

// Note: not `mockall::automock` — trait has `&str` parameters that mockall cannot derive for.
#[async_trait]
pub trait SitesRepository: Send + Sync {
    async fn sites_list(&self, include_archived: bool) -> AppResult<Vec<Site>>;
    async fn sites_get_by_id(&self, id: i64) -> AppResult<Site>;
    async fn sites_rename(&self, id: i64, name: &str) -> AppResult<Site>;
    async fn sites_update(&self, id: i64, name: Option<&str>, default: Option<bool>) -> AppResult<Site>;
    async fn sites_count_active_items(&self, site_id: i64) -> AppResult<i64>;
    async fn sites_archive(&self, id: i64) -> AppResult<Site>;
    async fn sites_create(&self, name: &str, default: Option<bool>) -> AppResult<Site>;
    async fn sites_reassign_items(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64>;
    async fn sites_reassign_biblios(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64>;
    async fn sites_archive_many(&self, ids: &[i64]) -> AppResult<()>;
    async fn sites_find_or_create_by_name(&self, name: &str) -> AppResult<i64>;
    async fn sites_get_default(&self) -> AppResult<Option<Site>>;
    /// Atomically create a merged source, reassign items, and archive old sources.
    async fn merge_sites_tx(&self, name: &str, old_site_ids: &[i64]) -> AppResult<Site>;
    /// Expose the underlying pool so service-level transactions can be initiated.
    fn pool(&self) -> &Pool<Postgres>;
}

#[async_trait::async_trait]
impl SitesRepository for Repository {
    async fn sites_list(&self, include_archived: bool) -> AppResult<Vec<Site>> {
        Repository::sites_list(self, include_archived).await
    }
    async fn sites_get_by_id(&self, id: i64) -> AppResult<Site> {
        Repository::sites_get_by_id(self, id).await
    }
    async fn sites_rename(&self, id: i64, name: &str) -> AppResult<Site> {
        Repository::sites_rename(self, id, name).await
    }
    async fn sites_update(&self, id: i64, name: Option<&str>, default: Option<bool>) -> AppResult<Site> {
        Repository::sites_update(self, id, name, default).await
    }
    async fn sites_count_active_items(&self, site_id: i64) -> AppResult<i64> {
        Repository::sites_count_active_items(self, site_id).await
    }
    async fn sites_archive(&self, id: i64) -> AppResult<Site> {
        Repository::sites_archive(self, id).await
    }
    async fn sites_create(&self, name: &str, default: Option<bool>) -> AppResult<Site> {
        Repository::sites_create(self, name, default).await
    }
    async fn sites_reassign_items(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64> {
        Repository::sites_reassign_items(self, old_site_ids, new_site_id).await
    }
    async fn sites_reassign_biblios(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64> {
        Repository::sites_reassign_biblios(self, old_site_ids, new_site_id).await
    }
    async fn sites_archive_many(&self, ids: &[i64]) -> AppResult<()> {
        Repository::sites_archive_many(self, ids).await
    }
    async fn sites_find_or_create_by_name(&self, name: &str) -> AppResult<i64> {
        Repository::sites_find_or_create_by_name(self, name).await
    }
    async fn sites_get_default(&self) -> AppResult<Option<Site>> {
        Repository::sites_get_default(self).await
    }
    async fn merge_sites_tx(&self, name: &str, old_site_ids: &[i64]) -> AppResult<Site> {
        Repository::merge_sites_tx(self, name, old_site_ids).await
    }
    fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }
}

impl Repository {
    /// List all sources, optionally filtering by archive status
    pub async fn sites_list(&self, include_archived: bool) -> AppResult<Vec<Site>> {
        let rows = if include_archived {
            sqlx::query_as::<_, Site>("SELECT * FROM sites ORDER BY name").fetch_all(&self.pool).await?
        } else {
            sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE is_archive IS NULL OR is_archive = 0 ORDER BY name")
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows)
    }

    /// Get source by ID
    pub async fn sites_get_by_id(&self, id: i64) -> AppResult<Site> {
        sqlx::query_as::<_, Site>("SELECT * FROM sites WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Site {} not found", id)))
    }

    /// Rename a source
    pub async fn sites_rename(&self, id: i64, name: &str) -> AppResult<Site> {
        sqlx::query_as::<_, Site>("UPDATE sites SET name = $1 WHERE id = $2 RETURNING *")
            .bind(name)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Site {} not found", id)))
    }

    /// Update a source (name and/or default status).
    ///
    /// When changing the default flag, both the clear-all and the target update are wrapped
    /// in a transaction to keep the invariant that exactly one source is default at all times.
    pub async fn sites_update(&self, id: i64, name: Option<&str>, default: Option<bool>) -> AppResult<Site> {
        if let Some(true) = default {
            let mut tx = self.pool.begin().await?;

            sqlx::query(r#"UPDATE sites SET "default" = false WHERE id != $1"#).bind(id).execute(&mut *tx).await?;

            let source = match name {
                Some(n) => {
                    sqlx::query_as::<_, Site>(r#"UPDATE sites SET name = $1, "default" = true WHERE id = $2 RETURNING *"#)
                        .bind(n)
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await?
                }
                None => {
                    sqlx::query_as::<_, Site>(r#"UPDATE sites SET "default" = true WHERE id = $1 RETURNING *"#)
                        .bind(id)
                        .fetch_optional(&mut *tx)
                        .await?
                }
            };

            tx.commit().await?;
            return source.ok_or_else(|| AppError::NotFound(format!("Site {} not found", id)));
        }

        // No default change — simple update without a transaction.
        let source = match (name, default) {
            (Some(n), Some(dv)) => {
                sqlx::query_as::<_, Site>(r#"UPDATE sites SET name = $1, "default" = $2 WHERE id = $3 RETURNING *"#)
                    .bind(n)
                    .bind(dv)
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            (Some(n), None) => {
                sqlx::query_as::<_, Site>("UPDATE sites SET name = $1 WHERE id = $2 RETURNING *")
                    .bind(n)
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            (None, Some(dv)) => {
                sqlx::query_as::<_, Site>(r#"UPDATE sites SET "default" = $1 WHERE id = $2 RETURNING *"#)
                    .bind(dv)
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await?
            }
            (None, None) => {
                return Err(AppError::Validation("At least one field must be provided for update".to_string()));
            }
        };

        source.ok_or_else(|| AppError::NotFound(format!("Site {} not found", id)))
    }

    /// Count non-archived items (physical copies) linked to a source
    pub async fn sites_count_active_items(&self, site_id: i64) -> AppResult<i64> {
        self.biblios_count_items_for_site(site_id).await
    }

    /// Archive a source
    pub async fn sites_archive(&self, id: i64) -> AppResult<Site> {
        let now = Utc::now();
        sqlx::query_as::<_, Site>("UPDATE sites SET is_archive = 1, archived_at = $1 WHERE id = $2 RETURNING *")
            .bind(now)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Site {} not found", id)))
    }

    /// Create a new source.
    ///
    /// When `default = Some(true)`, the UPDATE-all + INSERT are wrapped in a transaction
    /// so the default flag is never lost if the INSERT fails.
    pub async fn sites_create(&self, name: &str, default: Option<bool>) -> AppResult<Site> {
        if default == Some(true) {
            let mut tx = self.pool.begin().await?;

            sqlx::query(r#"UPDATE sites SET "default" = false"#).execute(&mut *tx).await?;

            let row = sqlx::query_as::<_, Site>(r#"INSERT INTO sites (name, "default") VALUES ($1, true) RETURNING *"#)
                .bind(name)
                .fetch_one(&mut *tx)
                .await?;

            tx.commit().await?;
            return Ok(row);
        }

        let row = sqlx::query_as::<_, Site>(r#"INSERT INTO sites (name, "default") VALUES ($1, false) RETURNING *"#)
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Reassign all physical items from given source IDs to a new source ID
    pub async fn sites_reassign_items(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64> {
        self.biblios_reassign_items_site(old_site_ids, new_site_id).await
    }

    /// Reassign all biblios from given source IDs to a new source ID (no-op: sources attach to items)
    pub async fn sites_reassign_biblios(&self, old_site_ids: &[i64], new_site_id: i64) -> AppResult<i64> {
        self.biblios_reassign_biblios_site(old_site_ids, new_site_id).await
    }

    /// Archive multiple sources by IDs
    pub async fn sites_archive_many(&self, ids: &[i64]) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("UPDATE sites SET is_archive = 1, archived_at = $1 WHERE id = ANY($2)")
            .bind(now)
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Find source by name or create it. Returns source id.
    pub async fn sites_find_or_create_by_name(&self, name: &str) -> AppResult<i64> {
        if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM sites WHERE name = $1").bind(name).fetch_optional(&self.pool).await? {
            return Ok(id);
        }
        let id: i64 = sqlx::query_scalar(r#"INSERT INTO sites (name) VALUES ($1) RETURNING id"#).bind(name).fetch_one(&self.pool).await?;
        Ok(id)
    }

    /// Get the default source (the one with default = true)
    pub async fn sites_get_default(&self) -> AppResult<Option<Site>> {
        let source = sqlx::query_as::<_, Site>(r#"SELECT * FROM sites WHERE "default" = true AND (is_archive IS NULL OR is_archive = 0) LIMIT 1"#)
            .fetch_optional(&self.pool)
            .await?;
        Ok(source)
    }

    /// Atomically merge sources: create target, reassign items, archive sources.
    pub async fn merge_sites_tx(&self, name: &str, old_site_ids: &[i64]) -> AppResult<Site> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        let new_site = sqlx::query_as::<_, Site>(r#"INSERT INTO sites (name, "default") VALUES ($1, false) RETURNING *"#)
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;

        sqlx::query("UPDATE items SET site_id = $1 WHERE site_id = ANY($2)")
            .bind(new_site.id)
            .bind(old_site_ids)
            .execute(&mut *tx)
            .await?;

        sqlx::query("UPDATE sites SET is_archive = 1, archived_at = $1 WHERE id = ANY($2)")
            .bind(now)
            .bind(old_site_ids)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(new_site)
    }
}
