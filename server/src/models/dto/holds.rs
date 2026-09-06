//! Hold policy and quota DTOs (shared by API and services).

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

/// Global default for how many `pending`/`ready` holds a patron may hold.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HoldsPolicy {
    /// Default cap when the patron's public type has no `maxActiveHolds` override.
    pub max_active_holds: i16,
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
    /// Current `pending` + `ready` holds (copy-level and title-level).
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
