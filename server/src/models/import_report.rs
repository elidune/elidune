//! Import report models for ISBN deduplication logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Result of an ISBN duplicate lookup before import.
#[derive(Debug, Clone)]
pub struct DuplicateCandidate {
    pub item_id: i32,
    pub archived_at: Option<DateTime<Utc>>,
    /// Number of active (non-archived) specimens linked to this item.
    pub specimen_count: i64,
}

/// What happened during import.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportAction {
    Created,
    MergedBibliographic,
    ReplacedArchived,
    ReplacedConfirmed,
}

/// Report returned alongside the imported/updated item.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportReport {
    pub action: ImportAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_id: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body returned on 409 when confirmation is required.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DuplicateConfirmationRequired {
    pub code: String,
    pub existing_id: i32,
    pub message: String,
}
