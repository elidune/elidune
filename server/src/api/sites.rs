//! Sites API endpoints

use crate::services::audit;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use crate::{
    error::AppResult,
    models::site::{CreateSite, MergeSites, Site, UpdateSite},
};

use super::{AuthenticatedUser, ClientIp};

/// Build the sources routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/merge", post(merge_sites))
        .route("/sites/:id", get(get_site).put(update_site))
        .route("/sites/:id/archive", post(archive_site))
}

/// Query parameters for listing sources
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SitesQuery {
    /// Include archived sources (default: false)
    pub include_archived: Option<bool>,
}

/// Create a source
#[utoipa::path(
    post,
    path = "/sites",
    tag = "sites",
    security(("bearer_auth" = [])),
    request_body = CreateSite,
    responses(
        (status = 201, description = "Source created", body = Site),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn create_site(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(data): Json<CreateSite>,
) -> AppResult<(StatusCode, Json<Site>)> {
    claims.require_write_items()?;
    let source = state.services.sites.create(&data).await?;
    state.services.audit.log(
        audit::event::SITE_CREATED,
        Some(claims.user_id),
        Some("site"),
        Some(source.id),
        ip,
        Some((&data, &source)),
        audit::AuditLogMeta::success(),
    );
    Ok((StatusCode::CREATED, Json(source)))
}

/// List all sources
#[utoipa::path(
    get,
    path = "/sites",
    tag = "sites",
    security(("bearer_auth" = [])),
    params(SitesQuery),
    responses(
        (status = 200, description = "Sources list", body = Vec<Site>),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn list_sites(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<SitesQuery>,
) -> AppResult<Json<Vec<Site>>> {
    claims.require_read_items()?;
    let sources = state
        .services
        .sites
        .list(query.include_archived.unwrap_or(false))
        .await?;
    Ok(Json(sources))
}

/// Get source by ID
#[utoipa::path(
    get,
    path = "/sites/{id}",
    tag = "sites",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Source ID")),
    responses(
        (status = 200, description = "Source details", body = Site),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn get_site(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Site>> {
    claims.require_read_items()?;
    let source = state.services.sites.get_by_id(id).await?;
    Ok(Json(source))
}

/// Update a source (name and/or default status)
#[utoipa::path(
    post,
    path = "/sites/{id}",
    tag = "sites",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Source ID")),
    request_body = UpdateSite,
    responses(
        (status = 200, description = "Source updated", body = Site),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn update_site(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(data): Json<UpdateSite>,
) -> AppResult<Json<Site>> {
    claims.require_write_items()?;
    let source = state.services.sites.update(id, &data).await?;
    state.services.audit.log(
        audit::event::SITE_UPDATED,
        Some(claims.user_id),
        Some("site"),
        Some(id),
        ip,
        Some((id, &data, &source)),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(source))
}

/// Archive a source (fails if non-archived items are still linked)
#[utoipa::path(
    post,
    path = "/sites/{id}/archive",
    tag = "sites",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Source ID")),
    responses(
        (status = 200, description = "Source archived", body = Site),
        (status = 422, description = "Cannot archive: active items linked")
    )
)]
pub async fn archive_site(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<Site>> {
    claims.require_write_items()?;
    let source = state.services.sites.archive(id).await?;
    state.services.audit.log(
        audit::event::SITE_ARCHIVED,
        Some(claims.user_id),
        Some("site"),
        Some(id),
        ip,
        Some(&source),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(source))
}

/// Merge multiple sources into a new one
#[utoipa::path(
    post,
    path = "/sites/merge",
    tag = "sites",
    security(("bearer_auth" = [])),
    request_body = MergeSites,
    responses(
        (status = 201, description = "New merged source created", body = Site),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn merge_sites(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(data): Json<MergeSites>,
) -> AppResult<(StatusCode, Json<Site>)> {
    claims.require_write_items()?;
    let source = state.services.sites.merge(&data).await?;
    state.services.audit.log(
        audit::event::SITE_MERGED,
        Some(claims.user_id),
        Some("site"),
        Some(source.id),
        ip,
        Some((&data, &source)),
        audit::AuditLogMeta::success(),
    );
    Ok((StatusCode::CREATED, Json(source)))
}
