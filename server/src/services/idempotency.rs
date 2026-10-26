//! Shared `Idempotency-Key` contract for desk mutations (checkout / renew).
//!
//! - Header: `Idempotency-Key` (optional). Valid value: 1–255 chars in `[A-Za-z0-9._~-]`.
//! - Scope: `(actor_id, key)` — the authenticated user who sent the header.
//! - TTL: 24 hours. After expiry the key may be reused.
//! - Replay: same key + same request fingerprint returns the stored 2xx body and status.
//! - Conflict: same key + different fingerprint → `409 conflict`.
//! - In-flight: a second identical request waits briefly for the first to finish, then
//!   replays; if it is still running, `409 conflict`.
//! - Failures (4xx/5xx) are not stored; the reservation is released so the client can retry.
//!
//! This does **not** replace SQL uniqueness on loans. It only makes client retries safe.

use std::future::Future;
use std::time::Duration;

use axum::http::StatusCode;
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::repository::Repository;

/// HTTP header name (case-insensitive on the wire).
pub const HEADER_NAME: &str = "Idempotency-Key";

/// How long a stored outcome is replayed for the same actor+key.
pub const TTL: chrono::Duration = chrono::Duration::hours(24);

const KEY_MAX_LEN: usize = 255;
const IN_FLIGHT_WAIT: Duration = Duration::from_secs(8);
const IN_FLIGHT_POLL: Duration = Duration::from_millis(50);

/// Validate a raw header value. Empty / missing is handled by the caller.
pub fn validate_key(raw: &str) -> AppResult<String> {
    let key = raw.trim();
    if key.is_empty() || key.len() > KEY_MAX_LEN {
        return Err(AppError::Validation(format!(
            "{HEADER_NAME} must be 1–{KEY_MAX_LEN} characters"
        )));
    }
    if !key
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
    {
        return Err(AppError::Validation(format!(
            "{HEADER_NAME} may only contain A–Z, a–z, 0–9, '-', '_', '.', '~'"
        )));
    }
    Ok(key.to_string())
}

pub fn request_hash(fingerprint: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(fingerprint).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

enum Claim {
    Reserved,
    Replay {
        status: StatusCode,
        body: serde_json::Value,
    },
}

/// Run `f` unless this actor already completed the same key+payload within TTL.
pub async fn execute<T, F, Fut>(
    repo: &Repository,
    actor_id: i64,
    raw_key: Option<&str>,
    operation: &str,
    fingerprint: &serde_json::Value,
    f: F,
) -> AppResult<(StatusCode, T)>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = AppResult<(StatusCode, T)>>,
{
    let Some(raw_key) = raw_key else {
        return f().await;
    };
    let key = validate_key(raw_key)?;
    let hash = request_hash(fingerprint);

    match claim(repo, actor_id, &key, operation, &hash).await? {
        Claim::Replay { status, body } => {
            let value = serde_json::from_value(body).map_err(|e| {
                AppError::Internal(format!("stored idempotency response is invalid: {e}"))
            })?;
            Ok((status, value))
        }
        Claim::Reserved => match f().await {
            Ok((status, body)) => {
                if let Err(e) = persist_success(repo, actor_id, &key, status, &body).await {
                    tracing::error!(error = %e, "failed to persist idempotency response");
                }
                Ok((status, body))
            }
            Err(e) => {
                if let Err(release_err) = repo.idempotency_release(actor_id, &key).await {
                    tracing::error!(error = %release_err, "failed to release idempotency key");
                }
                Err(e)
            }
        },
    }
}

async fn persist_success<T: Serialize>(
    repo: &Repository,
    actor_id: i64,
    key: &str,
    status: StatusCode,
    body: &T,
) -> AppResult<()> {
    let json = serde_json::to_value(body)
        .map_err(|e| AppError::Internal(format!("serialize idempotency response: {e}")))?;
    repo.idempotency_store_success(actor_id, key, status.as_u16() as i16, &json)
        .await
}

async fn claim(
    repo: &Repository,
    actor_id: i64,
    key: &str,
    operation: &str,
    request_hash: &str,
) -> AppResult<Claim> {
    let deadline = tokio::time::Instant::now() + IN_FLIGHT_WAIT;
    loop {
        repo.idempotency_delete_expired(actor_id, key).await?;

        if repo
            .idempotency_try_reserve(actor_id, key, operation, request_hash, Utc::now() + TTL)
            .await?
            .is_some()
        {
            return Ok(Claim::Reserved);
        }

        let Some(existing) = repo.idempotency_get(actor_id, key).await? else {
            // Released between conflict and select — try again immediately.
            continue;
        };

        if existing.request_hash != request_hash {
            return Err(AppError::Conflict(format!(
                "{HEADER_NAME} already used with a different request"
            )));
        }

        if let (Some(code), Some(body)) = (existing.status_code, existing.response_body) {
            let status = StatusCode::from_u16(code as u16).unwrap_or(StatusCode::OK);
            return Ok(Claim::Replay { status, body });
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Conflict(format!(
                "{HEADER_NAME} request is already in progress"
            )));
        }
        tokio::time::sleep(IN_FLIGHT_POLL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_key_accepts_uuid() {
        let key = validate_key("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(key, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn validate_key_rejects_spaces() {
        let err = validate_key("not a key").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_key_rejects_empty() {
        let err = validate_key("   ").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn request_hash_is_stable() {
        let a = serde_json::json!({"op":"loans.create","userId":"1","force":false});
        let b = serde_json::json!({"op":"loans.create","userId":"1","force":false});
        assert_eq!(request_hash(&a), request_hash(&b));
        assert_eq!(request_hash(&a).len(), 64);
    }
}
