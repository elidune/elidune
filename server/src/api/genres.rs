//! Genre catalog API (read-only + admin reload).

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use crate::{error::AppResult, services::genre_catalog::GenreApiRow};

use super::{AdminUser, AuthenticatedUser, StaffUser};

/// List active genres with shelf codes, labels, and synonyms.
#[utoipa::path(
    get,
    path = "/genres",
    tag = "genres",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Active genre catalog", body = Vec<GenreApiRow>),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Staff only"),
    )
)]
pub async fn list_genres(State(state): State<crate::AppState>, _staff: StaffUser) -> AppResult<Json<Vec<GenreApiRow>>> {
    let snap = state.services.genre_catalog.current().await?;
    Ok(Json(snap.api_rows()))
}

/// Reload genre catalog from the database (after SQL edits).
#[utoipa::path(
    post,
    path = "/genres/reload",
    tag = "genres",
    security(("bearer_auth" = [])),
    responses(
        (status = 204, description = "Catalog reloaded"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin only"),
    )
)]
pub async fn reload_genres(State(state): State<crate::AppState>, _admin: AdminUser, _user: AuthenticatedUser) -> AppResult<StatusCode> {
    state.services.genre_catalog.reload().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<crate::AppState> {
    Router::new().route("/genres", get(list_genres)).route("/genres/reload", post(reload_genres))
}
