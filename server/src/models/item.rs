//! Item (physical copy) model and related types.
//!
//! An Item is one borrowable physical copy of a bibliographic record (Biblio).
//! Soft delete is tracked solely via `archived_at` (NULL = active, set = archived).
//! Weeding (`weeding_status`) is orthogonal to `archived_at` and circulation exceptions.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

fn default_borrowable() -> bool {
    true
}

/// Presence of the retired item JSON key `notes`, including `null`.
fn deserialize_legacy_notes_present<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(true)
}

/// Returned as 400 `validation_error` when an item write still sends `notes`.
pub const LEGACY_ITEM_NOTES_MESSAGE: &str =
    "notes is no longer accepted on item writes. Use publicNote and internalNote.";

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

    /// Weeded / discarded. `candidate` (weeding in progress) still circulates,
    /// so a return scan of a candidate follows the normal loan or shelf path.
    #[must_use]
    pub fn blocks_return_scan(self) -> bool {
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

/// Live copy looked up by barcode before a return scan mutates anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnScanCopy {
    pub item_id: i64,
    pub circulation_status: crate::models::circulation::CirculationStatus,
    pub weeding_status: WeedingStatus,
}

impl ReturnScanCopy {
    /// Lost, damaged, missing, to-search, or withdrawn. A return scan must not
    /// change the status, assign a hold, or start transit.
    #[must_use]
    pub fn blocks_return_scan(self) -> bool {
        self.circulation_status.blocks_return_scan() || self.weeding_status.blocks_return_scan()
    }
}

/// Staff availability of one physical copy.
///
/// The server decides this. Clients must not infer it from `borrowed`,
/// `borrowable`, or `circulationStatus`. Readers and the OPAC receive
/// [`crate::models::catalog_view::ItemPublicAvailability`] (`available`,
/// `onLoan`, `onSiteOnly`, `unavailable`) and only for copies that
/// [`included_in_public_catalog`] keeps.
///
/// Precedence, first match wins:
///
/// 1. `onLoan` — a `loans` row for this copy with `returned_at` null.
/// 2. `inTransit` — an `item_transits` row in `requested` or `in_transit`.
/// 3. `onHold` — a hold assigned to this copy with status `to_pick` or `ready`.
/// 4. `toSearch` — `circulation_status` 4 (to be searched; still in the public list).
/// 5. `unavailable` — any other non-borrowable state:
///    reception incomplete (`borrowable` false);
///    lost (`circulation_status` 1);
///    damaged (`circulation_status` 2, including in repair);
///    claimed returned (`circulation_status` 3) when no open loan remains;
///    declared missing (`circulation_status` 5);
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

