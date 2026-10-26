//! Serials API: frequencies, numbering patterns, and the prediction preview.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::AppResult,
    models::serial::{
        FrequencyWrite, NumberingPatternWrite, PredictionResponse, SerialFrequency,
        SerialFrequencyListResponse, SerialListQuery, SerialNumberingPattern,
        SerialNumberingPatternListResponse, TestPredictionRequest,
    },
    services::audit,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

/// Build the serials routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route(
            "/serials/frequencies",
            get(list_frequencies).post(create_frequency),
        )
        .route(
            "/serials/frequencies/:id",
            get(get_frequency)
                .put(update_frequency)
                .delete(delete_frequency),
        )
        .route(
            "/serials/numbering-patterns",
            get(list_numbering_patterns).post(create_numbering_pattern),
        )
        .route(
            "/serials/numbering-patterns/:id",
            get(get_numbering_pattern)
                .put(update_numbering_pattern)
                .delete(delete_numbering_pattern),
        )
        .route("/serials/predictions", post(test_prediction))
        .route(
            "/serials/subscriptions",
            get(super::serial_subscriptions::list_subscriptions)
                .post(super::serial_subscriptions::create_subscription),
        )
        .route(
            "/serials/subscriptions/:id",
            get(super::serial_subscriptions::get_subscription)
                .put(super::serial_subscriptions::update_subscription)
                .delete(super::serial_subscriptions::delete_subscription),
        )
        .route(
            "/serials/subscriptions/:id/issues",
            get(super::serial_subscriptions::list_subscription_issues),
        )
        .route(
            "/serials/subscriptions/:id/generate",
            post(super::serial_subscriptions::generate_subscription),
        )
}

#[utoipa::path(
    get,
    path = "/serials/frequencies",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(SerialListQuery),
    responses(
        (status = 200, description = "Frequency list", body = SerialFrequencyListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn list_frequencies(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<SerialListQuery>,
) -> AppResult<Json<SerialFrequencyListResponse>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.list_frequencies(&query).await?))
}

#[utoipa::path(
    get,
    path = "/serials/frequencies/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Frequency id")),
    responses(
        (status = 200, description = "Frequency", body = SerialFrequency),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Frequency not found", body = ErrorResponse),
    )
)]
pub async fn get_frequency(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<SerialFrequency>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.get_frequency(id).await?))
}

#[utoipa::path(
    post,
    path = "/serials/frequencies",
    tag = "serials",
    security(("bearer_auth" = [])),
    request_body = FrequencyWrite,
    responses(
        (status = 201, description = "Frequency created", body = SerialFrequency),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn create_frequency(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<FrequencyWrite>,
) -> AppResult<(StatusCode, Json<SerialFrequency>)> {
    claims.require_write_serials()?;
    match state.services.serials.create_frequency(&data).await {
        Ok(frequency) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_CREATED,
                Some(claims.user_id),
                Some("serial_frequency"),
                Some(frequency.id),
                ip,
                Some(&frequency),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(frequency)))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_CREATED,
                Some(claims.user_id),
                Some("serial_frequency"),
                None,
                ip,
                Some(&data.description),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    put,
    path = "/serials/frequencies/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Frequency id")),
    request_body = FrequencyWrite,
    responses(
        (status = 200, description = "Frequency updated", body = SerialFrequency),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Frequency not found", body = ErrorResponse),
    )
)]
pub async fn update_frequency(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<FrequencyWrite>,
) -> AppResult<Json<SerialFrequency>> {
    claims.require_write_serials()?;
    match state.services.serials.update_frequency(id, &data).await {
        Ok(frequency) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_UPDATED,
                Some(claims.user_id),
                Some("serial_frequency"),
                Some(id),
                ip,
                Some(&frequency),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(frequency))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_UPDATED,
                Some(claims.user_id),
                Some("serial_frequency"),
                Some(id),
                ip,
                Some(&data.description),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/serials/frequencies/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Frequency id")),
    responses(
        (status = 204, description = "Frequency deleted"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Frequency not found", body = ErrorResponse),
        (status = 409, description = "Frequency is still referenced", body = ErrorResponse),
    )
)]
pub async fn delete_frequency(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    claims.require_write_serials()?;
    match state.services.serials.delete_frequency(id).await {
        Ok(frequency) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_DELETED,
                Some(claims.user_id),
                Some("serial_frequency"),
                Some(id),
                ip,
                Some(&frequency),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_FREQUENCY_DELETED,
                Some(claims.user_id),
                Some("serial_frequency"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    get,
    path = "/serials/numbering-patterns",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(SerialListQuery),
    responses(
        (status = 200, description = "Numbering pattern list", body = SerialNumberingPatternListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn list_numbering_patterns(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<SerialListQuery>,
) -> AppResult<Json<SerialNumberingPatternListResponse>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.list_patterns(&query).await?))
}

#[utoipa::path(
    get,
    path = "/serials/numbering-patterns/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Numbering pattern id")),
    responses(
        (status = 200, description = "Numbering pattern", body = SerialNumberingPattern),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Numbering pattern not found", body = ErrorResponse),
    )
)]
pub async fn get_numbering_pattern(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<SerialNumberingPattern>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.get_pattern(id).await?))
}

