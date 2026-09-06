//! Transit list query DTOs.

use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use utoipa::{IntoParams, ToSchema};

use crate::models::transit::TransitStatus;

/// Query parameters for `GET /transits`.
#[serde_as]
#[derive(Debug, Default, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ListTransitsQuery {
    pub status: Option<TransitStatus>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub hold_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub to_source_id: Option<i64>,
    /// Page number (1-based, default 1).
    pub page: Option<i64>,
    /// Page size (default 50, max 200).
    pub per_page: Option<i64>,
}