/// Whether a copy is sent on OPAC and patron catalog projections.
///
/// Omitted entirely (the public count is the length of what remains):
/// lost (`circulation_status` 1), declared missing (`circulation_status` 5),
/// weeding in progress (`candidate`, pilon), and withdrawn.
/// A to-search copy (`circulation_status` 4) stays until it is declared missing or lost.
#[must_use]
pub fn included_in_public_catalog(weeding: WeedingStatus, circulation_status: Option<i16>) -> bool {
    if matches!(weeding, WeedingStatus::Candidate | WeedingStatus::Withdrawn) {
        return false;
    }
    !matches!(
        crate::models::circulation::CirculationStatus::from_db(circulation_status),
        crate::models::circulation::CirculationStatus::Lost
            | crate::models::circulation::CirculationStatus::Missing
    )
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
    /// Item circulation exception (`items.circulation_status`). Read-only on
    /// `PUT /items/{id}`: a present `circulationStatus` key (any JSON value)
    /// returns 422 `circulation_status_read_only` and writes nothing. Use the
    /// dedicated actions (lost, missing, to-search).
    pub circulation_status: Option<i16>,
    /// Orthogonal weeding lifecycle. Not derived from `archived_at`.
    #[serde(default)]
    #[sqlx(default)]
    pub weeding_status: WeedingStatus,
    #[validate(length(max = 200, message = "Weeding reason must be at most 200 characters"))]
    #[serde(default)]
    #[sqlx(default)]
    pub weeding_reason: Option<String>,
    /// Copy note shown to patrons and the OPAC. Blank stores SQL NULL.
    ///
    /// At most [`COPY_NOTE_MAX_CHARS`] Unicode scalar values (`chars().count()`), not bytes.
    /// On `PUT /items/{id}`, null keeps the stored value and `""` clears it.
    /// Create and biblio replace treat null and blank as empty.
    #[validate(length(max = 4000, message = "Public note must be at most 4000 characters"))]
    pub public_note: Option<String>,
    /// Staff-only copy note. Former `items.notes` values live here.
    ///
    /// Same length and null/blank rules as [`Self::public_note`]. Patrons and the OPAC never receive it.
    #[validate(length(max = 4000, message = "Internal note must be at most 4000 characters"))]
    pub internal_note: Option<String>,
    /// Set only when the JSON body includes the retired `notes` key (including `null`).
    ///
    /// Item writes reject that key with 400 `validation_error` and write nothing.
    /// Omitted from responses and from the OpenAPI schema.
    #[serde(
        default,
        rename = "notes",
        skip_serializing,
        deserialize_with = "deserialize_legacy_notes_present"
    )]
    #[sqlx(default)]
    pub legacy_notes_present: bool,
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
    /// Patron id when `availability` is `onHold`. Not part of the item JSON.
    /// Staff catalog views project it into `holdFor`.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_id: Option<i64>,
    /// `users.barcode` for that patron. Not part of the item JSON.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_barcode: Option<String>,
    /// Used only to build `holdFor.initials`. Not part of the item JSON.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_firstname: Option<String>,
    /// Used only to build `holdFor.initials`. Not part of the item JSON.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_lastname: Option<String>,
    /// Active loan `expiry_at`. Not part of the item write contract.
    /// Copied onto the staff catalog view as `dueDate`. The public view omits it.
    #[sqlx(default)]
    #[serde(skip)]
    pub due_date: Option<DateTime<Utc>>,
    /// Library-local calendar date of [`Self::due_date`] while this copy is on loan
    /// and that date is today or later in the library timezone.
    ///
    /// Filled after the catalog read. Cleared once the local due date is before
    /// the local today. Not selected from SQL and not part of the item JSON.
    /// [`crate::models::catalog_view::ItemPublicView`] exposes it as
    /// `expectedReturnDate` only when public availability is `onLoan`.
    #[sqlx(skip)]
    #[serde(skip)]
    pub expected_return_date: Option<NaiveDate>,
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

    /// OPAC and patron catalogs omit lost, missing, and weeding copies.
    #[must_use]
    pub fn included_in_public_catalog(&self) -> bool {
        included_in_public_catalog(self.weeding_status, self.circulation_status)
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

    /// Check both copy notes. Blank values are empty, not over the limit.
    pub fn validate_copy_notes(&self) -> Result<(), String> {
        validate_copy_note("Public note", self.public_note.as_deref())?;
        validate_copy_note("Internal note", self.internal_note.as_deref())?;
        Ok(())
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

/// Maximum length of [`Item::public_note`] and [`Item::internal_note`], in Unicode scalar values.
pub const COPY_NOTE_MAX_CHARS: usize = 4000;

/// `None` means the caller omitted the note (a partial update keeps the stored value).
/// Blank or whitespace becomes `Some("")`, which writers store as SQL NULL.
#[must_use]
pub fn note_write_value(value: Option<String>) -> Option<String> {
    match value {
        None => None,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Some(String::new())
            } else {
                Some(trimmed.to_string())
            }
        }
    }
}

/// Value stored and echoed after a full write. Blank clears to `None`.
#[must_use]
pub fn note_stored_value(value: Option<String>) -> Option<String> {
    note_write_value(value).filter(|text| !text.is_empty())
}

/// Reject a copy note that is longer than [`COPY_NOTE_MAX_CHARS`] after trimming.
pub fn validate_copy_note(label: &str, value: Option<&str>) -> Result<(), String> {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(());
    };
    if text.chars().count() > COPY_NOTE_MAX_CHARS {
        return Err(format!(
            "{label} must be at most {COPY_NOTE_MAX_CHARS} characters"
        ));
    }
    Ok(())
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
    /// `items.circulation_status`. Staff lists expose this so lost and missing
    /// stay distinct from a temporary `unavailable`.
    #[serde(default)]
    #[sqlx(default)]
    pub circulation_status: Option<i16>,
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
    /// Patron id when [`Self::availability`] is [`ItemAvailability::OnHold`]. Not serialized.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_id: Option<i64>,
    /// `users.barcode` for that patron. Not serialized.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_barcode: Option<String>,
    /// Used only to build initials. Not serialized.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_firstname: Option<String>,
    /// Used only to build initials. Not serialized.
    #[sqlx(default)]
    #[serde(skip)]
    pub hold_user_lastname: Option<String>,
}

impl ItemShort {
    /// OPAC and patron catalogs omit lost, missing, and weeding copies.
    #[must_use]
    pub fn included_in_public_catalog(&self) -> bool {
        included_in_public_catalog(self.weeding_status, self.circulation_status)
    }
}

