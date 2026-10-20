//! Site model (physical catalog location)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Physical site record
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub key: Option<String>,
    pub name: Option<String>,
    pub is_archive: Option<i16>,
    pub archived_at: Option<DateTime<Utc>>,
    pub default: Option<bool>,
}

/// Create site request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSite {
    /// Site name
    pub name: String,
    /// Set as default site
    pub default: Option<bool>,
}

/// Update site request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSite {
    /// New name for the site
    pub name: Option<String>,
    /// Set as default site
    pub default: Option<bool>,
}

/// Merge sites request
#[serde_as]
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MergeSites {
    /// IDs of sites to merge
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub site_ids: Vec<i64>,
    /// Name for the new merged site
    pub name: String,
}
