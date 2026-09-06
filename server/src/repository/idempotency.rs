//! Persistence for `Idempotency-Key` replay records.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::Repository;
use crate::error::AppResult;

/// Row used while claiming or replaying a key.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IdempotencyRow {
    pub request_hash: String,
    pub status_code: Option<i16>,
    pub response_body: Option<Value>,
}

impl Repository {
    /// Drop an expired record so the same actor+key can be reused after TTL.
    pub async fn idempotency_delete_expired(&self, actor_id: i64, key: &str) -> AppResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM idempotency_keys
            WHERE actor_id = $1
              AND idempotency_key = $2
              AND expires_at <= NOW()
            "#,
        )
        .bind(actor_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Insert a reserved (in-flight) row. Returns `None` if the key is already held.
    pub async fn idempotency_try_reserve(
        &self,
        actor_id: i64,
        key: &str,
        operation: &str,
        request_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> AppResult<Option<IdempotencyRow>> {
        let row = sqlx::query_as::<_, IdempotencyRow>(
            r#"
            INSERT INTO idempotency_keys (
                actor_id, idempotency_key, operation, request_hash, expires_at
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (actor_id, idempotency_key) DO NOTHING
            RETURNING request_hash, status_code, response_body
            "#,
        )
        .bind(actor_id)
        .bind(key)
        .bind(operation)
        .bind(request_hash)
        .bind(expires_at)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn idempotency_get(
        &self,
        actor_id: i64,
        key: &str,
    ) -> AppResult<Option<IdempotencyRow>> {
        let row = sqlx::query_as::<_, IdempotencyRow>(
            r#"
            SELECT request_hash, status_code, response_body
            FROM idempotency_keys
            WHERE actor_id = $1
              AND idempotency_key = $2
              AND expires_at > NOW()
            "#,
        )
        .bind(actor_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn idempotency_store_success(
        &self,
        actor_id: i64,
        key: &str,
        status_code: i16,
        response_body: &Value,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status_code = $3,
                response_body = $4
            WHERE actor_id = $1
              AND idempotency_key = $2
              AND status_code IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(key)
        .bind(status_code)
        .bind(response_body)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Release an in-flight reservation so a later retry can execute.
    pub async fn idempotency_release(&self, actor_id: i64, key: &str) -> AppResult<()> {
        sqlx::query(
            r#"
            DELETE FROM idempotency_keys
            WHERE actor_id = $1
              AND idempotency_key = $2
              AND status_code IS NULL
            "#,
        )
        .bind(actor_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
