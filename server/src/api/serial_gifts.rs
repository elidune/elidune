//! One-off periodical gifts (no subscription).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::AppResult,
    models::serial::{CreateSerialGift, SerialGift, SerialGiftListResponse, SerialGiftQuery},
    services::audit,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

#[utoipa::path(
    get,
    path = "/serials/gifts",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(SerialGiftQuery),
    responses(
        (status = 200, description = "Recent periodical gifts", body = SerialGiftListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn list_gifts(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<SerialGiftQuery>,
) -> AppResult<Json<SerialGiftListResponse>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.list_gifts(&query).await?))
}

#[utoipa::path(
    post,
    path = "/serials/gifts",
    tag = "serials",
    security(("bearer_auth" = [])),
    request_body = CreateSerialGift,
    responses(
        (status = 201, description = "Gift copy registered", body = SerialGift),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Periodical not found", body = ErrorResponse),
        (status = 409, description = "Duplicate barcode", body = ErrorResponse),
    )
)]
pub async fn create_gift(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(body): ValidatedJson<CreateSerialGift>,
) -> AppResult<(StatusCode, Json<SerialGift>)> {
    claims.require_write_serials()?;
    let created = state.services.serials.create_gift(&body).await?;
    state.services.audit.log(
        audit::event::ITEM_CREATED,
        Some(claims.user_id),
        Some("item"),
        Some(created.item_id),
        ip,
        Some(serde_json::json!({
            "serialGiftId": created.id.to_string(),
            "biblioId": created.biblio_id.to_string(),
            "volumeLabel": created.volume_label,
        })),
        audit::AuditLogMeta::success(),
    );
    Ok((StatusCode::CREATED, Json(created)))
}
