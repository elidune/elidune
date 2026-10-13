//! Item (physical copy) model and related types.
//!
//! An Item is one borrowable physical copy of a bibliographic record (Biblio).
//! Soft delete is tracked solely via `archived_at` (NULL = active, set = archived).
//! Weeding (`weeding_status`) is orthogonal to `archived_at` and circulation exceptions.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

fn default_borrowable() -> bool {
    true
}

/// `items.price` as a decimal euro amount (`"12.50"`, `"15"`, `"0"`).
///
/// Blank and unparseable text are `None`. Zero and negative amounts are kept.
/// Replacement billing drops non-positive amounts after this parse.
#[must_use]
pub fn parse_stored_item_price(price: Option<&str>) -> Option<Decimal> {
    let raw = price?.trim();
    if raw.is_empty() {
        return None;
    }
    Decimal::from_str_exact(raw).or_else(|_| raw.parse()).ok()
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

/// Staff availability of one physical copy.
///
/// The server decides this. Clients must not infer it from `borrowed`,
/// `borrowable`, or `circulationStatus`. Readers and the OPAC receive
/// [`crate::models::catalog_view::ItemPublicAvailability`] instead: `toSearch`
/// and `unavailable` are both `unavailable`, with no reason.
///
/// Precedence, first match wins:
///
/// 1. `onLoan` — a `loans` row for this copy with `returned_at` null.
/// 2. `inTransit` — an `item_transits` row in `requested` or `in_transit`.
/// 3. `onHold` — a hold assigned to this copy with status `to_pick` or `ready`.
/// 4. `toSearch` — `circulation_status` 4 (missing from the shelf).
/// 5. `unavailable` — any other non-borrowable state:
///    reception incomplete (`borrowable` false);
///    lost (`circulation_status` 1);
///    damaged (`circulation_status` 2);
///    claimed returned (`circulation_status` 3) when no open loan remains;
///    withdrawn (`weeding_status` `withdrawn`).
///    Weeding `candidate` does not by itself make a copy unavailable.
/// 6. `available` — on the shelf and borrowable.
///
/// `canHoldItem` is true only for `available` and `onLoan`, and never when the
/// copy is withdrawn. Item-level hold is the “reserve this copy” action.
/// A title-level hold is separate and stays possible while this is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum ItemAvailability {
    #[default]
    Available,
    OnHold,
    InTransit,
    OnLoan,
    ToSearch,
    Unavailable,
}

impl ItemAvailability {
    #[must_use]
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::OnHold => "onHold",
            Self::InTransit => "inTransit",
            Self::OnLoan => "onLoan",
            Self::ToSearch => "toSearch",
            Self::Unavailable => "unavailable",
        }
    }

    #[must_use]
    pub fn from_api_str(value: &str) -> Self {
        match value {
            "onHold" => Self::OnHold,
            "inTransit" => Self::InTransit,
            "onLoan" => Self::OnLoan,
            "toSearch" => Self::ToSearch,
            "unavailable" => Self::Unavailable,
            "available" => Self::Available,
            _ => Self::Unavailable,
        }
    }

    /// Shelf copy or copy on loan. Assigned, moving, missing, and blocked copies are not.
    #[must_use]
    pub fn allows_item_hold(self) -> bool {
        matches!(self, Self::Available | Self::OnLoan)
    }

    /// Fallback when a response is built without the catalog availability joins.
    ///
    /// An open loan wins, then `toSearch`, then other non-borrowable states.
    /// Holds and transits are visible only to the catalog SQL.
    #[must_use]
    pub fn from_local_flags(
        borrowed: bool,
        borrowable: bool,
        circulation_status: Option<i16>,
        weeding: WeedingStatus,
    ) -> Self {
        if borrowed {
            return Self::OnLoan;
        }
        let circulation =
            crate::models::circulation::CirculationStatus::from_db(circulation_status);
        if circulation == crate::models::circulation::CirculationStatus::ToSearch {
            return Self::ToSearch;
        }
        if !borrowable || weeding.blocks_circulation() || circulation.blocks_circulation() {
            return Self::Unavailable;
        }
        Self::Available
    }

    #[must_use]
    pub fn can_hold_item(self, weeding: WeedingStatus) -> bool {
        self.allows_item_hold() && !weeding.blocks_circulation()
    }

    /// `(availability, canHoldItem)` for a copy that was not loaded by the catalog query.
    #[must_use]
    pub fn local_pair(
        borrowed: bool,
        borrowable: bool,
        circulation_status: Option<i16>,
        weeding: WeedingStatus,
    ) -> (Self, bool) {
        let availability =
            Self::from_local_flags(borrowed, borrowable, circulation_status, weeding);
        (availability, availability.can_hold_item(weeding))
    }
}

