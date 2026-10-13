//! Hold endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use utoipa::{IntoParams, ToSchema};

use std::collections::HashMap;

use crate::{
    error::{AppError, AppResult},
    models::{
        catalog_view::{
            partial_user_short, project_listed_hold_patron, HoldDetailsPublic, HoldDetailsStaff,
        },
        dto::holds::{HoldPickListItem, HoldPickListRow, HoldQuota, HoldsPolicy},
        hold::{CreateHold, Hold},
        my_account::MyHoldView,
        user::{Rights, UserShort},
    },
    services::audit,
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, StaffUser};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{delete, get, post};
    axum::Router::new()
        .route("/me/holds", get(list_my_holds))
        .route("/holds", get(list_holds).post(create_hold))
        .route("/holds/pick-list", get(list_pick_list))
        .route("/holds/:id/pick-not-found", post(pick_not_found))
        .route(
            "/holds/policy",
            get(get_holds_policy).put(update_holds_policy),
        )
        .route("/holds/quota", get(get_hold_quota))
        .route("/holds/:id", delete(cancel_hold))
        .route("/items/:id/holds", get(list_holds_for_item))
        .route("/biblios/:id/holds", get(list_holds_for_biblio))
        .route("/users/:id/holds", get(list_holds_for_user))
}

/// Query for `GET /holds/pick-list`. `siteId` is the copy's current branch.
#[serde_as]
#[derive(Debug, Default, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PickListQuery {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
}

/// Shelf copies waiting to be pulled (`to_pick`). Staff only.
#[utoipa::path(
    get,
    path = "/holds/pick-list",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(PickListQuery),
    responses(
        (status = 200, description = "Hold-shelf labels for copies to pull. `holdFor` is `UserShortStaff` when `users_rights` is read or write, otherwise `{ id, cardNumber, initials }`. `pickupDeadline` is `holds.expires_at` (null while `to_pick`). `pickDueAt` is the shelf-pick due time. The patron name is omitted unless the caller can read users.", body = [HoldPickListItem]),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Staff access required", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_pick_list(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    Query(query): Query<PickListQuery>,
) -> AppResult<Json<Vec<HoldPickListItem>>> {
    let rows = state.services.holds.list_pick(query.site_id).await?;
    let full = pick_patrons(&state, &claims, &rows).await?;
    let items = rows
        .into_iter()
        .map(|row| {
            let user = full.get(&row.user_id).cloned().unwrap_or_else(|| {
                partial_user_short(row.user_id, row.card_number, row.firstname, row.lastname)
            });
            HoldPickListItem {
                hold_id: row.hold_id,
                item_id: row.item_id,
                barcode: row.barcode,
                call_number: row.call_number,
                title: row.title,
                user_id: row.user_id,
                hold_for: project_listed_hold_patron(user, &claims),
                pickup_site_id: row.pickup_site_id,
                pickup_site_name: row.pickup_site_name,
                pick_started_at: row.pick_started_at,
                pick_due_at: row.pick_due_at,
                pickup_deadline: row.pickup_deadline,
                overdue: row.overdue,
            }
        })
        .collect();
    Ok(Json(items))
}

async fn pick_patrons(
    state: &crate::AppState,
    claims: &crate::models::user::UserClaims,
    rows: &[HoldPickListRow],
) -> AppResult<HashMap<i64, UserShort>> {
    if claims.rights.users_rights.rank() < Rights::Read.rank() {
        return Ok(HashMap::new());
    }
    let mut ids: Vec<i64> = rows.iter().map(|row| row.user_id).collect();
    ids.sort_unstable();
    ids.dedup();
    state.services.repository.users_get_short_by_ids(&ids).await
}

