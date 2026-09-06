//! Item (physical copy) model and related types.
//!
//! An Item is one borrowable physical copy of a bibliographic record (Biblio).
//! Soft delete is tracked solely via `archived_at` (NULL = active, set = archived).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

fn default_borrowable() -> bool {
    true
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
            source_name: item.source_name,
            borrowed: item.borrowed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Item;

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
