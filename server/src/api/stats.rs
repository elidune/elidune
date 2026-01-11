//! Statistics endpoints

use axum::{extract::State, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::AppResult;

use super::AuthenticatedUser;

/// Statistics response
#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    /// Item statistics
    pub items: ItemStats,
    /// User statistics
    pub users: UserStats,
    /// Loan statistics
    pub loans: LoanStats,
}

#[derive(Serialize, ToSchema)]
pub struct ItemStats {
    /// Total number of items
    pub total: i64,
    /// Items by media type
    pub by_media_type: Vec<StatEntry>,
    /// Items by public type
    pub by_public_type: Vec<StatEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct UserStats {
    /// Total number of users
    pub total: i64,
    /// Users with active loans
    pub active: i64,
    /// Users by account type
    pub by_account_type: Vec<StatEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct LoanStats {
    /// Active loans
    pub active: i64,
    /// Overdue loans
    pub overdue: i64,
    /// Items returned today
    pub returned_today: i64,
    /// Loans by media type
    pub by_media_type: Vec<StatEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct StatEntry {
    /// Label
    pub label: String,
    /// Value
    pub value: i64,
}

/// Get library statistics
#[utoipa::path(
    get,
    path = "/stats",
    tag = "stats",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Library statistics", body = StatsResponse)
    )
)]
pub async fn get_stats(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<StatsResponse>> {
    claims.require_read_items()?;

    let stats = state.services.stats.get_stats().await?;
    Ok(Json(stats))
}
