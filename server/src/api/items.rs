//! Physical item (copy) endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    api::biblios::BiblioReadResponse,
    error::AppResult,
    models::catalog_view::{sees_internal_catalog, BiblioPublicView},
    models::dto::weeding::SetWeedingRequest,
    models::item::Item,
    services::audit::{self},
};

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/items/barcode/:barcode", get(get_biblio_by_barcode))
        .route(
            "/items/:id",
            get(get_biblio_by_item).put(update_item).delete(delete_item),
        )
        .route("/items/:id/weeding", post(set_item_weeding))
}

/// Get the bibliographic record for a physical copy.
///
/// Response is a full [`Biblio`]; `items` contains **only** the copy whose id was requested.
#[utoipa::path(
    get,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Physical copy (item) ID")
    ),
    responses(
        (status = 200, description = "Biblio with a single item entry. Callers with internal catalog rights receive `BiblioStaffView` including `circulationStatus`. Catalog read alone receives `BiblioPublicView` with public availability. Lost, declared missing, weeding-in-progress, and withdrawn copies are not returned. The record includes `holdsCount`. The copy includes `dueDate` and `onHold` (same rules as `GET /biblios/{id}`). Users or loans read receive `holdFor` on an `onHold` copy. The public view omits `holdFor`. This route does not include `borrower`.", body = BiblioReadResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 404, description = "Item not found, archived, or hidden from this caller", body = crate::error::ErrorResponse),
        (status = 410, description = "Bibliographic record is archived", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_biblio_by_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(item_id): Path<i64>,
) -> AppResult<Json<BiblioReadResponse>> {
    claims.require_read_items()?;
    let biblio = state.services.catalog.get_biblio_for_item(item_id).await?;
    if !sees_internal_catalog(&claims) && copy_hidden_from_public(&biblio) {
        return Err(crate::error::AppError::NotFound(
            "Bibliographic record not found".into(),
        ));
    }
    let body = if sees_internal_catalog(&claims) {
        BiblioReadResponse::Staff(
            super::catalog_hold::staff_biblio_view(&state, &claims, biblio).await?,
        )
    } else {
        BiblioReadResponse::Public(BiblioPublicView::from(biblio))
    };
    Ok(Json(body))
}

/// Lost, missing, weeding-in-progress, and withdrawn copies are not a public record.
fn copy_hidden_from_public(biblio: &crate::models::biblio::Biblio) -> bool {
    biblio
        .items
        .iter()
        .any(|item| !item.included_in_public_catalog())
}

/// Get the bibliographic record for a physical copy identified by barcode.
///
/// Response is a full [`Biblio`]; `items` contains **only** the matching copy.
#[utoipa::path(
    get,
    path = "/items/barcode/{barcode}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("barcode" = String, Path, description = "Physical copy barcode (exact match)")
    ),
    responses(
        (status = 200, description = "Biblio with a single item entry. Callers with internal catalog rights receive `BiblioStaffView` including `circulationStatus`. Catalog read alone receives `BiblioPublicView` with public availability. Lost, declared missing, weeding-in-progress, and withdrawn copies are not returned. The record includes `holdsCount`. The copy includes `dueDate` and `onHold` (same rules as `GET /biblios/{id}`). Users or loans read receive `holdFor` on an `onHold` copy. The public view omits `holdFor`. This route does not include `borrower`.", body = BiblioReadResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 404, description = "No active item with this barcode, or it is hidden from this caller", body = crate::error::ErrorResponse),
        (status = 410, description = "Bibliographic record is archived", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_biblio_by_barcode(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(barcode): Path<String>,
) -> AppResult<Json<BiblioReadResponse>> {
    claims.require_read_items()?;
    let biblio = state
        .services
        .catalog
        .get_biblio_for_item_barcode(barcode.as_str())
        .await?;
    if !sees_internal_catalog(&claims) && copy_hidden_from_public(&biblio) {
        return Err(crate::error::AppError::NotFound(
            "Bibliographic record not found".into(),
        ));
    }
    let body = if sees_internal_catalog(&claims) {
        BiblioReadResponse::Staff(
            super::catalog_hold::staff_biblio_view(&state, &claims, biblio).await?,
        )
    } else {
        BiblioReadResponse::Public(BiblioPublicView::from(biblio))
    };
    Ok(Json(body))
}

/// Update a physical item. The path id is authoritative.
///
/// `circulationStatus` is read-only. A request that includes it returns 422
/// `circulation_status_read_only` and does not apply any other field. Use the
/// dedicated actions (lost, missing, to-search).
#[utoipa::path(
    put,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Physical copy (item) ID")
    ),
    request_body = Item,
    responses(
        (status = 200, description = "Physical item updated", body = Item),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 404, description = "Biblio or item not found", body = crate::error::ErrorResponse),
        (status = 409, description = "An item with this barcode already exists"),
        (status = 422, description = "`circulationStatus` is read-only (`circulation_status_read_only`). Use the dedicated actions (lost, missing, to-search). No other field in the request is applied.", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    ValidatedJson(mut item): ValidatedJson<Item>,
) -> AppResult<Json<Item>> {
    claims.require_write_items()?;
    match state.services.catalog.update_item(item_id, &mut item).await {
        Ok((biblio_id, _)) => {
            state.services.audit.log(
                audit::event::ITEM_UPDATED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip.clone(),
                Some((biblio_id, &item)),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(item))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_UPDATED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip,
                Some(&item),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Delete a physical item (soft delete unless `force` when borrowed).
#[utoipa::path(
    delete,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Physical copy (item) ID"),
        ("force" = Option<bool>, Query, description = "Force delete even if borrowed")
    ),
    responses(
        (status = 204, description = "Physical item deleted"),
        (status = 404, description = "Item not found"),
        (status = 409, description = "Item is borrowed")
    )
)]
pub async fn delete_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    Query(params): Query<DeleteItemParams>,
) -> AppResult<StatusCode> {
    claims.require_delete_items()?;
    let force = params.force.unwrap_or(false);
    match state.services.catalog.delete_item(item_id, force).await {
        Ok(biblio_id) => {
            state.services.audit.log(
                audit::event::ITEM_DELETED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip.clone(),
                Some(serde_json::json!({
                    "biblio_id": biblio_id,
                    "item_id": item_id,
                    "force": force,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_DELETED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip,
                Some(serde_json::json!({
                    "item_id": item_id,
                    "force": force,
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteItemParams {
    pub force: Option<bool>,
}

/// Set weeding status on a physical copy. Candidate stays circulable; withdrawn is not.
#[utoipa::path(
    post,
    path = "/items/{id}/weeding",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Physical copy (item) ID")
    ),
    request_body = SetWeedingRequest,
    responses(
        (status = 200, description = "Weeding status updated", body = Item),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 404, description = "Item not found or archived", body = crate::error::ErrorResponse)
    )
)]
pub async fn set_item_weeding(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    ValidatedJson(body): ValidatedJson<SetWeedingRequest>,
) -> AppResult<Json<Item>> {
    claims.require_weeding()?;
    match state
        .services
        .catalog
        .set_item_weeding(item_id, body.status, body.reason.clone())
        .await
    {
        Ok(item) => {
            state.services.audit.log(
                audit::event::ITEM_WEEDING_UPDATED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip.clone(),
                Some(serde_json::json!({
                    "itemId": item_id.to_string(),
                    "status": body.status,
                    "reason": body.reason,
                    "archivedAt": item.archived_at,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(item))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WEEDING_UPDATED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip,
                Some(serde_json::json!({
                    "itemId": item_id.to_string(),
                    "status": body.status,
                    "reason": body.reason,
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}
