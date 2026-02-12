//! Sources repository

use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::{
    error::{AppError, AppResult},
    models::source::Source,
};

#[derive(Clone)]
pub struct SourcesRepository {
    pool: Pool<Postgres>,
}

impl SourcesRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// List all sources, optionally filtering by archive status
    pub async fn list(&self, include_archived: bool) -> AppResult<Vec<Source>> {
        let rows = if include_archived {
            sqlx::query_as::<_, Source>("SELECT * FROM sources ORDER BY name")
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as::<_, Source>(
                "SELECT * FROM sources WHERE is_archive IS NULL OR is_archive = 0 ORDER BY name",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Get source by ID
    pub async fn get_by_id(&self, id: i32) -> AppResult<Source> {
        sqlx::query_as::<_, Source>("SELECT * FROM sources WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Source {} not found", id)))
    }

    /// Rename a source
    pub async fn rename(&self, id: i32, name: &str) -> AppResult<Source> {
        sqlx::query_as::<_, Source>("UPDATE sources SET name = $1 WHERE id = $2 RETURNING *")
            .bind(name)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Source {} not found", id)))
    }

    /// Update a source (name and/or default status)
    pub async fn update(&self, id: i32, name: Option<&str>, default: Option<bool>) -> AppResult<Source> {
        // If setting this source as default, first unset all other default sources
        if let Some(true) = default {
            sqlx::query(r#"UPDATE sources SET "default" = false WHERE id != $1"#)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }

        // Build update query based on what fields are provided
        let source = match (name, default) {
            (Some(name), Some(default_val)) => {
                sqlx::query_as::<_, Source>(
                    r#"UPDATE sources SET name = $1, "default" = $2 WHERE id = $3 RETURNING *"#
                )
                .bind(name)
                .bind(default_val)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
            }
            (Some(name), None) => {
                sqlx::query_as::<_, Source>(
                    "UPDATE sources SET name = $1 WHERE id = $2 RETURNING *"
                )
                .bind(name)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
            }
            (None, Some(default_val)) => {
                sqlx::query_as::<_, Source>(
                    r#"UPDATE sources SET "default" = $1 WHERE id = $2 RETURNING *"#
                )
                .bind(default_val)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
            }
            (None, None) => {
                return Err(AppError::Validation("At least one field must be provided for update".to_string()));
            }
        };

        source.ok_or_else(|| AppError::NotFound(format!("Source {} not found", id)))
    }

    /// Count non-archived specimens linked to a source
    pub async fn count_active_specimens(&self, source_id: i32) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint FROM specimens
            WHERE source_id = $1
              AND (is_archive IS NULL OR is_archive = 0)
              AND lifecycle_status != 2
            "#,
        )
        .bind(source_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Archive a source
    pub async fn archive(&self, id: i32) -> AppResult<Source> {
        let now = Utc::now();
        sqlx::query_as::<_, Source>(
            "UPDATE sources SET is_archive = 1, archive_date = $1 WHERE id = $2 RETURNING *",
        )
        .bind(now)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Source {} not found", id)))
    }

    /// Create a new source (used during merge)
    pub async fn create(&self, name: &str) -> AppResult<Source> {
        let row = sqlx::query_as::<_, Source>(
            "INSERT INTO sources (name) VALUES ($1) RETURNING *",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Reassign all specimens from given source IDs to a new source ID
    pub async fn reassign_specimens(
        &self,
        old_source_ids: &[i32],
        new_source_id: i32,
    ) -> AppResult<u64> {
        let result = sqlx::query(
            "UPDATE specimens SET source_id = $1 WHERE source_id = ANY($2)",
        )
        .bind(new_source_id)
        .bind(old_source_ids)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Reassign all items from given source IDs to a new source ID
    pub async fn reassign_items(
        &self,
        old_source_ids: &[i32],
        new_source_id: i32,
    ) -> AppResult<u64> {
        let result = sqlx::query(
            "UPDATE items SET source_id = $1 WHERE source_id = ANY($2)",
        )
        .bind(new_source_id)
        .bind(old_source_ids)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Archive multiple sources by IDs
    pub async fn archive_many(&self, ids: &[i32]) -> AppResult<()> {
        let now = Utc::now();
        sqlx::query("UPDATE sources SET is_archive = 1, archive_date = $1 WHERE id = ANY($2)")
            .bind(now)
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get the default source (the one with default = true)
    pub async fn get_default(&self) -> AppResult<Option<Source>> {
        let source = sqlx::query_as::<_, Source>(
            r#"SELECT * FROM sources WHERE "default" = true AND (is_archive IS NULL OR is_archive = 0) LIMIT 1"#
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(source)
    }
}