#[utoipa::path(
    post,
    path = "/serials/numbering-patterns",
    tag = "serials",
    security(("bearer_auth" = [])),
    request_body = NumberingPatternWrite,
    responses(
        (status = 201, description = "Numbering pattern created", body = SerialNumberingPattern),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn create_numbering_pattern(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<NumberingPatternWrite>,
) -> AppResult<(StatusCode, Json<SerialNumberingPattern>)> {
    claims.require_write_serials()?;
    match state.services.serials.create_pattern(&data).await {
        Ok(pattern) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_CREATED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                Some(pattern.id),
                ip,
                Some(&pattern),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(pattern)))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_CREATED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                None,
                ip,
                Some(&data.label),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    put,
    path = "/serials/numbering-patterns/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Numbering pattern id")),
    request_body = NumberingPatternWrite,
    responses(
        (status = 200, description = "Numbering pattern updated", body = SerialNumberingPattern),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Numbering pattern not found", body = ErrorResponse),
    )
)]
pub async fn update_numbering_pattern(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<NumberingPatternWrite>,
) -> AppResult<Json<SerialNumberingPattern>> {
    claims.require_write_serials()?;
    match state.services.serials.update_pattern(id, &data).await {
        Ok(pattern) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_UPDATED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                Some(id),
                ip,
                Some(&pattern),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(pattern))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_UPDATED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                Some(id),
                ip,
                Some(&data.label),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/serials/numbering-patterns/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Numbering pattern id")),
    responses(
        (status = 204, description = "Numbering pattern deleted"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Numbering pattern not found", body = ErrorResponse),
        (status = 409, description = "Numbering pattern is still referenced", body = ErrorResponse),
    )
)]
pub async fn delete_numbering_pattern(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    claims.require_write_serials()?;
    match state.services.serials.delete_pattern(id).await {
        Ok(pattern) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_DELETED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                Some(id),
                ip,
                Some(&pattern),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_NUMBERING_PATTERN_DELETED,
                Some(claims.user_id),
                Some("serial_numbering_pattern"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/predictions",
    tag = "serials",
    security(("bearer_auth" = [])),
    request_body = TestPredictionRequest,
    responses(
        (status = 200, description = "Predicted issues. Nothing is stored.", body = PredictionResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Frequency or numbering pattern not found", body = ErrorResponse),
        (status = 422, description = "Irregular frequency or prediction cap", body = ErrorResponse),
    )
)]
pub async fn test_prediction(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(data): Json<TestPredictionRequest>,
) -> AppResult<Json<PredictionResponse>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.test_prediction(&data).await?))
}
