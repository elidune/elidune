//! Item (physical copy) model and related types.
//!
//! An Item is one borrowable physical copy of a bibliographic record (Biblio).
//! Soft delete is tracked solely via `archived_at` (NULL = active, set = archived).
//! Weeding (`weeding_status`) is orthogonal to `archived_at` and circulation exceptions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

fn default_borrowable() -> bool {
    true
}

/// Item weeding lifecycle. Stored as snake_case strings; serialized as camelCase.
///
/// Orthogonal to [`crate::models::circulation::CirculationStatus`] and `archived_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum WeedingStatus {
    #[default]
    OnShelf,
    Candidate,
    Withdrawn,
}

impl WeedingStatus {
    pub const CHECKOUT_BLOCKED: &'static str = "Item is withdrawn and cannot be checked out";
    pub const RENEW_BLOCKED: &'static str = "Item is withdrawn and cannot be renewed";
    pub const HOLD_BLOCKED: &'static str = "Item is withdrawn and cannot be placed on hold";
    pub const TITLE_HOLD_BLOCKED: &'static str = "No circulable copies remain for this title";

    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::OnShelf => "on_shelf",
            Self::Candidate => "candidate",
            Self::Withdrawn => "withdrawn",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "candidate" => Self::Candidate,
            "withdrawn" => Self::Withdrawn,
            _ => Self::OnShelf,
        }
    }

    /// Candidate stays on the shelf and circulates; only withdrawn is blocked.
    #[must_use]
    pub fn blocks_circulation(self) -> bool {
        matches!(self, Self::Withdrawn)
    }

    /// Biblio-level mark: withdrawn only when every remaining copy is withdrawn.
    #[must_use]
    pub fn for_copies<I>(statuses: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        let mut any = false;
        for status in statuses {
            any = true;
            if status != Self::Withdrawn {
                return Self::OnShelf;
            }
        }
        if any {
            Self::Withdrawn
        } else {
            Self::OnShelf
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for WeedingStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for WeedingStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from_db_str(&s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for WeedingStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str().to_string(), buf)
    }
}

/// Full item (physical copy) model from database.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(default)]
    pub id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub source_id: Option<i64>,
    #[validate(length(max = 100, message = "Barcode must be at most 100 characters"))]
    pub barcode: Option<String>,
    #[validate(length(max = 200, message = "Call number must be at most 200 characters"))]
    pub call_number: Option<String>,
    #[validate(length(
        max = 100,
        message = "Volume designation must be at most 100 characters"
    ))]
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    #[serde(default = "default_borrowable")]
    pub borrowable: bool,
    pub circulation_status: Option<i16>,
    /// Orthogonal weeding lifecycle. Not derived from `archived_at`.
    #[serde(default)]
    #[sqlx(default)]
    pub weeding_status: WeedingStatus,
    #[validate(length(max = 200, message = "Weeding reason must be at most 200 characters"))]
    #[serde(default)]
    #[sqlx(default)]
    pub weeding_reason: Option<String>,
    pub notes: Option<String>,
    pub price: Option<String>,
    /// Write-only: treat a missing price as an explicit deferral when enabling circulation.
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub price_deferred: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub borrowed: bool,
    /// Active loan id when `borrowed` is true (staff circulation lookup).
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loan_id: Option<i64>,
}

impl Item {
    pub fn is_available(&self) -> bool {
        self.archived_at.is_none()
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// Typed item circulation state (lost / damaged / claimed-returned).
    #[must_use]
    pub fn circulation_state(&self) -> crate::models::circulation::CirculationStatus {
        crate::models::circulation::CirculationStatus::from_db(self.circulation_status)
    }

    /// Weeding policy for this copy (candidate circulates; withdrawn does not).
    #[must_use]
    pub fn weeding_blocks_circulation(&self) -> bool {
        self.weeding_status.blocks_circulation()
    }

    /// Whether barcode, site, and price (or an explicit deferral) are present.
    ///
    /// Used for acquisitions receipt and when enabling circulation on an incomplete copy.
    #[must_use]
    pub fn ready_to_circulate(
        barcode: Option<&str>,
        source_id: Option<i64>,
        source_name: Option<&str>,
        price: Option<&str>,
        price_deferred: bool,
    ) -> bool {
        let barcode_ok = nonempty(barcode).is_some();
        let site_ok = source_id.is_some() || nonempty(source_name).is_some();
        let price_ok = price_deferred || nonempty(price).is_some();
        barcode_ok && site_ok && price_ok
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// Short item (physical copy) representation for lists
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemShort {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub borrowable: bool,
    #[serde(default)]
    #[sqlx(default)]
    pub weeding_status: WeedingStatus,
    pub source_name: Option<String>,
    #[sqlx(skip)]
    #[serde(default)]
    pub borrowed: bool,
}

impl From<Item> for ItemShort {
    fn from(item: Item) -> Self {
        Self {
            id: item.id.unwrap_or(0),
            barcode: item.barcode,
            call_number: item.call_number,
            borrowable: item.borrowable,
            weeding_status: item.weeding_status,
            source_name: item.source_name,
            borrowed: item.borrowed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, WeedingStatus};

    #[test]
    fn weeding_candidate_circulates_withdrawn_blocks() {
        assert!(!WeedingStatus::OnShelf.blocks_circulation());
        assert!(!WeedingStatus::Candidate.blocks_circulation());
        assert!(WeedingStatus::Withdrawn.blocks_circulation());
        assert_eq!(
            WeedingStatus::for_copies([WeedingStatus::OnShelf, WeedingStatus::Withdrawn]),
            WeedingStatus::OnShelf
        );
        assert_eq!(
            WeedingStatus::for_copies([WeedingStatus::Withdrawn, WeedingStatus::Withdrawn]),
            WeedingStatus::Withdrawn
        );
        assert_eq!(WeedingStatus::for_copies([]), WeedingStatus::OnShelf);
        assert_eq!(
            WeedingStatus::from_db_str("withdrawn"),
            WeedingStatus::Withdrawn
        );
        assert_eq!(WeedingStatus::Candidate.as_db_str(), "candidate");
    }

    #[test]
    fn ready_to_circulate_requires_barcode_site_and_price_or_deferral() {
        assert!(!Item::ready_to_circulate(None, None, None, None, false));
        assert!(!Item::ready_to_circulate(
            Some("B1"),
            None,
            None,
            Some("10.00"),
            false
        ));
        assert!(!Item::ready_to_circulate(
            Some("B1"),
            Some(1),
            None,
            None,
            false
        ));
        assert!(!Item::ready_to_circulate(
            None,
            Some(1),
            None,
            Some("10.00"),
            false
        ));
        assert!(Item::ready_to_circulate(
            Some("B1"),
            Some(1),
            None,
            Some("10.00"),
            false
        ));
        assert!(Item::ready_to_circulate(
            Some("B1"),
            None,
            Some("Main"),
            None,
            true
        ));
        assert!(!Item::ready_to_circulate(
            Some("  "),
            Some(1),
            None,
            Some("10.00"),
            false
        ));
    }
}
