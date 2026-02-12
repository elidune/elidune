//! Source model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Source record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Source {
    pub id: i32,
    pub key: Option<String>,
    pub name: Option<String>,
    pub is_archive: Option<i16>,
    pub archive_date: Option<DateTime<Utc>>,
    pub default: Option<bool>,
}

/// Update source request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSource {
    /// New name for the source
    pub name: Option<String>,
    /// Set as default source
    pub default: Option<bool>,
}

/// Merge sources request
#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeSources {
    /// IDs of sources to merge
    pub source_ids: Vec<i32>,
    /// Name for the new merged source
    pub name: String,
}
