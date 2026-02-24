//! Z39.50 catalog search endpoints

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    error::AppResult,
    models::{item::Item, remote_item::ItemRemoteShort},
};

use super::AuthenticatedUser;

/// Z39.50 search query parameters
#[derive(Deserialize, IntoParams, ToSchema, Debug)]
pub struct Z3950SearchQuery {
    pub query: String,
    pub server_id: Option<i32>,
    pub max_results: Option<i32>,
}




#[derive(Serialize, ToSchema)]
pub struct Z3950SearchResponse {
    /// Total results found
    pub total: i32,
    /// List of found items
    pub items: Vec<ItemRemoteShort>,
    /// Source server name
    pub source: String,
}

/// Z39.50 import request
#[derive(Deserialize, ToSchema)]
pub struct Z3950ImportRequest {
    /// Remote item ID to import
    pub remote_item_id: i32,
    /// Specimens to create for the imported item
    pub specimens: Option<Vec<ImportSpecimen>>,
}

#[derive(Deserialize, ToSchema)]
pub struct ImportSpecimen {
    /// Specimen barcode (must be unique when provided)
    pub barcode: Option<String>,
    /// Shelf location / call number
    pub call_number: Option<String>,
    /// Status code
    pub status: Option<String>,
    /// Place (shelf/room number)
    pub place: Option<i16>,
    /// Notes
    pub notes: Option<String>,
    /// Price
    pub price: Option<String>,
    /// Source ID
    pub source_id: Option<i32>,
}

/// Search remote catalogs via Z39.50
#[utoipa::path(
    get,
    path = "/z3950/search",
    tag = "z3950",
    security(("bearer_auth" = [])),
    params(
        ("isbn" = Option<String>, Query, description = "ISBN to search"),
        ("title" = Option<String>, Query, description = "Title to search"),
        ("author" = Option<String>, Query, description = "Author to search"),
        ("max_results" = Option<i32>, Query, description = "Max results (default: 50)")
    ),
    responses(
        (status = 200, description = "Search results", body = Z3950SearchResponse),
        (status = 502, description = "Z39.50 server error")
    )
)]
pub async fn search(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<Z3950SearchQuery>,
) -> AppResult<Json<Z3950SearchResponse>> {
    claims.require_read_items()?;

    let (items, total, source) = state.services.z3950.search(&query).await?;

    Ok(Json(Z3950SearchResponse {
        total,
        items,
        source,
    }))
}

/// Import a record from Z39.50 search results into local catalog
#[utoipa::path(
    post,
    path = "/z3950/import",
    tag = "z3950",
    security(("bearer_auth" = [])),
    request_body = Z3950ImportRequest,
    responses(
        (status = 201, description = "Record imported", body = Item),
        (status = 404, description = "Remote item not found"),
        (status = 409, description = "Item already exists in local catalog")
    )
)]
pub async fn import_record(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(request): Json<Z3950ImportRequest>,
) -> AppResult<(StatusCode, Json<Item>)> {
    claims.require_write_items()?;

    let item = state
        .services
        .z3950
        .import_record(request.remote_item_id, request.specimens)
        .await?;

    Ok((StatusCode::CREATED, Json(item)))
}
