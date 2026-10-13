//! Physical item (copy) endpoints

use axum::{
    async_trait,
    extract::{FromRequest, Path, Query, Request, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    api::biblios::BiblioReadResponse,
    error::{AppError, AppResult},
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

/// `PUT /items/{id}` body after the `circulationStatus` presence check.
enum ParsedItemUpdate {
    /// The key was present. The handler still checks write permission first.
    CirculationStatusPresent(serde_json::Value),
    Ready(Box<Item>),
}

/// Accepts `circulationStatus` as any JSON value so a present key is visible
/// before [`Item`] deserialization. `Option<Value>` would treat JSON null as
/// absent; this deserializer runs only when the key is present, including null.
#[derive(Debug, Deserialize)]
struct ItemUpdateProbe {
    #[serde(
        default,
        rename = "circulationStatus",
        deserialize_with = "deserialize_present_json_value"
    )]
    circulation_status: Option<serde_json::Value>,
}

fn deserialize_present_json_value<'de, D>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(Some(value))
}

fn circulation_status_key_present(value: &serde_json::Value) -> bool {
    serde_json::from_value::<ItemUpdateProbe>(value.clone())
        .ok()
        .and_then(|probe| probe.circulation_status)
        .is_some()
}

fn parse_item_update(bytes: &[u8]) -> AppResult<ParsedItemUpdate> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::Validation(format!("Invalid JSON body: {e}")))?;
    if circulation_status_key_present(&value) {
        return Ok(ParsedItemUpdate::CirculationStatusPresent(value));
    }
    let item: Item = serde_json::from_value(value)
        .map_err(|e| AppError::Validation(format!("Invalid JSON body: {e}")))?;
    item.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    Ok(ParsedItemUpdate::Ready(Box::new(item)))
}

pub(crate) struct ItemUpdateBody(ParsedItemUpdate);

#[async_trait]
impl<S> FromRequest<S> for ItemUpdateBody
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        parse_item_update(&bytes).map(Self)
    }
}

/// Update a physical item. The path id is authoritative.
///
/// `circulationStatus` is read-only. A request that includes the key, with any
/// JSON value, returns 422 `circulation_status_read_only` and does not apply
/// any other field. The key is detected before the body is read as [`Item`].
/// Use the dedicated actions (lost, missing, to-search).
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
        (status = 422, description = "`circulationStatus` is read-only (`circulation_status_read_only`) for any JSON value. Use the dedicated actions (lost, missing, to-search). No other field in the request is applied.", body = crate::error::ErrorResponse)
    )
)]
pub(crate) async fn update_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    ItemUpdateBody(parsed): ItemUpdateBody,
) -> AppResult<Json<Item>> {
    claims.require_write_items()?;
    let mut item = match parsed {
        ParsedItemUpdate::CirculationStatusPresent(body) => {
            let err = AppError::circulation_status_read_only();
            state.services.audit.log(
                audit::event::ITEM_UPDATED,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip,
                Some(body),
                audit::AuditLogMeta::from_app_error(&err),
            );
            return Err(err);
        }
        ParsedItemUpdate::Ready(item) => *item,
    };
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

#[cfg(test)]
mod tests {
    use super::{circulation_status_key_present, parse_item_update, ParsedItemUpdate};
    use serde_json::json;

    #[test]
    fn circulation_status_key_is_present_for_any_json_value() {
        for value in [
            json!(1),
            json!("lost"),
            json!(null),
            json!({"status": "lost"}),
            json!(true),
            json!(["lost"]),
        ] {
            let body = json!({
                "callNumber": "SHELF-B",
                "circulationStatus": value,
            });
            assert!(circulation_status_key_present(&body), "present for {value}");
            let parsed =
                parse_item_update(&serde_json::to_vec(&body).expect("json")).expect("parse");
            assert!(
                matches!(parsed, ParsedItemUpdate::CirculationStatusPresent(_)),
                "rejected before item decode for {value}"
            );
        }
    }

    #[test]
    fn omitted_circulation_status_still_parses_as_item() {
        let body = json!({
            "callNumber": "SHELF-B",
            "notes": "changed"
        });
        assert!(!circulation_status_key_present(&body));
        let parsed = parse_item_update(&serde_json::to_vec(&body).expect("json")).expect("parse");
        match parsed {
            ParsedItemUpdate::Ready(item) => {
                assert!(item.circulation_status.is_none());
                assert_eq!(item.call_number.as_deref(), Some("SHELF-B"));
                assert_eq!(item.notes.as_deref(), Some("changed"));
            }
            ParsedItemUpdate::CirculationStatusPresent(_) => {
                panic!("omitted key must not be treated as present");
            }
        }
    }
}
