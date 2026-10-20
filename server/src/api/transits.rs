//! Inter-site item transit endpoints (hold fulfillment).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::{AppError, AppResult},
    models::{
        dto::transits::ListTransitsQuery,
        transit::{CreateTransit, ItemTransit, TransitActionRequest, TransitWithHold},
        user::{Rights, UserClaims},
    },
    services::audit,
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, StaffUser};

/// Librarian or admin, or any account with holds read or write.
///
/// A librarian without holds rights still receives the row (`patron` is null).
/// A custom desk account such as agent de guichet receives it when holds rights
/// are at least read. Readers stay forbidden.
fn require_transit_reader(claims: &UserClaims) -> Result<(), AppError> {
    if claims.is_librarian() || claims.rights.holds_rights.rank() >= Rights::Read.rank() {
        Ok(())
    } else {
        Err(AppError::Authorization("Staff access required".to_string()))
    }
}

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/transits", get(list_transits))
        .route("/transits/:id", get(get_transit))
        .route("/transits/:id/ship", post(ship_transit))
        .route("/transits/:id/receive", post(receive_transit))
        .route("/transits/:id/cancel", post(cancel_transit))
        .route("/holds/:id/transits", get(get_hold_transit).post(create_hold_transit))
        .route("/items/:id/transit", get(get_item_transit))
}

#[utoipa::path(
    get,
    path = "/transits",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(ListTransitsQuery),
    responses(
        (status = 200, description = "Transit list. Each row includes `barcode`, `title`, `sourceId`, `sourceName`, and `patron` (`{ cardNumber, initials }` or null). `patron` is set only while the linked hold is pending, set aside, or ready, and only when the caller has holds read or write. A return to the home site, a cancelled or expired hold, and a caller without holds rights yield `patron: null`. Users read does not add the name.", body = PaginatedResponse<ItemTransit>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Librarian or admin, or holds read, is required")
    )
)]
pub async fn list_transits(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<ListTransitsQuery>,
) -> AppResult<Json<PaginatedResponse<ItemTransit>>> {
    require_transit_reader(&claims)?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let (items, total) = state
        .services
        .transits
        .list(&claims, query.status, query.item_id, query.hold_id, query.to_site_id, page, per_page)
        .await?;
    Ok(Json(PaginatedResponse::new(items, total, page, per_page)))
}

#[utoipa::path(
    get,
    path = "/transits/{id}",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Transit ID")),
    responses(
        (status = 200, description = "Transit", body = TransitWithHold),
        (status = 404, description = "Not found")
    )
)]
pub async fn get_transit(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<Json<TransitWithHold>> {
    require_transit_reader(&claims)?;
    Ok(Json(state.services.transits.get_by_id(id, &claims).await?))
}

#[utoipa::path(
    get,
    path = "/holds/{id}/transits",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Hold ID")),
    responses(
        (status = 200, description = "Active transit for this hold", body = Option<ItemTransit>),
        (status = 404, description = "Hold not found")
    )
)]
pub async fn get_hold_transit(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<Json<Option<ItemTransit>>> {
    require_transit_reader(&claims)?;
    Ok(Json(state.services.transits.active_for_hold(id, &claims).await?))
}

#[utoipa::path(
    get,
    path = "/items/{id}/transit",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Active transit for this copy", body = Option<ItemTransit>)
    )
)]
pub async fn get_item_transit(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<Json<Option<ItemTransit>>> {
    require_transit_reader(&claims)?;
    Ok(Json(state.services.transits.active_for_item(id, &claims).await?))
}

#[utoipa::path(
    post,
    path = "/holds/{id}/transits",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Hold ID")),
    request_body = CreateTransit,
    responses(
        (status = 201, description = "Transit created", body = TransitWithHold),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Active transit already exists"),
        (status = 422, description = "Business rule. When the copy's site is already the pickup site the code is `transit_same_site` (`error` is `Business Rule Violation`) and nothing is written: no hold update and no transit row. Other rules stay `business_rule_violation`.", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_hold_transit(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(data): Json<CreateTransit>,
) -> AppResult<(StatusCode, Json<TransitWithHold>)> {
    claims.require_write_holds()?;
    match state.services.transits.request(&claims, id, data, claims.user_id, ip.clone()).await {
        Ok(body) => Ok((StatusCode::CREATED, Json(body))),
        Err(e) => {
            state.services.audit.log(
                audit::event::TRANSIT_REQUESTED,
                Some(claims.user_id),
                Some("item_transit"),
                None,
                ip,
                Some(serde_json::json!({ "holdId": id.to_string() })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/transits/{id}/ship",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Transit ID")),
    request_body = TransitActionRequest,
    responses(
        (status = 200, description = "Transit shipped", body = ItemTransit),
        (status = 422, description = "Not in requested state")
    )
)]
pub async fn ship_transit(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(req): Json<TransitActionRequest>,
) -> AppResult<Json<ItemTransit>> {
    claims.require_write_holds()?;
    match state.services.transits.ship(&claims, id, claims.user_id, req.notes, ip.clone()).await {
        Ok(transit) => Ok(Json(transit)),
        Err(e) => {
            state.services.audit.log(
                audit::event::TRANSIT_SHIPPED,
                Some(claims.user_id),
                Some("item_transit"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/transits/{id}/receive",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Transit ID")),
    request_body = TransitActionRequest,
    responses(
        (status = 200, description = "Received at pickup; hold ready", body = TransitWithHold),
        (status = 422, description = "Not in transit")
    )
)]
pub async fn receive_transit(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(req): Json<TransitActionRequest>,
) -> AppResult<Json<TransitWithHold>> {
    claims.require_write_holds()?;
    match state.services.transits.receive(&claims, id, claims.user_id, req.notes, ip.clone()).await {
        Ok(body) => Ok(Json(body)),
        Err(e) => {
            state.services.audit.log(
                audit::event::TRANSIT_RECEIVED,
                Some(claims.user_id),
                Some("item_transit"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/transits/{id}/cancel",
    tag = "transits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Transit ID")),
    request_body = TransitActionRequest,
    responses(
        (status = 200, description = "Transit cancelled", body = TransitWithHold)
    )
)]
pub async fn cancel_transit(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(req): Json<TransitActionRequest>,
) -> AppResult<Json<TransitWithHold>> {
    claims.require_write_holds()?;
    match state.services.transits.cancel(&claims, id, claims.user_id, req.reverse, req.notes, ip.clone()).await {
        Ok(body) => Ok(Json(body)),
        Err(e) => {
            state.services.audit.log(
                audit::event::TRANSIT_CANCELLED,
                Some(claims.user_id),
                Some("item_transit"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}