impl From<Item> for ItemShort {
    fn from(item: Item) -> Self {
        Self {
            id: item.id.unwrap_or(0),
            barcode: item.barcode,
            call_number: item.call_number,
            borrowable: item.borrowable,
            circulation_status: item.circulation_status,
            weeding_status: item.weeding_status,
            source_name: item.source_name,
            borrowed: item.borrowed,
            availability: item.availability,
            can_hold_item: item.can_hold_item,
            hold_user_id: item.hold_user_id,
            hold_user_barcode: item.hold_user_barcode,
            hold_user_firstname: item.hold_user_firstname,
            hold_user_lastname: item.hold_user_lastname,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        included_in_public_catalog, note_stored_value, note_write_value, Item, ItemAvailability,
        WeedingStatus, COPY_NOTE_MAX_CHARS,
    };

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

    #[test]
    fn public_catalog_omits_lost_missing_and_weeding_only() {
        let shelf = WeedingStatus::OnShelf;
        assert!(included_in_public_catalog(shelf, None));
        assert!(included_in_public_catalog(shelf, Some(0)));
        assert!(included_in_public_catalog(shelf, Some(2)));
        assert!(included_in_public_catalog(shelf, Some(4)));
        assert!(!included_in_public_catalog(shelf, Some(1)));
        assert!(!included_in_public_catalog(shelf, Some(5)));
        assert!(!included_in_public_catalog(WeedingStatus::Candidate, None));
        assert!(!included_in_public_catalog(
            WeedingStatus::Withdrawn,
            Some(0)
        ));
    }

    #[test]
    fn copy_note_write_trims_and_blank_clears() {
        assert_eq!(note_write_value(None), None);
        assert_eq!(note_write_value(Some("  ".into())), Some(String::new()));
        assert_eq!(
            note_write_value(Some("  CD missing  ".into())),
            Some("CD missing".into())
        );
        assert_eq!(note_stored_value(Some("  ".into())), None);
        assert_eq!(
            note_stored_value(Some(" 2-disc box set ".into())),
            Some("2-disc box set".into())
        );
        assert!(Item {
            public_note: Some("x".repeat(4001)),
            internal_note: None,
            ..sample_item()
        }
        .validate_copy_notes()
        .is_err());
        assert!(sample_item().validate_copy_notes().is_ok());
    }

    #[test]
    fn copy_note_limit_counts_unicode_scalars_not_bytes() {
        use validator::Validate;

        let accepted = "é".repeat(COPY_NOTE_MAX_CHARS);
        assert_eq!(accepted.chars().count(), COPY_NOTE_MAX_CHARS);
        assert!(accepted.len() > COPY_NOTE_MAX_CHARS);
        let ok = Item {
            public_note: Some(accepted.clone()),
            internal_note: Some(accepted.clone()),
            ..sample_item()
        };
        assert!(ok.validate_copy_notes().is_ok());
        assert!(ok.validate().is_ok());

        let rejected = "é".repeat(COPY_NOTE_MAX_CHARS + 1);
        assert_eq!(rejected.chars().count(), COPY_NOTE_MAX_CHARS + 1);
        let too_long = Item {
            public_note: Some(rejected.clone()),
            ..sample_item()
        };
        assert!(too_long.validate_copy_notes().is_err());
        assert!(too_long.validate().is_err());
        let internal_too_long = Item {
            internal_note: Some(rejected),
            ..sample_item()
        };
        assert!(internal_too_long.validate().is_err());
    }

    #[test]
    fn legacy_notes_key_is_present_including_null_and_is_not_serialized() {
        let with_text: Item = serde_json::from_str(r#"{"notes":"legacy"}"#).unwrap();
        assert!(with_text.legacy_notes_present);
        let with_null: Item = serde_json::from_str(r#"{"notes":null}"#).unwrap();
        assert!(with_null.legacy_notes_present);
        let absent: Item = serde_json::from_str("{}").unwrap();
        assert!(!absent.legacy_notes_present);
        let body = serde_json::to_value(with_text).unwrap();
        assert!(body.get("notes").is_none());
        assert!(body.get("legacyNotesPresent").is_none());
    }

    fn sample_item() -> Item {
        Item {
            id: None,
            biblio_id: None,
            source_id: None,
            barcode: None,
            call_number: None,
            volume_designation: None,
            place: None,
            borrowable: true,
            circulation_status: None,
            weeding_status: WeedingStatus::OnShelf,
            weeding_reason: None,
            public_note: None,
            internal_note: None,
            price: None,
            price_deferred: false,
            created_at: None,
            updated_at: None,
            archived_at: None,
            source_name: None,
            borrowed: false,
            loan_id: None,
            availability: ItemAvailability::Available,
            can_hold_item: true,
            hold_user_id: None,
            hold_user_barcode: None,
            hold_user_firstname: None,
            hold_user_lastname: None,
            due_date: None,
            expected_return_date: None,
            on_hold: false,
            legacy_notes_present: false,
        }
    }
}
