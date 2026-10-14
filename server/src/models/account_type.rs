//! Library account types (`account_types` table): staff/patron role definitions and per-domain rights.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// One row from `account_types` (code is immutable via this API).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountTypeDefinition {
    pub code: String,
    pub name: Option<String>,
    pub items_rights: Option<String>,
    pub users_rights: Option<String>,
    pub loans_rights: Option<String>,
    pub items_archive_rights: Option<String>,
    pub holds_rights: Option<String>,
    pub settings_rights: Option<String>,
    pub events_rights: Option<String>,
    pub acquisitions_rights: Option<String>,
    pub weeding_rights: Option<String>,
    pub weeding_cancel_rights: Option<String>,
    /// Write may force a held copy and an expired subscription. Distinct from settings.
    pub circulation_override_rights: Option<String>,
    /// Built-in types (`guest`, `reader`, `librarian`, `admin`, `group`) are true.
    pub is_system: bool,
}

/// Create a custom account type. Omitted rights are stored as `n`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountType {
    pub code: String,
    pub name: String,
    pub items_rights: Option<String>,
    pub users_rights: Option<String>,
    pub loans_rights: Option<String>,
    pub items_archive_rights: Option<String>,
    pub holds_rights: Option<String>,
    pub settings_rights: Option<String>,
    pub events_rights: Option<String>,
    pub acquisitions_rights: Option<String>,
    pub weeding_rights: Option<String>,
    pub weeding_cancel_rights: Option<String>,
    pub circulation_override_rights: Option<String>,
}

/// Partial update for `account_types`. Omit a field to leave it unchanged.
/// `code` and `is_system` are immutable.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountTypeDefinition {
    pub name: Option<String>,
    pub items_rights: Option<String>,
    pub users_rights: Option<String>,
    pub loans_rights: Option<String>,
    pub items_archive_rights: Option<String>,
    pub holds_rights: Option<String>,
    pub settings_rights: Option<String>,
    pub events_rights: Option<String>,
    pub acquisitions_rights: Option<String>,
    pub weeding_rights: Option<String>,
    pub weeding_cancel_rights: Option<String>,
    pub circulation_override_rights: Option<String>,
}
