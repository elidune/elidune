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
}

/// Partial update for `account_types` (admin only). Omit a field to leave it unchanged.
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
}
