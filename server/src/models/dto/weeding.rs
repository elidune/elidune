//! Staff DTO for the item weeding lifecycle (candidate / withdrawn).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
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

/// Filters for `GET /weeding-candidates`.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WeedingCandidateQuery {
    pub site_id: Option<i64>,
    pub collection_id: Option<i64>,
    /// Shelfmark substring.
    pub call_number: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// One copy marked `candidate` (not yet withdrawn).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WeedingCandidate {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub biblio_title: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub site_name: Option<String>,
    pub weeding_reason: Option<String>,
    pub marked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WeedingCandidateList {
    pub items: Vec<WeedingCandidate>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}
