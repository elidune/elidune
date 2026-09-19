//! Staff DTO for the item weeding lifecycle (candidate / withdrawn).

use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

use crate::models::item::WeedingStatus;

/// Set weeding status on a physical copy. Withdraw cancels item-targeted holds.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SetWeedingRequest {
    pub status: WeedingStatus,
    /// Optional short reason (max 200 characters). Empty/whitespace is stored as null.
    #[validate(length(max = 200, message = "Weeding reason must be at most 200 characters"))]
    pub reason: Option<String>,
}
