//! Hold (physical item / title queue) model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::biblio::BiblioShort;
use crate::models::user::UserShort;

/// Hold lifecycle status (stored as lowercase strings in DB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HoldStatus {
    Pending,
    Ready,
    Fulfilled,
    Cancelled,
    Expired,
}

impl HoldStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Fulfilled => "fulfilled",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }
}

impl From<String> for HoldStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "ready" => Self::Ready,
            "fulfilled" => Self::Fulfilled,
            "cancelled" => Self::Cancelled,
            "expired" => Self::Expired,
            _ => Self::Pending,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for HoldStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for HoldStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for HoldStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Hold row from database (`holds` table).
///
/// Copy-level holds have `item_id` set. Title-level holds keep `item_id` NULL
/// until fulfillment assigns a concrete copy and moves the row to `ready`.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    pub notes: Option<String>,
}

impl Hold {
    /// Title-level until a concrete copy is assigned.
    #[must_use]
    pub fn is_title_level(&self) -> bool {
        self.item_id.is_none()
    }
}

/// Hold with bibliographic context and user details.
///
/// `biblio.items` is empty for an unassigned title-level hold, otherwise the
/// single physical copy this hold is queued on or trapped for.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldDetails {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub biblio: BiblioShort,
    pub user: Option<UserShort>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    pub notes: Option<String>,
}

/// Default global cap when `circulation_settings` has no row yet.
pub const DEFAULT_MAX_ACTIVE_HOLDS: i16 = 20;
/// Inclusive bounds for stored `max_active_holds` values.
pub const MIN_MAX_ACTIVE_HOLDS: i16 = 1;
pub const MAX_MAX_ACTIVE_HOLDS: i16 = 1000;

/// Create hold request — same reservation type, queued on the biblio.
///
/// `biblio_id` alone waits for any copy. `item_id` pins a specimen on that
/// title’s queue (staff). When both are sent, the item must belong to the biblio.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateHold {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    /// Pickup site (`sources.id`). When different from the copy's current site, fulfillment uses transit.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub notes: Option<String>,
    /// Staff-only: place the hold even when the patron is at/over the active-hold cap.
    #[serde(default)]
    pub force: bool,
}

impl CreateHold {
    #[must_use]
    pub fn for_item(user_id: i64, item_id: i64) -> Self {
        Self {
            user_id,
            item_id: Some(item_id),
            biblio_id: None,
            pickup_site_id: None,
            notes: None,
            force: false,
        }
    }

    #[must_use]
    pub fn for_biblio(user_id: i64, biblio_id: i64) -> Self {
        Self {
            user_id,
            item_id: None,
            biblio_id: Some(biblio_id),
            pickup_site_id: None,
            notes: None,
            force: false,
        }
    }

    #[must_use]
    pub fn is_title_level(&self) -> bool {
        self.item_id.is_none()
    }
}