impl sqlx::Type<sqlx::Postgres> for ItemAvailability {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ItemAvailability {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from_api_str(&s))
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
    /// Server-computed copy state. See [`ItemAvailability`] for the mapping.
    #[sqlx(default)]
    #[serde(default)]
    pub availability: ItemAvailability,
    /// False unless an item-level hold is meaningful for [`Self::availability`].
    ///
    /// True for `available` and `onLoan` when the copy is not withdrawn.
    /// False for `onHold`, `inTransit`, `toSearch`, and `unavailable`, so the
    /// client does not offer “reserve this copy” on an assigned copy.
    #[sqlx(default)]
    #[serde(default)]
    pub can_hold_item: bool,
    /// Patron id when `availability` is `onHold`. Present on staff responses only.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_user_id: Option<i64>,
    /// Patron display name (`firstname` + `lastname`) when `availability` is `onHold`.
    /// Present on staff responses only. Readers and the OPAC never receive it.
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_user_name: Option<String>,
    /// Active loan `expiry_at`. Not part of the item write contract.
    /// Copied onto catalog views as `dueDate`.
    #[sqlx(default)]
    #[serde(skip)]
    pub due_date: Option<DateTime<Utc>>,
    /// True only when this copy is assigned to a hold.
    ///
    /// That is a hold in `to_pick` or `ready` with `holds.item_id` equal to
    /// this copy, or an active `item_transits` row (`requested` or
    /// `in_transit`) whose `hold_id` is set. An unassigned title hold
    /// (`pending`, `item_id` null) does not set this on any copy, so a shelved
    /// copy stays `available`. Independent of [`ItemAvailability`]: transit
    /// precedence can leave `availability` at `inTransit` while this is true.
    /// Not part of the item write contract. Catalog views expose it as `onHold`.
    #[sqlx(default)]
    #[serde(skip)]
    pub on_hold: bool,
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
    /// Server-computed copy state. See [`ItemAvailability`].
    #[sqlx(default)]
    #[serde(default)]
    pub availability: ItemAvailability,
    /// See [`Item::can_hold_item`].
    #[sqlx(default)]
    #[serde(default)]
    pub can_hold_item: bool,
    /// Present on staff catalog views only, and only when [`Self::availability`] is [`ItemAvailability::OnHold`].
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_user_id: Option<i64>,
    /// Present on staff catalog views only, and only when [`Self::availability`] is [`ItemAvailability::OnHold`].
    #[sqlx(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_user_name: Option<String>,
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
            availability: item.availability,
            can_hold_item: item.can_hold_item,
            hold_user_id: item.hold_user_id,
            hold_user_name: item.hold_user_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Item, ItemAvailability, WeedingStatus};

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

    #[test]
    fn availability_api_names_and_local_flags() {
        assert_eq!(ItemAvailability::Available.as_api_str(), "available");
        assert_eq!(ItemAvailability::OnHold.as_api_str(), "onHold");
        assert_eq!(ItemAvailability::InTransit.as_api_str(), "inTransit");
        assert_eq!(ItemAvailability::OnLoan.as_api_str(), "onLoan");
        assert_eq!(ItemAvailability::ToSearch.as_api_str(), "toSearch");
        assert_eq!(ItemAvailability::Unavailable.as_api_str(), "unavailable");
        assert_eq!(
            ItemAvailability::from_api_str("onHold"),
            ItemAvailability::OnHold
        );
        assert_eq!(
            ItemAvailability::from_api_str("mystery"),
            ItemAvailability::Unavailable
        );

        let shelf = WeedingStatus::OnShelf;
        assert_eq!(
            ItemAvailability::from_local_flags(false, true, None, shelf),
            ItemAvailability::Available
        );
        assert!(ItemAvailability::Available.can_hold_item(shelf));
        assert_eq!(
            ItemAvailability::from_local_flags(true, true, None, shelf),
            ItemAvailability::OnLoan
        );
        assert!(ItemAvailability::OnLoan.allows_item_hold());
        assert_eq!(
            ItemAvailability::from_local_flags(false, true, Some(4), shelf),
            ItemAvailability::ToSearch
        );
        assert!(!ItemAvailability::ToSearch.allows_item_hold());
        assert_eq!(
            ItemAvailability::from_local_flags(false, true, Some(1), shelf),
            ItemAvailability::Unavailable
        );
        assert_eq!(
            ItemAvailability::from_local_flags(false, false, Some(0), shelf),
            ItemAvailability::Unavailable
        );
        assert_eq!(
            ItemAvailability::from_local_flags(false, true, Some(0), WeedingStatus::Withdrawn),
            ItemAvailability::Unavailable
        );
        assert!(!ItemAvailability::OnLoan.can_hold_item(WeedingStatus::Withdrawn));
        assert_eq!(
            ItemAvailability::from_local_flags(false, true, Some(0), WeedingStatus::Candidate),
            ItemAvailability::Available
        );
        assert!(!ItemAvailability::OnHold.allows_item_hold());
        assert!(!ItemAvailability::InTransit.allows_item_hold());
        assert!(!ItemAvailability::Unavailable.allows_item_hold());

        let json = serde_json::to_value(ItemAvailability::OnHold).unwrap();
        assert_eq!(json, "onHold");
    }
}
