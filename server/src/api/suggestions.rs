//! Patron purchase suggestions: propose a title; staff accept or refuse.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::AppResult,
    models::{
        acquisition::{
            CreatePurchaseSuggestion, PurchaseSuggestion, PurchaseSuggestionListResponse,
            PurchaseSuggestionQuery, ReviewPurchaseSuggestion,
        },
        user::Rights,
    },
    services::audit,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

/// Build suggestion routes (`/suggestions`).
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route(
            "/suggestions",
            get(list_suggestions).post(create_suggestion),
        )
        .route("/suggestions/:id", get(get_suggestion))
        .route("/suggestions/:id/accept", post(accept_suggestion))
        .route("/suggestions/:id/refuse", post(refuse_suggestion))
}

fn can_read_all_suggestions(claims: &crate::models::user::UserClaims) -> bool {
    claims.rights.acquisitions_rights.rank() >= Rights::Read.rank()
}

#[utoipa::path(
    post,
    path = "/suggestions",
    tag = "suggestions",
    security(("bearer_auth" = [])),
    request_body = CreatePurchaseSuggestion,
    responses(
        (status = 201, description = "Suggestion created (proposed)", body = PurchaseSuggestion),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
    )
)]
pub async fn create_suggestion(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreatePurchaseSuggestion>,
) -> AppResult<(StatusCode, Json<PurchaseSuggestion>)> {
    match state
        .services
        .acquisitions
        .create_suggestion(claims.user_id, &data)
        .await
    {
        Ok(suggestion) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_CREATED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                Some(suggestion.id),
                ip,
                Some(&suggestion),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(suggestion)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_CREATED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                None,
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    get,
    path = "/suggestions",
    tag = "suggestions",
    security(("bearer_auth" = [])),
    params(PurchaseSuggestionQuery),
    responses(
        (status = 200, description = "Suggestion list (own rows, or all with acquisitions read)", body = PurchaseSuggestionListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
    )
)]
pub async fn list_suggestions(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<PurchaseSuggestionQuery>,
) -> AppResult<Json<PurchaseSuggestionListResponse>> {
    let scope = if can_read_all_suggestions(&claims) {
        None
    } else {
        Some(claims.user_id)
    };
    let (suggestions, total) = state
        .services
        .acquisitions
        .list_suggestions(&query, scope)
        .await?;
    Ok(Json(PurchaseSuggestionListResponse { suggestions, total }))
}

#[utoipa::path(
    get,
    path = "/suggestions/{id}",
    tag = "suggestions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Suggestion id")),
    responses(
        (status = 200, description = "Suggestion", body = PurchaseSuggestion),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Not the proposer and no acquisitions read", body = ErrorResponse),
        (status = 404, description = "Suggestion not found", body = ErrorResponse),
    )
)]
pub async fn get_suggestion(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<PurchaseSuggestion>> {
    let suggestion = state.services.acquisitions.get_suggestion(id).await?;
    if suggestion.proposed_by != claims.user_id && !can_read_all_suggestions(&claims) {
        return Err(crate::error::AppError::Authorization(
            "Insufficient rights to view this suggestion".into(),
        ));
    }
    Ok(Json(suggestion))
}

#[utoipa::path(
    post,
    path = "/suggestions/{id}/accept",
    tag = "suggestions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Suggestion id")),
    request_body = ReviewPurchaseSuggestion,
    responses(
        (status = 200, description = "Accepted; draft purchase-order line opened (no biblio, no item)", body = PurchaseSuggestion),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Suggestion not found", body = ErrorResponse),
        (status = 409, description = "Already accepted", body = ErrorResponse),
        (status = 422, description = "Refused suggestions stay refused", body = ErrorResponse),
    )
)]
pub async fn accept_suggestion(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<ReviewPurchaseSuggestion>,
) -> AppResult<Json<PurchaseSuggestion>> {
    claims.require_write_acquisitions()?;
    match state
        .services
        .acquisitions
        .accept_suggestion(id, claims.user_id, &data)
        .await
    {
        Ok(suggestion) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_ACCEPTED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                Some(id),
                ip.clone(),
                Some(&suggestion),
                audit::AuditLogMeta::success(),
            );
            if let Some(order_id) = suggestion.purchase_order_id {
                state.services.audit.log(
                    audit::event::PURCHASE_ORDER_CREATED,
                    Some(claims.user_id),
                    Some("purchase_order"),
                    Some(order_id),
                    ip,
                    Some(&suggestion),
                    audit::AuditLogMeta::success(),
                );
            }
            Ok(Json(suggestion))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_ACCEPTED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/suggestions/{id}/refuse",
    tag = "suggestions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Suggestion id")),
    request_body = ReviewPurchaseSuggestion,
    responses(
        (status = 200, description = "Refused; no order intent created", body = PurchaseSuggestion),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Suggestion not found", body = ErrorResponse),
        (status = 409, description = "Already refused", body = ErrorResponse),
        (status = 422, description = "Accepted suggestions cannot be refused", body = ErrorResponse),
    )
)]
pub async fn refuse_suggestion(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<ReviewPurchaseSuggestion>,
) -> AppResult<Json<PurchaseSuggestion>> {
    claims.require_write_acquisitions()?;
    match state
        .services
        .acquisitions
        .refuse_suggestion(id, claims.user_id, &data)
        .await
    {
        Ok(suggestion) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_REFUSED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                Some(id),
                ip,
                Some(&suggestion),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(suggestion))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_SUGGESTION_REFUSED,
                Some(claims.user_id),
                Some("purchase_suggestion"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}
