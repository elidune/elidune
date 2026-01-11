//! Settings endpoints

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::error::AppResult;

use super::AuthenticatedUser;

/// Loan settings by media type
#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoanSettings {
    /// Media type code
    pub media_type: String,
    /// Maximum simultaneous loans
    pub max_loans: i16,
    /// Maximum renewals allowed
    pub max_renewals: i16,
    /// Loan duration in days
    pub duration_days: i16,
}

/// Settings response
#[derive(Serialize, ToSchema)]
pub struct SettingsResponse {
    /// Loan settings per media type
    pub loan_settings: Vec<LoanSettings>,
    /// Z39.50 server configurations
    pub z3950_servers: Vec<Z3950ServerConfig>,
}

/// Z39.50 server configuration
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Z3950ServerConfig {
    /// Server ID
    pub id: i32,
    /// Server name
    pub name: String,
    /// Server address
    pub address: String,
    /// Server port
    pub port: i32,
    /// Database name
    pub database: Option<String>,
    /// MARC format (UNIMARC, MARC21)
    pub format: Option<String>,
    /// Whether server is active
    pub is_active: bool,
}

/// Update settings request
#[derive(Deserialize, ToSchema)]
pub struct UpdateSettingsRequest {
    /// Loan settings to update
    pub loan_settings: Option<Vec<LoanSettings>>,
    /// Z39.50 servers to update
    pub z3950_servers: Option<Vec<Z3950ServerConfig>>,
}

/// Get current settings
#[utoipa::path(
    get,
    path = "/settings",
    tag = "settings",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current settings", body = SettingsResponse)
    )
)]
pub async fn get_settings(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<SettingsResponse>> {
    claims.require_read_settings()?;

    let settings = state.services.settings.get_settings().await?;
    Ok(Json(settings))
}

/// Update settings
#[utoipa::path(
    put,
    path = "/settings",
    tag = "settings",
    security(("bearer_auth" = [])),
    request_body = UpdateSettingsRequest,
    responses(
        (status = 200, description = "Settings updated", body = SettingsResponse),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn update_settings(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(request): Json<UpdateSettingsRequest>,
) -> AppResult<Json<SettingsResponse>> {
    claims.require_write_settings()?;

    let settings = state.services.settings.update_settings(request).await?;
    Ok(Json(settings))
}
