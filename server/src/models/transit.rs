//! Inter-site item transit (hold fulfillment movement).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::hold::Hold;

/// Transit lifecycle (stored as lowercase strings in DB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TransitStatus {
    Requested,
    InTransit,
    Received,
    Cancelled,
}

impl TransitStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::InTransit => "in_transit",
            Self::Received => "received",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Requested | Self::InTransit)
    }

    #[must_use]
    pub fn blocks_checkout(self, force: bool) -> bool {
        match self {
            Self::InTransit => true,
            Self::Requested => !force,
            Self::Received | Self::Cancelled => false,
        }
    }
}

impl From<String> for TransitStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "in_transit" => Self::InTransit,
            "received" => Self::Received,
            "cancelled" => Self::Cancelled,
            _ => Self::Requested,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for TransitStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TransitStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for TransitStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Item movement row (`item_transits`).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemTransit {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub hold_id: Option<i64>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub from_source_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub to_source_id: i64,
    pub status: TransitStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub received_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub shipped_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub received_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub cancelled_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub reversed_from_id: Option<i64>,
}

/// Staff request to start (and optionally ship) a hold transit.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransit {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub from_source_id: Option<i64>,
    /// Destination. Must match `hold.pickupSiteId` when that field is already set.
    /// If the hold has no pickup site yet, this value is stored on the hold.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub to_source_id: Option<i64>,
    pub notes: Option<String>,
    /// When true, skip `requested` and mark the copy in transit immediately.
    #[serde(default)]
    pub ship: bool,
}

/// Ship / receive / cancel notes.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitActionRequest {
    pub notes: Option<String>,
    /// On cancel: create a reverse transit back to the origin (default true when in transit).
    pub reverse: Option<bool>,
}

/// Transit plus the hold it fulfills (when linked).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitWithHold {
    pub transit: ItemTransit,
    pub hold: Option<Hold>,
}

#[cfg(test)]
mod tests {
    use super::TransitStatus;

    #[test]
    fn in_transit_always_blocks_checkout() {
        assert!(TransitStatus::InTransit.blocks_checkout(false));
        assert!(TransitStatus::InTransit.blocks_checkout(true));
    }

    #[test]
    fn requested_blocks_normal_checkout_but_allows_force() {
        assert!(TransitStatus::Requested.blocks_checkout(false));
        assert!(!TransitStatus::Requested.blocks_checkout(true));
    }

    #[test]
    fn terminal_states_do_not_block() {
        assert!(!TransitStatus::Received.blocks_checkout(false));
        assert!(!TransitStatus::Cancelled.blocks_checkout(false));
    }
}
