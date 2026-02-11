//! Visitor counts API endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;

use crate::{
    error::AppResult,
    models::visitor_count::{CreateVisitorCount, VisitorCount, VisitorCountQuery},
};

use super::AuthenticatedUser;

/// List visitor counts
#[utoipa::path(
    get,
    path = "/visitor-counts",
    tag = "visitor_counts",
    security(("bearer_auth" = [])),
    params(VisitorCountQuery),
    responses(
        (status = 200, description = "Visitor counts list", body = Vec<VisitorCount>)
    )
)]
pub async fn list_visitor_counts(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<VisitorCountQuery>,
) -> AppResult<Json<Vec<VisitorCount>>> {
    claims.require_read_settings()?;

    let start = query.start_date.as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let end = query.end_date.as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let counts = state.services.visitor_counts.list(start, end).await?;
    Ok(Json(counts))
}

/// Create a visitor count record
#[utoipa::path(
    post,
    path = "/visitor-counts",
    tag = "visitor_counts",
    security(("bearer_auth" = [])),
    request_body = CreateVisitorCount,
    responses(
        (status = 201, description = "Visitor count created", body = VisitorCount)
    )
)]
pub async fn create_visitor_count(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(data): Json<CreateVisitorCount>,
) -> AppResult<(StatusCode, Json<VisitorCount>)> {
    claims.require_write_settings()?;

    let count = state.services.visitor_counts.create(&data).await?;
    Ok((StatusCode::CREATED, Json(count)))
}

/// Delete a visitor count record
#[utoipa::path(
    delete,
    path = "/visitor-counts/{id}",
    tag = "visitor_counts",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Visitor count ID")),
    responses(
        (status = 204, description = "Visitor count deleted")
    )
)]
pub async fn delete_visitor_count(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
) -> AppResult<StatusCode> {
    claims.require_write_settings()?;

    state.services.visitor_counts.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
