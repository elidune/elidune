//! Hold policy, quota, and pick-list DTOs (shared by API and services).

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

/// Global holds policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HoldsPolicy {
    /// Default cap when the patron's public type has no `maxActiveHolds` override.
    pub max_active_holds: i16,
    /// Days after `pickStartedAt` before a pick-list row is `overdue`. No automatic expiry.
    /// Omitted on update to leave the stored value unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick_delay_days: Option<i16>,
}

/// Resolved hold slots for one patron (desk + OPAC).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldQuota {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    /// Effective cap (public-type override, else global default).
    pub max_active_holds: i16,
    /// Current `pending` + `to_pick` + `ready` holds (copy-level and title-level).
    pub active_holds: i64,
    /// Slots left before `place_hold` is refused (`max - active`, floored at 0).
    pub remaining: i64,
}

impl HoldQuota {
    #[must_use]
    pub fn from_counts(user_id: i64, max_active_holds: i16, active_holds: i64) -> Self {
        let remaining = (i64::from(max_active_holds) - active_holds).max(0);
        Self {
            user_id,
            max_active_holds,
            active_holds,
            remaining,
        }
    }
}

/// One `to_pick` row for the staff shelf-pick list.
///
/// JSON is camelCase and contains exactly these fields, sorted by `callNumber`.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldPickListItem {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub hold_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub title: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    /// Patron display name (`firstname` + `lastname`).
    pub user_name: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub pickup_site_name: Option<String>,
    pub pick_started_at: chrono::DateTime<chrono::Utc>,
    pub pick_due_at: chrono::DateTime<chrono::Utc>,
    pub overdue: bool,
}

/// Result of checking in a shelf copy that was `to_pick`.
#[derive(Debug, Clone)]
pub struct ToPickCompletion {
    pub hold: crate::models::hold::Hold,
    pub title: Option<String>,
    pub barcode: Option<String>,
    /// Pickup site differs from the copy's site: a `requested` transit was opened.
    /// The hold stays allocated and `pending` until receive marks it `ready`.
    pub transit_started: bool,
    pub destination_site_id: Option<i64>,
    pub destination_site_name: Option<String>,
}

/// A `to_search` copy was scanned and returned to the shelf.
#[derive(Debug, Clone)]
pub struct ShelfFound {
    pub hold: Option<crate::models::hold::Hold>,
    pub title: Option<String>,
    pub barcode: Option<String>,
    pub transit_started: bool,
    pub destination_site_id: Option<i64>,
    pub destination_site_name: Option<String>,
}

/// Staff could not find the `to_pick` copy. The copy is now `to_search`.
#[derive(Debug, Clone)]
pub struct PickNotFoundOutcome {
    pub hold: crate::models::hold::Hold,
    pub item_id: i64,
}
