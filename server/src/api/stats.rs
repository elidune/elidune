//! Statistics endpoints

use axum::{extract::Query, extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{error::AppResult, models::item::MediaType};

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

/// Time interval for grouping statistics
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Interval {
    Day,
    Week,
    Month,
    Year,
}


/// Advanced loan statistics query parameters
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct LoanStatsQuery {
    /// Start date (ISO 8601 format)
    pub start_date: Option<String>,
    /// End date (ISO 8601 format)
    pub end_date: Option<String>,
    /// Grouping interval (day, week, month, year)
    pub interval: Option<Interval>,
    /// Filter by media type (e.g., 'b', 'bc', 'amc', 'vd')
    pub media_type: Option<MediaType>,
    /// Filter by specific user ID (admin only)
    pub user_id: Option<i32>,
}

/// Loan statistics response with time series data
#[derive(Serialize, ToSchema)]
pub struct LoanStatsResponse {
    /// Total number of loans in the period
    pub total_loans: i64,
    /// Total number of returns in the period
    pub total_returns: i64,
    /// Time series data grouped by interval
    pub time_series: Vec<TimeSeriesEntry>,
    /// Statistics by media type
    pub by_media_type: Vec<StatEntry>,
}

/// Time series entry for loan statistics
#[derive(Serialize, ToSchema)]
pub struct TimeSeriesEntry {
    /// Period label (e.g., "2024-01-15" for day, "2024-W03" for week)
    pub period: String,
    /// Number of loans in this period
    pub loans: i64,
    /// Number of returns in this period
    pub returns: i64,
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

/// Get advanced loan statistics
#[utoipa::path(
    get,
    path = "/stats/loans",
    tag = "stats",
    security(("bearer_auth" = [])),
    params(LoanStatsQuery),
    responses(
        (status = 200, description = "Loan statistics", body = LoanStatsResponse),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn get_loan_stats(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<LoanStatsQuery>,
) -> AppResult<Json<LoanStatsResponse>> {
    claims.require_read_loans()?;

    // Parse dates
    let start_date = query.start_date
        .as_ref()
        .map(|s| DateTime::parse_from_rfc3339(s))
        .transpose()
        .map_err(|_| crate::error::AppError::Validation("Invalid start_date format. Use ISO 8601 (RFC 3339)".to_string()))?
        .map(|dt| dt.with_timezone(&Utc));

    let end_date = query.end_date
        .as_ref()
        .map(|s| DateTime::parse_from_rfc3339(s))
        .transpose()
        .map_err(|_| crate::error::AppError::Validation("Invalid end_date format. Use ISO 8601 (RFC 3339)".to_string()))?
        .map(|dt| dt.with_timezone(&Utc));

    // Check if user can query other users' stats
    let user_id = if let Some(uid) = query.user_id {
        if uid != claims.user_id && !claims.is_admin() {
            return Err(crate::error::AppError::Authorization(
                "Only administrators can query statistics for other users".to_string()
            ));
        }
        Some(uid)
    } else {
        // If not admin and no user_id specified, default to own stats
        if !claims.is_admin() {
            Some(claims.user_id)
        } else {
            None
        }
    };

    let interval = query.interval.unwrap_or(Interval::Day);

    let stats = state.services.stats.get_loan_stats(
        start_date,
        end_date,
        interval,
        query.media_type.as_ref(),
        user_id,
    ).await?;

    Ok(Json(stats))
}
