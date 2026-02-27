//! Item (catalog) endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult},
    marc::parse_unimarc_to_items,
    models::{
        item::{Item, ItemQuery, ItemShort},
        specimen::{CreateSpecimen, Specimen, UpdateSpecimen},
    },
};

use super::AuthenticatedUser;

#[derive(Debug, Deserialize, Default)]
pub struct GetItemQuery {
    /// If true, include the full MARC record (marc_record JSONB) in the response
    #[serde(default)]
    pub full_record: bool,
}

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
        ("isbn" = Option<String>, Query, description = "Search by ISBN/ISSN"),
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
        ("id" = i32, Path, description = "Item ID"),
        ("full_record" = Option<bool>, Query, description = "If true, include full MARC record data")
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
    Query(query): Query<GetItemQuery>,
) -> AppResult<Json<Item>> {
    claims.require_read_items()?;

    let item = if query.full_record {
        state.services.catalog.get_item_with_full_record(id).await?
    } else {
        state.services.catalog.get_item(id).await?
    };
    Ok(Json(item))
}

/// Query params for create item
#[derive(Debug, Deserialize, Default)]
pub struct CreateItemQuery {
    /// If true, allow creating an item even when another item has the same ISBN
    #[serde(default)]
    pub allow_duplicate_isbn: bool,
}

/// Create a new item
#[utoipa::path(
    post,
    path = "/items",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("allow_duplicate_isbn" = Option<bool>, Query, description = "Allow duplicate ISBN (default: false)")
    ),
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
    Query(query): Query<CreateItemQuery>,
    Json(item): Json<Item>,
) -> AppResult<(StatusCode, Json<Item>)> {
    claims.require_write_items()?;

    let created = state
        .services
        .catalog
        .create_item(item, query.allow_duplicate_isbn)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Upload a UNIMARC file and return parsed items with linked specimens (995/952).
#[utoipa::path(
    post,
    path = "/items/upload-unimarc",
    tag = "items",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Parsed items with specimens", body = Vec<Item>),
        (status = 400, description = "Missing file or invalid UNIMARC"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn upload_unimarc(
    AuthenticatedUser(claims): AuthenticatedUser,
    mut multipart: Multipart,
) -> AppResult<Json<Vec<Item>>> {
    claims.require_read_items()?;

    let mut data = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))?
    {
        if field.name().as_deref() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("Failed to read field: {}", e)))?;
            data = bytes.to_vec();
            break;
        }
    }
    if data.is_empty() {
        return Err(AppError::BadRequest(
            "Missing 'file' field in multipart form".to_string(),
        ));
    }

    let items = parse_unimarc_to_items(&data)
        .map_err(|e| AppError::Validation(format!("UNIMARC parse error: {}", e)))?;
    Ok(Json(items))
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
        (status = 404, description = "Item not found"),
        (status = 409, description = "A specimen with this barcode already exists")
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

/// Update a specimen
#[utoipa::path(
    put,
    path = "/items/{item_id}/specimens/{specimen_id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("item_id" = i32, Path, description = "Item ID"),
        ("specimen_id" = i32, Path, description = "Specimen ID")
    ),
    request_body = UpdateSpecimen,
    responses(
        (status = 200, description = "Specimen updated", body = Specimen),
        (status = 404, description = "Item or specimen not found"),
        (status = 409, description = "A specimen with this barcode already exists")
    )
)]
pub async fn update_specimen(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path((item_id, specimen_id)): Path<(i32, i32)>,
    Json(specimen): Json<UpdateSpecimen>,
) -> AppResult<Json<Specimen>> {
    claims.require_write_items()?;

    let updated = state
        .services
        .catalog
        .update_specimen(item_id, specimen_id, specimen)
        .await?;
    Ok(Json(updated))
}

/// Delete a specimen
#[utoipa::path(
    delete,
    path = "/items/{item_id}/specimens/{specimen_id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(
        ("item_id" = i32, Path, description = "Item ID"),
        ("specimen_id" = i32, Path, description = "Specimen ID"),
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
    Path((item_id, specimen_id)): Path<(i32, i32)>,
    Query(params): Query<DeleteSpecimenParams>,
) -> AppResult<StatusCode> {
    claims.require_write_items()?;

    state
        .services
        .catalog
        .delete_specimen(item_id, specimen_id, params.force.unwrap_or(false))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeleteSpecimenParams {
    pub force: Option<bool>,
}
