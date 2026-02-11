//! Item (catalog) endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    error::AppResult,
    models::{
        item::{Item, ItemQuery, ItemShort},
        specimen::{CreateSpecimen, Specimen},
    },
};

use super::AuthenticatedUser;

/// Paginated response wrapper
#[derive(Serialize, ToSchema)]
pub struct PaginatedResponse<T>
where
    T: for<'a> ToSchema<'a>,
{
    /// List of items
    pub items: Vec<T>,
    /// Total number of items
    pub total: i64,
    /// Current page number
    pub page: i64,
    /// Items per page
    pub per_page: i64,
}

/// List items with search and pagination
#[utoipa::path(
    get,
    path = "/items",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("media_type" = Option<String>, Query, description = "Filter by media type"),
        ("title" = Option<String>, Query, description = "Search in title"),
        ("author" = Option<String>, Query, description = "Search by author"),
        ("identification" = Option<String>, Query, description = "Search by ISBN/ISSN"),
        ("freesearch" = Option<String>, Query, description = "Full-text search"),
        ("page" = Option<i64>, Query, description = "Page number (default: 1)"),
        ("per_page" = Option<i64>, Query, description = "Items per page (default: 20)")
    ),
    responses(
        (status = 200, description = "List of items", body = PaginatedResponse<ItemShort>),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn list_items(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<ItemQuery>,
) -> AppResult<Json<PaginatedResponse<ItemShort>>> {
    claims.require_read_items()?;

    let (items, total) = state.services.catalog.search_items(&query).await?;

    Ok(Json(PaginatedResponse {
        items,
        total,
        page: query.page.unwrap_or(1),
        per_page: query.per_page.unwrap_or(20),
    }))
}

/// Get item details by ID
#[utoipa::path(
    get,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "Item details", body = Item),
        (status = 404, description = "Item not found")
    )
)]
pub async fn get_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
) -> AppResult<Json<Item>> {
    claims.require_read_items()?;

    let item = state.services.catalog.get_item(id).await?;
    Ok(Json(item))
}

/// Create a new item
#[utoipa::path(
    post,
    path = "/items",
    tag = "items",
    security(("bearer_auth" = [])),
    request_body = Item,
    responses(
        (status = 201, description = "Item created", body = Item),
        (status = 400, description = "Invalid input"),
        (status = 409, description = "Item already exists")
    )
)]
pub async fn create_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(item): Json<Item>,
) -> AppResult<(StatusCode, Json<Item>)> {
    claims.require_write_items()?;

    let created = state.services.catalog.create_item(item).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Update an existing item
#[utoipa::path(
    put,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Item ID")
    ),
    request_body = Item,
    responses(
        (status = 200, description = "Item updated", body = Item),
        (status = 404, description = "Item not found")
    )
)]
pub async fn update_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
    Json(item): Json<Item>,
) -> AppResult<Json<Item>> {
    claims.require_write_items()?;

    let updated = state.services.catalog.update_item(id, item).await?;
    Ok(Json(updated))
}

/// Delete an item
#[utoipa::path(
    delete,
    path = "/items/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Item ID"),
        ("force" = Option<bool>, Query, description = "Force delete even if specimens are borrowed")
    ),
    responses(
        (status = 204, description = "Item deleted"),
        (status = 404, description = "Item not found"),
        (status = 409, description = "Item has borrowed specimens")
    )
)]
pub async fn delete_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
    Query(params): Query<DeleteItemParams>,
) -> AppResult<StatusCode> {
    claims.require_write_items()?;

    state
        .services
        .catalog
        .delete_item(id, params.force.unwrap_or(false))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeleteItemParams {
    pub force: Option<bool>,
}

/// List specimens for an item
#[utoipa::path(
    get,
    path = "/items/{id}/specimens",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Item ID")
    ),
    responses(
        (status = 200, description = "List of specimens", body = Vec<Specimen>),
        (status = 404, description = "Item not found")
    )
)]
pub async fn list_specimens(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(item_id): Path<i32>,
) -> AppResult<Json<Vec<Specimen>>> {
    claims.require_read_items()?;

    let specimens = state.services.catalog.get_specimens(item_id).await?;
    Ok(Json(specimens))
}

/// Create a new specimen for an item
#[utoipa::path(
    post,
    path = "/items/{id}/specimens",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Item ID")
    ),
    request_body = CreateSpecimen,
    responses(
        (status = 201, description = "Specimen created", body = Specimen),
        (status = 404, description = "Item not found")
    )
)]
pub async fn create_specimen(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(item_id): Path<i32>,
    Json(specimen): Json<CreateSpecimen>,
) -> AppResult<(StatusCode, Json<Specimen>)> {
    claims.require_write_items()?;

    let created = state
        .services
        .catalog
        .create_specimen(item_id, specimen)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Delete a specimen
#[utoipa::path(
    delete,
    path = "/specimens/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Specimen ID"),
        ("force" = Option<bool>, Query, description = "Force delete even if borrowed")
    ),
    responses(
        (status = 204, description = "Specimen deleted"),
        (status = 404, description = "Specimen not found"),
        (status = 409, description = "Specimen is borrowed")
    )
)]
pub async fn delete_specimen(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
    Query(params): Query<DeleteSpecimenParams>,
) -> AppResult<StatusCode> {
    claims.require_write_items()?;

    state
        .services
        .catalog
        .delete_specimen(id, params.force.unwrap_or(false))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeleteSpecimenParams {
    pub force: Option<bool>,
}