/// The assigned copy was not on the shelf. Staff only.
#[utoipa::path(
    post,
    path = "/holds/{id}/pick-not-found",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Hold id")),
    responses(
        (status = 200, description = "Copy marked to_search; hold reassigned or released", body = Hold),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Staff access required", body = crate::error::ErrorResponse),
        (status = 404, description = "Hold not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Hold is not to_pick", body = crate::error::ErrorResponse)
    )
)]
pub async fn pick_not_found(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<Hold>> {
    match state.services.holds.pick_not_found(id).await {
        Ok(outcome) => {
            state.services.audit.log(
                audit::event::ITEM_MARKED_TO_SEARCH,
                Some(claims.user_id),
                Some("item"),
                Some(outcome.item_id),
                ip.clone(),
                Some(serde_json::json!({
                    "holdId": outcome.hold.id.to_string(),
                    "status": outcome.hold.status,
                    "reassignedItemId": outcome.hold.item_id.map(|v| v.to_string()),
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(outcome.hold))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_MARKED_TO_SEARCH,
                Some(claims.user_id),
                Some("hold"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Query parameters for `GET /holds` (global list).
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListHoldsQuery {
    /// Page number (1-based, default 1)
    pub page: Option<i64>,
    /// Page size (default 50, max 200)
    pub per_page: Option<i64>,
    /// If true, only `pending`, `to_pick`, and `ready` holds (ongoing).
    pub active_only: Option<bool>,
}

/// The caller's holds. Filtered in SQL by `user_id = caller`.
#[utoipa::path(
    get,
    path = "/me/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(ListHoldsQuery),
    responses(
        (status = 200, description = "The caller's holds (`MyHoldView`)", body = PaginatedResponse<MyHoldView>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_my_holds(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<ListHoldsQuery>,
) -> AppResult<Json<PaginatedResponse<MyHoldView>>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let active_only = query.active_only.unwrap_or(false);
    let (items, total) = state
        .services
        .repository
        .holds_list_for_owner(claims.user_id, page, per_page, active_only)
        .await?;
    Ok(Json(PaginatedResponse::new(items, total, page, per_page)))
}

/// Staff or reader hold page. Selected with [`crate::models::user::UserClaims::is_librarian`].
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum HoldListResponse {
    Staff(PaginatedResponse<HoldDetailsStaff>),
    Public(PaginatedResponse<HoldDetailsPublic>),
}

/// Paginated list of holds: staff (`holds_rights` read/write) sees all rows; `o` sees only their holds.
#[utoipa::path(
    get,
    path = "/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(ListHoldsQuery),
    responses(
        (status = 200, description = "Holds. Librarians and admins receive `HoldDetailsStaff`. `user` is `UserShortStaff` when `users_rights` is read or write, otherwise `{ id, cardNumber, initials }`. `expiresAt` is the pickup deadline. Readers receive `HoldDetailsPublic` for their own holds.", body = HoldListResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_holds(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<ListHoldsQuery>,
) -> AppResult<Json<HoldListResponse>> {
    claims.require_list_holds()?;

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 200);
    let active_only = query.active_only.unwrap_or(false);

    let (items, total) = if claims.rights.holds_rights.rank() >= Rights::Read.rank() {
        state
            .services
            .holds
            .list_all(page, per_page, active_only)
            .await?
    } else {
        state
            .services
            .holds
            .list_for_user_paginated(claims.user_id, page, per_page, active_only)
            .await?
    };
    let body = if claims.is_librarian() {
        HoldListResponse::Staff(PaginatedResponse::new(
            items
                .into_iter()
                .map(|hold| HoldDetailsStaff::for_caller(hold, &claims))
                .collect(),
            total,
            page,
            per_page,
        ))
    } else {
        HoldListResponse::Public(PaginatedResponse::new(
            items.into_iter().map(HoldDetailsPublic::from).collect(),
            total,
            page,
            per_page,
        ))
    };
    Ok(Json(body))
}

#[serde_as]
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateHoldRequest {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    /// Physical copy (`items.id`). Omit for a title-level hold.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    /// Bibliographic record. Required when `itemId` is omitted (title-level hold).
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    /// Pickup site (`sources.id`). Required for inter-site transit.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub notes: Option<String>,
    /// Staff-only: place the hold even when the patron is at/over the active-hold cap.
    pub force: Option<bool>,
}

#[utoipa::path(
    post,
    path = "/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    request_body = CreateHoldRequest,
    responses(
        (status = 201, description = "Hold created", body = Hold),
        (status = 400, description = "Invalid request", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient rights or force requires staff", body = crate::error::ErrorResponse),
        (status = 409, description = "User already has a hold for this item or title", body = crate::error::ErrorResponse),
        (status = 422, description = "Business rule: max active holds reached (staff may retry with force=true)")
    )
)]
pub async fn create_hold(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(req): Json<CreateHoldRequest>,
) -> AppResult<(StatusCode, Json<Hold>)> {
    claims.require_create_hold()?;
    if claims.rights.holds_rights.rank() < Rights::Write.rank() && req.user_id != claims.user_id {
        return Err(AppError::Authorization(
            "Insufficient rights to place a hold for another user".into(),
        ));
    }
    let force = req.force.unwrap_or(false);
    if force && claims.rights.holds_rights.rank() < Rights::Write.rank() {
        return Err(AppError::Authorization(
            "Staff rights required to override the holds cap".into(),
        ));
    }
    if req.item_id.is_none() && req.biblio_id.is_none() {
        return Err(AppError::Validation(
            "itemId or biblioId is required".into(),
        ));
    }
    let data = CreateHold {
        user_id: req.user_id,
        item_id: req.item_id,
        biblio_id: req.biblio_id,
        pickup_site_id: req.pickup_site_id,
        notes: req.notes,
        force,
    };
    match state
        .services
        .holds
        .place_hold(data, Some(claims.user_id), ip.clone())
        .await
    {
        Ok(hold) => {
            state.services.audit.log(
                audit::event::HOLD_CREATED,
                Some(claims.user_id),
                Some("hold"),
                Some(hold.id),
                ip.clone(),
                Some(serde_json::json!({
                    "user_id": req.user_id,
                    "item_id": req.item_id,
                    "biblio_id": req.biblio_id,
                    "force": force,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(hold)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::HOLD_CREATED,
                Some(claims.user_id),
                Some("hold"),
                None,
                ip,
                Some(serde_json::json!({
                    "user_id": req.user_id,
                    "item_id": req.item_id,
                    "biblio_id": req.biblio_id,
                    "force": force,
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Query for `GET /holds/quota` (omit `userId` to read the caller's slots).
#[serde_as]
#[derive(Debug, Default, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct HoldQuotaQuery {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub user_id: Option<i64>,
}

/// Global default max active holds — readable by desk and OPAC patrons who can list holds.
#[utoipa::path(
    get,
    path = "/holds/policy",
    tag = "holds",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Global holds policy", body = HoldsPolicy),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_holds_policy(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<HoldsPolicy>> {
    claims.require_list_holds()?;
    Ok(Json(state.services.holds.get_policy().await?))
}

/// Update the global default max active holds.
#[utoipa::path(
    put,
    path = "/holds/policy",
    tag = "holds",
    security(("bearer_auth" = [])),
    request_body = HoldsPolicy,
    responses(
        (status = 200, description = "Holds policy saved", body = HoldsPolicy),
        (status = 400, description = "Invalid cap", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Staff access required", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_holds_policy(
    State(state): State<crate::AppState>,
    StaffUser(claims): StaffUser,
    ClientIp(ip): ClientIp,
    Json(req): Json<HoldsPolicy>,
) -> AppResult<Json<HoldsPolicy>> {
    claims.require_write_settings()?;
    match state
        .services
        .holds
        .set_policy(req.max_active_holds, req.pick_delay_days)
        .await
    {
        Ok(policy) => {
            state.services.audit.log(
                audit::event::HOLD_POLICY_UPDATED,
                Some(claims.user_id),
                Some("circulation_settings"),
                Some(1),
                ip,
                Some(serde_json::json!({
                    "maxActiveHolds": policy.max_active_holds,
                    "pickDelayDays": policy.pick_delay_days,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(policy))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::HOLD_POLICY_UPDATED,
                Some(claims.user_id),
                Some("circulation_settings"),
                Some(1),
                ip,
                Some(serde_json::json!({
                    "maxActiveHolds": req.max_active_holds,
                    "pickDelayDays": req.pick_delay_days,
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Remaining hold slots for the caller (or a patron, for staff).
#[utoipa::path(
    get,
    path = "/holds/quota",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(HoldQuotaQuery),
    responses(
        (status = 200, description = "Resolved hold quota", body = HoldQuota),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Cannot read another user's quota", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_hold_quota(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<HoldQuotaQuery>,
) -> AppResult<Json<HoldQuota>> {
    claims.require_list_holds()?;
    let user_id = query.user_id.unwrap_or(claims.user_id);
    claims.require_self_or_staff(user_id)?;
    Ok(Json(state.services.holds.quota_for_user(user_id).await?))
}

#[utoipa::path(
    get,
    path = "/items/{id}/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Item ID")),
    responses(
        (status = 200, description = "Hold queue for this item (`HoldDetailsStaff`). `user` is `UserShortStaff` with users read, otherwise `{ id, cardNumber, initials }`. `expiresAt` is the pickup deadline.", body = Vec<HoldDetailsStaff>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 404, description = "Item not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_holds_for_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(item_id): Path<i64>,
) -> AppResult<Json<Vec<HoldDetailsStaff>>> {
    claims.require_read_holds_staff()?;
    let list = state.services.holds.get_for_item(item_id).await?;
    Ok(Json(
        list.into_iter()
            .map(|hold| HoldDetailsStaff::for_caller(hold, &claims))
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/biblios/{id}/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Biblio ID")),
    responses(
        (status = 200, description = "Hold queue for this bibliographic record (notice-level FIFO, `HoldDetailsStaff`). `user` is `UserShortStaff` with users read, otherwise `{ id, cardNumber, initials }`. `expiresAt` is the pickup deadline.", body = Vec<HoldDetailsStaff>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 404, description = "Biblio not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_holds_for_biblio(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(biblio_id): Path<i64>,
) -> AppResult<Json<Vec<HoldDetailsStaff>>> {
    claims.require_read_holds_staff()?;
    let list = state.services.holds.get_for_biblio(biblio_id).await?;
    Ok(Json(
        list.into_iter()
            .map(|hold| HoldDetailsStaff::for_caller(hold, &claims))
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/users/{id}/holds",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "User's holds (`HoldDetailsStaff`). Caller already has users read, so `user` is `UserShortStaff`. `expiresAt` is the pickup deadline.", body = Vec<HoldDetailsStaff>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 404, description = "User not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_holds_for_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(user_id): Path<i64>,
) -> AppResult<Json<Vec<HoldDetailsStaff>>> {
    claims.require_read_holds_staff()?;
    claims.require_read_users()?;
    let list = state.services.holds.get_for_user(user_id).await?;
    Ok(Json(
        list.into_iter()
            .map(|hold| HoldDetailsStaff::for_caller(hold, &claims))
            .collect(),
    ))
}

#[utoipa::path(
    delete,
    path = "/holds/{id}",
    tag = "holds",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Hold ID")),
    responses(
        (status = 200, description = "Hold cancelled", body = Hold),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Cannot cancel another user's hold", body = crate::error::ErrorResponse),
        (status = 404, description = "Hold not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn cancel_hold(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<Hold>> {
    claims.require_cancel_hold()?;
    let can_manage_others = claims.rights.holds_rights.rank() >= Rights::Write.rank();
    match state
        .services
        .holds
        .cancel(id, claims.user_id, can_manage_others)
        .await
    {
        Ok(hold) => {
            state.services.audit.log(
                audit::event::HOLD_CANCELLED,
                Some(claims.user_id),
                Some("hold"),
                Some(id),
                ip.clone(),
                None::<()>,
                audit::AuditLogMeta::success(),
            );
            Ok(Json(hold))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::HOLD_CANCELLED,
                Some(claims.user_id),
                Some("hold"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}
