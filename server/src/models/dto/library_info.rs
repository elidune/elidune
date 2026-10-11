//! Library info DTOs shared by API and services.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub name: Option<String>,
    pub addr_line1: Option<String>,
    pub addr_line2: Option<String>,
    pub addr_postcode: Option<String>,
    pub addr_city: Option<String>,
    /// ISO 3166-1 alpha-2 code (for example `FR`). Unmapped legacy free text may still be stored.
    pub addr_country: Option<String>,
    pub phones: Vec<String>,
    pub email: Option<String>,
    /// IANA timezone name (for example `Europe/Paris`).
    pub timezone: String,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryInfoRequest {
    pub name: Option<String>,
    pub addr_line1: Option<String>,
    pub addr_line2: Option<String>,
    pub addr_postcode: Option<String>,
    pub addr_city: Option<String>,
    /// Country. French and English names are stored as ISO 3166-1 alpha-2 (`France` → `FR`).
    /// A 2-letter code is kept only when it is in the shared zone table. Any other non-empty
    /// value is a 400 `validation_error`. Omitted or blank leaves the stored value unchanged.
    /// When a country is in effect, `timezone` must be one of that country's zones
    /// (`timezone_country_mismatch`).
    pub addr_country: Option<String>,
    pub phones: Option<Vec<String>>,
    pub email: Option<String>,
    /// IANA timezone name. Omitted fields are left unchanged. Invalid names return 400 `invalid_timezone`.
    pub timezone: Option<String>,
}
