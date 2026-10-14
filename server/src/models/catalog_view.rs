//! Staff and public JSON shapes for catalog copies and the user summaries embedded with them.
//!
//! Handlers pick one type with [`sees_internal_catalog`] (rights, not account type). The same
//! check decides whether lost, missing, weeding-in-progress, and withdrawn copies stay in the
//! payload. The public structs do not contain staff fields, so a reader response cannot include
//! them as blank options.
//!
//! A patron's own loans and holds are [`crate::models::my_account::MyLoanView`] and
//! [`crate::models::my_account::MyHoldView`] from `GET /me/loans` and `GET /me/holds`.
//! Those payloads are not these catalog embeds.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use super::author::{Author, Function};
use super::biblio::{
    AudienceType, Biblio, BiblioShort, Collection, Edition, Isbn, MediaType, Serie,
};
use super::hold::{HoldDetails, HoldStatus};
use super::item::{Item, ItemAvailability, ItemShort, WeedingStatus};
use super::loan::LoanDetails;
use super::user::{AccountTypeSlug, Rights, UserClaims, UserRights, UserShort, UserStatus};
use super::Language;

/// Staff catalog JSON (internal copy data) versus the public projection.
///
/// `items_rights` read is the catalog read right patrons share with staff. It does not
/// select the staff view. Internal data is selected when the caller has any of:
/// catalog write (`items_rights` write), weeding read, loans read, holds read
/// (not own), users read, or copy-deletion write (`items_archive_rights` write).
/// Account type is ignored.
///
/// `holdFor` on a staff copy requires `holds_rights` read or write. Holds rights
/// alone return id, card number, and initials. Holds rights plus users read return
/// the staff patron. Users read or loans read without holds rights leave `holdFor`
/// empty. Patrons and the OPAC stay on the public view, which omits lost, missing,
/// weeding-in-progress, and withdrawn copies.
#[must_use]
pub fn sees_internal_catalog(claims: &UserClaims) -> bool {
    let rights = &claims.rights;
    rights.items_rights.rank() >= Rights::Write.rank()
        || rights.weeding_rights.rank() >= Rights::Read.rank()
        || rights.loans_rights.rank() >= Rights::Read.rank()
        || rights.holds_rights.rank() >= Rights::Read.rank()
        || rights.users_rights.rank() >= Rights::Read.rank()
        || rights.items_archive_rights.rank() >= Rights::Write.rank()
}

/// Availability returned to readers and the OPAC.
///
/// Four values, and no reason: `available`, `onLoan`, `onSiteOnly` (not borrowable),
/// `unavailable` (damaged, to be searched, in transit, in repair, or reserved).
/// Lost, declared missing, weeding-in-progress, and withdrawn copies are omitted
/// before this value is chosen. Staff [`ItemAvailability`] is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ItemPublicAvailability {
    Available,
    OnLoan,
    /// Excluded from loan. The copy stays on site.
    OnSiteOnly,
    Unavailable,
}

impl ItemPublicAvailability {
    /// Public availability of a copy that is still included in the catalog list.
    ///
    /// An open loan wins. Temporary states (damaged, to search, claimed returned,
    /// in transit, reserved, in repair) are [`Self::Unavailable`]. A copy that is
    /// simply not borrowable is [`Self::OnSiteOnly`].
    #[must_use]
    pub fn for_copy(
        availability: ItemAvailability,
        borrowable: bool,
        circulation_status: Option<i16>,
    ) -> Self {
        if availability == ItemAvailability::OnLoan {
            return Self::OnLoan;
        }
        let circulation =
            crate::models::circulation::CirculationStatus::from_db(circulation_status);
        if matches!(
            availability,
            ItemAvailability::InTransit | ItemAvailability::OnHold | ItemAvailability::ToSearch
        ) || matches!(
            circulation,
            crate::models::circulation::CirculationStatus::Damaged
                | crate::models::circulation::CirculationStatus::ClaimedReturned
                | crate::models::circulation::CirculationStatus::ToSearch
                | crate::models::circulation::CirculationStatus::Lost
                | crate::models::circulation::CirculationStatus::Missing
        ) {
            return Self::Unavailable;
        }
        if !borrowable {
            return Self::OnSiteOnly;
        }
        if availability == ItemAvailability::Unavailable {
            return Self::Unavailable;
        }
        Self::Available
    }
}

/// Author embedded on a public catalog record. Staff author notes are omitted.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AuthorPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub key: Option<String>,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub function: Option<Function>,
}

impl From<Author> for AuthorPublic {
    fn from(author: Author) -> Self {
        Self {
            id: author.id,
            key: author.key,
            lastname: author.lastname,
            firstname: author.firstname,
            bio: author.bio,
            function: author.function,
        }
    }
}

/// Full physical copy returned to staff.
///
/// Includes weeding, `publicNote`, `internalNote`, circulation status, the active loan id, `dueDate`,
/// and `onHold`. `holdFor` is the patron this copy is held for, projected from
/// the caller's rights. `borrower` is filled only by `GET /biblios/{id}` for
/// callers with loans read or write.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemStaffView {
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
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    pub borrowable: bool,
    pub circulation_status: Option<i16>,
    pub weeding_status: WeedingStatus,
    pub weeding_reason: Option<String>,
    /// Copy note shown to patrons and the OPAC.
    pub public_note: Option<String>,
    /// Staff-only copy note. Former `items.notes` values live here.
    pub internal_note: Option<String>,
    /// Decimal-euro string (`"12.50"`). Stored as integer cents. Null when unknown.
    #[schema(value_type = Option<String>, example = "12.50")]
    pub price: Option<String>,
    /// True when a missing price is an explicit deferral, including a legacy price the cents migration rejected.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub price_deferred: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub source_name: Option<String>,
    pub borrowed: bool,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan_id: Option<i64>,
    /// Active loan `expiry_at`. Null when the copy is not on loan.
    pub due_date: Option<DateTime<Utc>>,
    pub availability: ItemAvailability,
    pub can_hold_item: bool,
    /// This copy is assigned to a hold, or in transit for one. See [`Item::on_hold`].
    pub on_hold: bool,
    /// Patron this copy is held for. Present only when [`Self::availability`] is
    /// `onHold` and the caller has `holds_rights` read or write.
    ///
    /// Holds rights alone: [`LoanBorrowerMinimized`] (`id`, `cardNumber`,
    /// `initials`). Holds rights and `users_rights` read or write:
    /// [`UserShortStaff`]. Omitted without holds rights, including when the
    /// caller has users read or loans read. Patrons and the OPAC omit it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_for: Option<LoanBorrowerResponse>,
    /// Set on `GET /biblios/{id}` when the caller has `loans_rights` read or write
    /// and the copy is on loan. `users_rights` read or write selects [`UserShortStaff`];
    /// otherwise [`LoanBorrowerMinimized`]. Omitted for everyone else, including
    /// staff who can read users but cannot read loans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub borrower: Option<LoanBorrowerResponse>,
}

impl From<Item> for ItemStaffView {
    fn from(item: Item) -> Self {
        Self {
            id: item.id,
            biblio_id: item.biblio_id,
            source_id: item.source_id,
            barcode: item.barcode,
            call_number: item.call_number,
            volume_designation: item.volume_designation,
            place: item.place,
            borrowable: item.borrowable,
            circulation_status: item.circulation_status,
            weeding_status: item.weeding_status,
            weeding_reason: item.weeding_reason,
            public_note: item.public_note,
            internal_note: item.internal_note,
            price: item.price.map(|cents| cents.to_euro_string()),
            price_deferred: item.price_deferred,
            created_at: item.created_at,
            updated_at: item.updated_at,
            archived_at: item.archived_at,
            source_name: item.source_name,
            borrowed: item.borrowed,
            loan_id: item.loan_id,
            due_date: item.due_date,
            availability: item.availability,
            can_hold_item: item.can_hold_item,
            on_hold: item.on_hold,
            hold_for: None,
            borrower: None,
        }
    }
}

/// Full physical copy returned to readers and the OPAC.
///
/// Includes [`Self::public_note`] only. No internal note, hold patron, weeding,
/// circulation status, loan id, borrower, due date, or other staff field.
/// [`Self::on_hold`] is safe for patrons and the OPAC.
/// [`Self::expected_return_date`] is the library-local calendar date. It is present
/// only when [`Self::availability`] is `onLoan` and that date is today or later
/// in the library timezone.
/// [`Self::availability`] uses [`ItemPublicAvailability`]. [`Self::can_hold_item`] matches
/// the staff rule. Staff copies use [`ItemStaffView`], which keeps `dueDate`.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemPublicView {
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
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    pub borrowable: bool,
    /// Copy note safe for patrons and the OPAC. Null when the copy has none.
    pub public_note: Option<String>,
    /// Decimal-euro string (`"12.50"`). Stored as integer cents. Null when unknown.
    #[schema(value_type = Option<String>, example = "12.50")]
    pub price: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub source_name: Option<String>,
    pub borrowed: bool,
    /// Library-local calendar date (`YYYY-MM-DD`) the copy is expected back.
    ///
    /// Present only when [`Self::availability`] is `onLoan`, the open loan has an
    /// `expiry_at`, and that civil date in the library timezone is today or later.
    /// Omitted from the next local day, and for every other availability. No time
    /// and no patron.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_return_date: Option<NaiveDate>,
    pub availability: ItemPublicAvailability,
    pub can_hold_item: bool,
    /// This copy is assigned to a hold, or in transit for one. See [`Item::on_hold`].
    pub on_hold: bool,
}

impl From<Item> for ItemPublicView {
    fn from(item: Item) -> Self {
        let availability = ItemPublicAvailability::for_copy(
            item.availability,
            item.borrowable,
            item.circulation_status,
        );
        let expected_return_date = if availability == ItemPublicAvailability::OnLoan {
            item.expected_return_date
        } else {
            None
        };
        Self {
            id: item.id,
            biblio_id: item.biblio_id,
            source_id: item.source_id,
            barcode: item.barcode,
            call_number: item.call_number,
            volume_designation: item.volume_designation,
            place: item.place,
            borrowable: item.borrowable,
            public_note: item.public_note,
            price: item.price.map(|cents| cents.to_euro_string()),
            created_at: item.created_at,
            updated_at: item.updated_at,
            archived_at: item.archived_at,
            source_name: item.source_name,
            borrowed: item.borrowed,
            expected_return_date,
            availability,
            can_hold_item: item.can_hold_item,
            on_hold: item.on_hold,
        }
    }
}

/// Copy embedded in a staff list (search, series, collection, loans, holds).
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemShortStaff {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub borrowable: bool,
    pub weeding_status: WeedingStatus,
    pub source_name: Option<String>,
    pub borrowed: bool,
    /// `items.circulation_status`. Null or 0 is available; 1 lost, 2 damaged,
    /// 3 claimed returned, 4 to search, 5 declared missing.
    pub circulation_status: Option<i16>,
    pub availability: ItemAvailability,
    pub can_hold_item: bool,
    /// Same rule as [`ItemStaffView::hold_for`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_for: Option<LoanBorrowerResponse>,
}

impl From<ItemShort> for ItemShortStaff {
    fn from(item: ItemShort) -> Self {
        Self {
            id: item.id,
            barcode: item.barcode,
            call_number: item.call_number,
            borrowable: item.borrowable,
            weeding_status: item.weeding_status,
            source_name: item.source_name,
            borrowed: item.borrowed,
            circulation_status: item.circulation_status,
            availability: item.availability,
            can_hold_item: item.can_hold_item,
            hold_for: None,
        }
    }
}

/// Copy embedded in a reader or OPAC list.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemShortPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub borrowable: bool,
    pub source_name: Option<String>,
    pub borrowed: bool,
    pub availability: ItemPublicAvailability,
    pub can_hold_item: bool,
}

impl From<ItemShort> for ItemShortPublic {
    fn from(item: ItemShort) -> Self {
        Self {
            id: item.id,
            barcode: item.barcode,
            call_number: item.call_number,
            borrowable: item.borrowable,
            source_name: item.source_name,
            borrowed: item.borrowed,
            availability: ItemPublicAvailability::for_copy(
                item.availability,
                item.borrowable,
                item.circulation_status,
            ),
            can_hold_item: item.can_hold_item,
        }
    }
}

/// Bibliographic record returned to staff, including full copies.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BiblioStaffView {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub id: Option<i64>,
    pub media_type: MediaType,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub isbn: Option<Isbn>,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub dewey: Option<String>,
    pub audience_type: Option<AudienceType>,
    pub lang: Option<Language>,
    pub lang_orig: Option<Language>,
    pub publication_date: Option<String>,
    pub page_extent: Option<String>,
    pub format: Option<String>,
    pub table_of_contents: Option<String>,
    pub accompanying_material: Option<String>,
    pub abstract_: Option<String>,
    pub notes: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub is_valid: Option<bool>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub series_ids: Vec<i64>,
    pub series_volume_numbers: Vec<Option<i16>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub edition_id: Option<i64>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub collection_ids: Vec<i64>,
    pub collection_volume_numbers: Vec<Option<i16>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub weeding_status: WeedingStatus,
    /// `pending` holds still waiting for a copy. `to_pick`, `ready`, and holds
    /// with an active item transit are excluded; those copies set `onHold`.
    /// No patron identity. Present for every caller.
    pub holds_count: i64,
    pub authors: Vec<Author>,
    pub series: Vec<Serie>,
    pub collections: Vec<Collection>,
    pub edition: Option<Edition>,
    pub items: Vec<ItemStaffView>,
}

impl From<Biblio> for BiblioStaffView {
    fn from(biblio: Biblio) -> Self {
        Self {
            id: biblio.id,
            media_type: biblio.media_type,
            isbn: biblio.isbn,
            title: biblio.title,
            subject: biblio.subject,
            dewey: biblio.dewey,
            audience_type: biblio.audience_type,
            lang: biblio.lang,
            lang_orig: biblio.lang_orig,
            publication_date: biblio.publication_date,
            page_extent: biblio.page_extent,
            format: biblio.format,
            table_of_contents: biblio.table_of_contents,
            accompanying_material: biblio.accompanying_material,
            abstract_: biblio.abstract_,
            notes: biblio.notes,
            keywords: biblio.keywords,
            is_valid: biblio.is_valid,
            series_ids: biblio.series_ids,
            series_volume_numbers: biblio.series_volume_numbers,
            edition_id: biblio.edition_id,
            collection_ids: biblio.collection_ids,
            collection_volume_numbers: biblio.collection_volume_numbers,
            created_at: biblio.created_at,
            updated_at: biblio.updated_at,
            archived_at: biblio.archived_at,
            weeding_status: biblio.weeding_status,
            holds_count: biblio.holds_count,
            authors: biblio.authors,
            series: biblio.series,
            collections: biblio.collections,
            edition: biblio.edition,
            items: biblio.items.into_iter().map(ItemStaffView::from).collect(),
        }
    }
}

/// Bibliographic record returned to readers and the OPAC.
///
/// No bibliographic notes, weeding mark, or staff copy fields.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BiblioPublicView {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub id: Option<i64>,
    pub media_type: MediaType,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub isbn: Option<Isbn>,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub dewey: Option<String>,
    pub audience_type: Option<AudienceType>,
    pub lang: Option<Language>,
    pub lang_orig: Option<Language>,
    pub publication_date: Option<String>,
    pub page_extent: Option<String>,
    pub format: Option<String>,
    pub table_of_contents: Option<String>,
    pub accompanying_material: Option<String>,
    pub abstract_: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub is_valid: Option<bool>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub series_ids: Vec<i64>,
    pub series_volume_numbers: Vec<Option<i16>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub edition_id: Option<i64>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub collection_ids: Vec<i64>,
    pub collection_volume_numbers: Vec<Option<i16>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    /// `pending` holds still waiting for a copy. `to_pick`, `ready`, and holds
    /// with an active item transit are excluded; those copies set `onHold`.
    /// No patron identity.
    pub holds_count: i64,
    pub authors: Vec<AuthorPublic>,
    pub series: Vec<Serie>,
    pub collections: Vec<Collection>,
    pub edition: Option<Edition>,
    pub items: Vec<ItemPublicView>,
}

impl From<Biblio> for BiblioPublicView {
    fn from(biblio: Biblio) -> Self {
        Self {
            id: biblio.id,
            media_type: biblio.media_type,
            isbn: biblio.isbn,
            title: biblio.title,
            subject: biblio.subject,
            dewey: biblio.dewey,
            audience_type: biblio.audience_type,
            lang: biblio.lang,
            lang_orig: biblio.lang_orig,
            publication_date: biblio.publication_date,
            page_extent: biblio.page_extent,
            format: biblio.format,
            table_of_contents: biblio.table_of_contents,
            accompanying_material: biblio.accompanying_material,
            abstract_: biblio.abstract_,
            keywords: biblio.keywords,
            is_valid: biblio.is_valid,
            series_ids: biblio.series_ids,
            series_volume_numbers: biblio.series_volume_numbers,
            edition_id: biblio.edition_id,
            collection_ids: biblio.collection_ids,
            collection_volume_numbers: biblio.collection_volume_numbers,
            created_at: biblio.created_at,
            updated_at: biblio.updated_at,
            archived_at: biblio.archived_at,
            holds_count: biblio.holds_count,
            authors: biblio.authors.into_iter().map(AuthorPublic::from).collect(),
            series: biblio.series,
            collections: biblio.collections,
            edition: biblio.edition,
            items: biblio
                .items
                .into_iter()
                .filter(Item::included_in_public_catalog)
                .map(ItemPublicView::from)
                .collect(),
        }
    }
}

/// List record returned to staff.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BiblioShortStaff {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub media_type: MediaType,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub isbn: Option<Isbn>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub status: i16,
    pub is_valid: Option<bool>,
    pub archived_at: Option<DateTime<Utc>>,
    pub weeding_status: WeedingStatus,
    pub author: Option<Author>,
    pub items: Vec<ItemShortStaff>,
}

impl From<BiblioShort> for BiblioShortStaff {
    fn from(biblio: BiblioShort) -> Self {
        Self {
            id: biblio.id,
            media_type: biblio.media_type,
            isbn: biblio.isbn,
            title: biblio.title,
            date: biblio.date,
            status: biblio.status,
            is_valid: biblio.is_valid,
            archived_at: biblio.archived_at,
            weeding_status: biblio.weeding_status,
            author: biblio.author,
            items: biblio.items.into_iter().map(ItemShortStaff::from).collect(),
        }
    }
}

/// List record returned to readers and the OPAC.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BiblioShortPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub media_type: MediaType,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub isbn: Option<Isbn>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub status: i16,
    pub is_valid: Option<bool>,
    pub archived_at: Option<DateTime<Utc>>,
    pub author: Option<AuthorPublic>,
    pub items: Vec<ItemShortPublic>,
}

impl From<BiblioShort> for BiblioShortPublic {
    fn from(biblio: BiblioShort) -> Self {
        Self {
            id: biblio.id,
            media_type: biblio.media_type,
            isbn: biblio.isbn,
            title: biblio.title,
            date: biblio.date,
            status: biblio.status,
            is_valid: biblio.is_valid,
            archived_at: biblio.archived_at,
            author: biblio.author.map(AuthorPublic::from),
            items: biblio
                .items
                .into_iter()
                .filter(ItemShort::included_in_public_catalog)
                .map(ItemShortPublic::from)
                .collect(),
        }
    }
}

/// Patron summary returned to staff. `login` and `barcode` exist only on this type.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserShortStaff {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub login: Option<String>,
    /// Library card number. The staff patron picker shows this when `login` is empty.
    pub barcode: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub account_type: Option<AccountTypeSlug>,
    pub public_type: Option<i64>,
    pub nb_loans: Option<i64>,
    pub nb_late_loans: Option<i64>,
    pub status: Option<UserStatus>,
    pub created_at: Option<DateTime<Utc>>,
    pub expiry_at: Option<DateTime<Utc>>,
}

impl From<UserShort> for UserShortStaff {
    fn from(user: UserShort) -> Self {
        Self {
            id: user.id,
            login: user.login,
            barcode: user.barcode,
            firstname: user.firstname,
            lastname: user.lastname,
            account_type: user.account_type,
            public_type: user.public_type,
            nb_loans: user.nb_loans,
            nb_late_loans: user.nb_late_loans,
            status: user.status,
            created_at: user.created_at,
            expiry_at: user.expiry_at,
        }
    }
}

/// Patron summary returned to readers. No `login`.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserShortPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub account_type: Option<AccountTypeSlug>,
    pub public_type: Option<i64>,
    pub nb_loans: Option<i64>,
    pub nb_late_loans: Option<i64>,
    pub status: Option<UserStatus>,
    pub created_at: Option<DateTime<Utc>>,
    pub expiry_at: Option<DateTime<Utc>>,
}

impl From<UserShort> for UserShortPublic {
    fn from(user: UserShort) -> Self {
        Self {
            id: user.id,
            firstname: user.firstname,
            lastname: user.lastname,
            account_type: user.account_type,
            public_type: user.public_type,
            nb_loans: user.nb_loans,
            nb_late_loans: user.nb_late_loans,
            status: user.status,
            created_at: user.created_at,
            expiry_at: user.expiry_at,
        }
    }
}

/// Borrower of an active loan.
///
/// Callers with `users_rights` read or write receive [`UserShortStaff`].
/// Everyone else who is allowed to see a borrower receives [`LoanBorrowerMinimized`].
/// `loans_rights` does not unlock the staff profile.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(untagged)]
pub enum LoanBorrowerResponse {
    Full(UserShortStaff),
    Minimized(LoanBorrowerMinimized),
}

/// Identity of the patron a copy is held for. Not serialized by itself.
#[derive(Debug, Clone)]
pub struct HoldPatronSource {
    pub id: i64,
    pub barcode: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
}

impl Item {
    /// Assigned hold patron when this copy is `onHold`.
    #[must_use]
    pub fn hold_patron_source(&self) -> Option<HoldPatronSource> {
        hold_patron_source(
            self.hold_user_id,
            self.hold_user_barcode.clone(),
            self.hold_user_firstname.clone(),
            self.hold_user_lastname.clone(),
        )
    }
}

impl ItemShort {
    /// Assigned hold patron when this copy is `onHold`.
    #[must_use]
    pub fn hold_patron_source(&self) -> Option<HoldPatronSource> {
        hold_patron_source(
            self.hold_user_id,
            self.hold_user_barcode.clone(),
            self.hold_user_firstname.clone(),
            self.hold_user_lastname.clone(),
        )
    }
}

fn hold_patron_source(
    id: Option<i64>,
    barcode: Option<String>,
    firstname: Option<String>,
    lastname: Option<String>,
) -> Option<HoldPatronSource> {
    Some(HoldPatronSource {
        id: id?,
        barcode: barcode.filter(|value| !value.trim().is_empty()),
        firstname,
        lastname,
    })
}

/// Which hold patron a caller may see.
///
/// One decision for `holdFor` and for later projections that show the same
/// patron (an item transit, a patron's loans). Pass the caller's rights and
/// the patron row. The result is:
///
/// 1. `None` when `holds_rights` is below read. Users read and loans read do
///    not reveal the patron on their own.
/// 2. [`LoanBorrowerMinimized`] (`id`, `cardNumber`, `initials`) when the
///    caller has holds read or write and does not have users read.
/// 3. [`UserShortStaff`] when the caller has holds read or write and users
///    read or write.
///
/// The patron row is not loaded here. Callers that have users read should
/// pass a full [`UserShort`] (login and barcode). Shelf labels and hold slips
/// do not use this helper; they stay minimized.
#[must_use]
pub fn hold_for_identity(rights: &UserRights, patron: UserShort) -> Option<LoanBorrowerResponse> {
    if rights.holds_rights.rank() < Rights::Read.rank() {
        return None;
    }
    if rights.users_rights.rank() >= Rights::Read.rank() {
        Some(LoanBorrowerResponse::Full(UserShortStaff::from(patron)))
    } else {
        Some(LoanBorrowerResponse::Minimized(
            LoanBorrowerMinimized::from(patron),
        ))
    }
}

/// `holdFor` on a staff catalog copy.
///
/// Resolves the patron from `full` (or the copy columns) and applies
/// [`hold_for_identity`].
#[must_use]
pub fn project_hold_for(
    source: HoldPatronSource,
    claims: &UserClaims,
    full: &HashMap<i64, UserShort>,
) -> Option<LoanBorrowerResponse> {
    let patron = full.get(&source.id).cloned().unwrap_or_else(|| {
        partial_user_short(source.id, source.barcode, source.firstname, source.lastname)
    });
    hold_for_identity(&claims.rights, patron)
}

/// Patron row built from the columns already on a catalog or pick-list query.
#[must_use]
pub fn partial_user_short(
    id: i64,
    barcode: Option<String>,
    firstname: Option<String>,
    lastname: Option<String>,
) -> UserShort {
    UserShort {
        id,
        login: None,
        barcode,
        firstname,
        lastname,
        account_type: None,
        public_type: None,
        nb_loans: None,
        nb_late_loans: None,
        status: None,
        created_at: None,
        expiry_at: None,
    }
}

/// Staff hold list and hold slip.
///
/// [`UserShortStaff`] only with `users_rights` read or write. Every other caller
/// who is allowed to see the row receives [`LoanBorrowerMinimized`]: card number
/// or initials, never the name. `GET /holds/pick-list` does not use this helper:
/// shelf labels are always minimized.
#[must_use]
pub fn project_listed_hold_patron(user: UserShort, claims: &UserClaims) -> LoanBorrowerResponse {
    project_borrower(user, claims, false)
}

/// Staff profile when the caller can read users, otherwise id, card, and initials.
///
/// `own_identity` is for `GET /loans/{id}/user`: the borrower of that loan receives
/// [`UserShortStaff`] even when `users_rights` is only own. Catalog notices pass
/// `false`, so a patron never receives another person's identity from the notice.
#[must_use]
pub fn project_borrower(
    user: UserShort,
    claims: &UserClaims,
    own_identity: bool,
) -> LoanBorrowerResponse {
    let users_read = claims.rights.users_rights.rank() >= Rights::Read.rank();
    if users_read || (own_identity && user.id == claims.user_id) {
        LoanBorrowerResponse::Full(UserShortStaff::from(user))
    } else {
        LoanBorrowerResponse::Minimized(LoanBorrowerMinimized::from(user))
    }
}

/// Id, library card, and initials. No name, login, email, phone, or address.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanBorrowerMinimized {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// `users.barcode`. Null when the patron has no card on file.
    pub card_number: Option<String>,
    /// Uppercase first letter of `firstname` plus first letter of `lastname`.
    /// A missing or blank part is skipped.
    pub initials: String,
}

impl From<UserShort> for LoanBorrowerMinimized {
    fn from(user: UserShort) -> Self {
        Self {
            id: user.id,
            card_number: user.barcode.filter(|value| !value.trim().is_empty()),
            initials: borrower_initials(user.firstname.as_deref(), user.lastname.as_deref()),
        }
    }
}

/// Uppercase first letter of each name part. Blank and missing parts are skipped.
#[must_use]
pub fn borrower_initials(firstname: Option<&str>, lastname: Option<&str>) -> String {
    let mut initials = String::new();
    for part in [firstname, lastname] {
        let Some(ch) = part.and_then(|value| value.trim().chars().next()) else {
            continue;
        };
        initials.extend(ch.to_uppercase());
    }
    initials
}

/// Staff hold queue row (`GET /items/{id}/holds`, `GET /biblios/{id}/holds`,
/// `GET /users/{id}/holds`). Both notes are included for callers with holds rights.
/// `GET /holds` uses [`HoldListStaff`] and omits them.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldDetailsStaff {
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
    pub biblio: BiblioShortStaff,
    /// `users_rights` read or write: [`UserShortStaff`]. Otherwise `{ id, cardNumber, initials }`.
    pub user: Option<LoanBorrowerResponse>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    /// Pickup deadline. Set when the hold is `ready`.
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    /// Note visible to the patron and to staff.
    pub patron_note: Option<String>,
    /// Staff-only remark. Absent from [`HoldListStaff`], [`HoldDetailsPublic`], and `GET /me/holds`.
    pub internal_note: Option<String>,
}

/// `GET /holds` row for callers with internal catalog rights.
///
/// Same shape as [`HoldDetailsStaff`] except both notes are omitted. Staff queues
/// keep the notes. `GET /me/holds` keeps `patronNote` on [`crate::models::my_account::MyHoldView`].
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldListStaff {
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
    pub biblio: BiblioShortStaff,
    /// `users_rights` read or write: [`UserShortStaff`]. Otherwise `{ id, cardNumber, initials }`.
    pub user: Option<LoanBorrowerResponse>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    /// Pickup deadline. Set when the hold is `ready`.
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
}

impl HoldListStaff {
    /// Search/list row. Drops both notes from the staff queue shape.
    #[must_use]
    pub fn for_caller(hold: HoldDetails, claims: &UserClaims) -> Self {
        let staff = HoldDetailsStaff::for_caller(hold, claims);
        Self {
            id: staff.id,
            user_id: staff.user_id,
            biblio_id: staff.biblio_id,
            item_id: staff.item_id,
            pickup_site_id: staff.pickup_site_id,
            biblio: staff.biblio,
            user: staff.user,
            created_at: staff.created_at,
            notified_at: staff.notified_at,
            expires_at: staff.expires_at,
            status: staff.status,
            position: staff.position,
        }
    }
}

impl HoldDetailsStaff {
    /// Staff hold row. `expiresAt` is the pickup deadline.
    #[must_use]
    pub fn for_caller(hold: HoldDetails, claims: &UserClaims) -> Self {
        Self {
            id: hold.id,
            user_id: hold.user_id,
            biblio_id: hold.biblio_id,
            item_id: hold.item_id,
            pickup_site_id: hold.pickup_site_id,
            biblio: BiblioShortStaff::from(hold.biblio),
            user: hold
                .user
                .map(|user| project_listed_hold_patron(user, claims)),
            created_at: hold.created_at,
            notified_at: hold.notified_at,
            expires_at: hold.expires_at,
            status: hold.status,
            position: hold.position,
            patron_note: hold.patron_note,
            internal_note: hold.internal_note,
        }
    }
}

/// Public hold list row (`GET /holds` without internal catalog rights).
/// Neither note is included. Embedded copies are [`BiblioShortPublic`].
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldDetailsPublic {
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
    pub biblio: BiblioShortPublic,
    pub user: Option<UserShortPublic>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
}

impl From<HoldDetails> for HoldDetailsPublic {
    fn from(hold: HoldDetails) -> Self {
        Self {
            id: hold.id,
            user_id: hold.user_id,
            biblio_id: hold.biblio_id,
            item_id: hold.item_id,
            pickup_site_id: hold.pickup_site_id,
            biblio: BiblioShortPublic::from(hold.biblio),
            user: hold.user.map(UserShortPublic::from),
            created_at: hold.created_at,
            notified_at: hold.notified_at,
            expires_at: hold.expires_at,
            status: hold.status,
            position: hold.position,
        }
    }
}

/// Loan list row for staff.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanDetailsStaff {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    /// Desk site at checkout. Null when the loan has no recorded site.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
    /// Set on closed loans (`returned` or `lost`). Omitted on open loans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reason: Option<crate::models::loan::LoanClosureReason>,
    /// When a loan closed as lost had its copy found. Omitted on open loans.
    /// JSON null until the copy is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub found_at: Option<Option<DateTime<Utc>>>,
    pub biblio: BiblioShortStaff,
    pub user: Option<UserShortStaff>,
    pub item_identification: Option<String>,
    pub is_overdue: bool,
}

impl From<LoanDetails> for LoanDetailsStaff {
    fn from(loan: LoanDetails) -> Self {
        Self {
            id: loan.id,
            item_id: loan.item_id,
            site_id: loan.site_id,
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
            closure_reason: loan.closure_reason,
            found_at: loan.found_at,
            biblio: BiblioShortStaff::from(loan.biblio),
            user: loan.user.map(UserShortStaff::from),
            item_identification: loan.item_identification,
            is_overdue: loan.is_overdue,
        }
    }
}

/// Loan list row for a reader.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanDetailsPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    /// Desk site at checkout. Null when the loan has no recorded site.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
    /// Set on closed loans (`returned` or `lost`). Omitted on open loans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reason: Option<crate::models::loan::LoanClosureReason>,
    /// When a loan closed as lost had its copy found. Omitted on open loans.
    /// JSON null until the copy is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub found_at: Option<Option<DateTime<Utc>>>,
    pub biblio: BiblioShortPublic,
    pub user: Option<UserShortPublic>,
    pub item_identification: Option<String>,
    pub is_overdue: bool,
}

impl From<LoanDetails> for LoanDetailsPublic {
    fn from(loan: LoanDetails) -> Self {
        Self {
            id: loan.id,
            item_id: loan.item_id,
            site_id: loan.site_id,
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
            closure_reason: loan.closure_reason,
            found_at: loan.found_at,
            biblio: BiblioShortPublic::from(loan.biblio),
            user: loan.user.map(UserShortPublic::from),
            item_identification: loan.item_identification,
            is_overdue: loan.is_overdue,
        }
    }
}

/// Loan list row when the caller cannot read users.
///
/// Copy fields match [`LoanDetailsPublic`]. `user` is [`LoanBorrowerMinimized`].
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanDetailsOwn {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    /// Desk site at checkout. Null when the loan has no recorded site.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
    /// Set on closed loans (`returned` or `lost`). Omitted on open loans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reason: Option<crate::models::loan::LoanClosureReason>,
    /// When a loan closed as lost had its copy found. Omitted on open loans.
    /// JSON null until the copy is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub found_at: Option<Option<DateTime<Utc>>>,
    pub biblio: BiblioShortPublic,
    pub user: Option<LoanBorrowerMinimized>,
    pub item_identification: Option<String>,
    pub is_overdue: bool,
}

impl From<LoanDetails> for LoanDetailsOwn {
    fn from(loan: LoanDetails) -> Self {
        Self {
            id: loan.id,
            item_id: loan.item_id,
            site_id: loan.site_id,
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
            closure_reason: loan.closure_reason,
            found_at: loan.found_at,
            biblio: BiblioShortPublic::from(loan.biblio),
            user: loan.user.map(LoanBorrowerMinimized::from),
            item_identification: loan.item_identification,
            is_overdue: loan.is_overdue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::UserRights;
    use chrono::TimeZone;

    #[test]
    fn public_availability_collapses_temporary_states_and_hides_staff_fields() {
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::ToSearch, false, Some(4)),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::InTransit, true, None),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::OnHold, true, None),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::Unavailable, false, Some(2)),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::Unavailable, false, None),
            ItemPublicAvailability::OnSiteOnly
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::OnLoan, true, Some(4)),
            ItemPublicAvailability::OnLoan
        );
        assert_eq!(
            ItemPublicAvailability::for_copy(ItemAvailability::Available, true, None),
            ItemPublicAvailability::Available
        );

        let item = ItemShort {
            id: 7,
            barcode: Some("B".into()),
            call_number: None,
            borrowable: false,
            circulation_status: Some(4),
            weeding_status: WeedingStatus::OnShelf,
            source_name: None,
            borrowed: false,
            availability: ItemAvailability::ToSearch,
            can_hold_item: false,
            hold_user_id: Some(3),
            hold_user_barcode: Some("CARD".into()),
            hold_user_firstname: Some("Ada".into()),
            hold_user_lastname: Some("Lovelace".into()),
        };
        let body = serde_json::to_value(ItemShortPublic::from(item)).unwrap();
        assert_eq!(body["availability"], "unavailable");
        assert_eq!(body["canHoldItem"], false);
        assert!(body.get("weedingStatus").is_none());
        assert!(body.get("circulationStatus").is_none());
        assert!(body.get("holdUserId").is_none());
        assert!(body.get("holdUserName").is_none());
        assert!(body.get("holdFor").is_none());
        assert!(body.get("firstname").is_none());
        assert!(body.get("lastname").is_none());
        assert!(body.get("borrower").is_none());
        assert!(body.get("notes").is_none());
        assert!(body.get("login").is_none());
    }

    fn claims(account_type: AccountTypeSlug, rights: UserRights) -> UserClaims {
        UserClaims {
            sub: "1".into(),
            user_id: 1,
            account_type,
            rights,
            exp: 0,
            iat: 0,
            token_version: 1,
            scope: None,
        }
    }

    #[test]
    fn internal_catalog_view_follows_rights_not_account_type() {
        let patron = UserRights {
            items_rights: Rights::Read,
            users_rights: Rights::Own,
            loans_rights: Rights::Own,
            holds_rights: Rights::Own,
            items_archive_rights: Rights::Read,
            settings_rights: Rights::Read,
            events_rights: Rights::Read,
            ..UserRights::default()
        };
        assert!(!sees_internal_catalog(&claims(
            AccountTypeSlug::Librarian,
            patron.clone()
        )));
        assert!(!sees_internal_catalog(&claims(
            AccountTypeSlug::Reader,
            patron
        )));

        let cataloger = UserRights {
            items_rights: Rights::Write,
            users_rights: Rights::Own,
            loans_rights: Rights::None,
            holds_rights: Rights::Own,
            ..UserRights::default()
        };
        assert!(sees_internal_catalog(&claims(
            AccountTypeSlug::Reader,
            cataloger
        )));

        let loans_desk = UserRights {
            items_rights: Rights::Read,
            loans_rights: Rights::Read,
            users_rights: Rights::Own,
            holds_rights: Rights::Own,
            ..UserRights::default()
        };
        assert!(sees_internal_catalog(&claims(
            AccountTypeSlug::Group,
            loans_desk
        )));

        let users_desk = UserRights {
            items_rights: Rights::Read,
            users_rights: Rights::Read,
            ..UserRights::default()
        };
        assert!(sees_internal_catalog(&claims(
            AccountTypeSlug::Reader,
            users_desk
        )));

        let holds_desk = UserRights {
            items_rights: Rights::Read,
            holds_rights: Rights::Read,
            ..UserRights::default()
        };
        assert!(sees_internal_catalog(&claims(
            AccountTypeSlug::Reader,
            holds_desk
        )));

        let weeder = UserRights {
            items_rights: Rights::Read,
            weeding_rights: Rights::Read,
            ..UserRights::default()
        };
        assert!(sees_internal_catalog(&claims(
            AccountTypeSlug::Reader,
            weeder
        )));
    }

    fn copy_with(availability: ItemAvailability, expected_return_date: Option<NaiveDate>) -> Item {
        Item {
            id: Some(11),
            biblio_id: Some(22),
            source_id: None,
            barcode: Some("BC".into()),
            call_number: None,
            volume_designation: None,
            place: None,
            borrowable: true,
            circulation_status: None,
            weeding_status: WeedingStatus::OnShelf,
            weeding_reason: None,
            public_note: None,
            internal_note: Some("staff only note".into()),
            price: None,
            price_deferred: false,
            created_at: None,
            updated_at: None,
            archived_at: None,
            source_name: None,
            borrowed: availability == ItemAvailability::OnLoan,
            loan_id: Some(99),
            availability,
            can_hold_item: true,
            hold_user_id: Some(5),
            hold_user_barcode: Some("SECRET-CARD".into()),
            hold_user_firstname: Some("Isolde".into()),
            hold_user_lastname: Some("Moreau".into()),
            due_date: Some(
                Utc.with_ymd_and_hms(2026, 1, 16, 21, 30, 0)
                    .single()
                    .expect("due instant"),
            ),
            expected_return_date,
            on_hold: false,
            legacy_notes_present: false,
        }
    }

    #[test]
    fn public_expected_return_date_is_date_only_and_only_on_loan() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 16).expect("date");
        let on_loan = serde_json::to_value(ItemPublicView::from(copy_with(
            ItemAvailability::OnLoan,
            Some(date),
        )))
        .expect("json");
        assert_eq!(on_loan["availability"], "onLoan");
        assert_eq!(on_loan["expectedReturnDate"], "2026-01-16");
        let rendered = on_loan["expectedReturnDate"].as_str().expect("date string");
        assert_eq!(rendered.len(), 10);
        assert!(!rendered.contains('T'));
        let text = on_loan.to_string();
        for needle in ["Isolde", "Moreau", "SECRET-CARD", "staff only note"] {
            assert!(!text.contains(needle), "{text}");
        }
        assert!(on_loan.get("borrower").is_none(), "{on_loan}");
        assert!(on_loan.get("holdFor").is_none(), "{on_loan}");
        assert!(on_loan.get("loanId").is_none(), "{on_loan}");
        assert!(
            on_loan.get("dueDate").is_none(),
            "ItemPublicView must not contain dueDate: {on_loan}"
        );

        let staff = serde_json::to_value(ItemStaffView::from(copy_with(
            ItemAvailability::OnLoan,
            Some(date),
        )))
        .expect("staff json");
        assert!(
            staff["dueDate"]
                .as_str()
                .is_some_and(|value| value.contains('T')),
            "ItemStaffView keeps dueDate: {staff}"
        );

        let shelved = serde_json::to_value(ItemPublicView::from(copy_with(
            ItemAvailability::Available,
            Some(date),
        )))
        .expect("json");
        assert_eq!(shelved["availability"], "available");
        assert!(shelved.get("expectedReturnDate").is_none(), "{shelved}");

        let undated = serde_json::to_value(ItemPublicView::from(copy_with(
            ItemAvailability::OnLoan,
            None,
        )))
        .expect("json");
        assert_eq!(undated["availability"], "onLoan");
        assert!(undated.get("expectedReturnDate").is_none(), "{undated}");
    }

    fn held_patron() -> UserShort {
        UserShort {
            id: 42,
            login: Some("ada".into()),
            barcode: Some("CARD".into()),
            firstname: Some("Ada".into()),
            lastname: Some("Lovelace".into()),
            account_type: None,
            public_type: None,
            nb_loans: None,
            nb_late_loans: None,
            status: None,
            created_at: None,
            expiry_at: None,
        }
    }

    /// (1) no holds rights -> absent, even with users read and loans read.
    /// (2) holds rights -> `{ id, cardNumber, initials }`.
    /// (3) holds rights and users read -> staff identity.
    #[test]
    fn hold_for_identity_follows_holds_rights_then_users_read() {
        let patron = held_patron();

        let users_read_without_holds = UserRights {
            users_rights: Rights::Read,
            loans_rights: Rights::Read,
            holds_rights: Rights::None,
            ..UserRights::default()
        };
        assert!(
            hold_for_identity(&users_read_without_holds, patron.clone()).is_none(),
            "users read without holds rights must omit holdFor"
        );

        let holds_only = UserRights {
            users_rights: Rights::None,
            loans_rights: Rights::None,
            holds_rights: Rights::Read,
            ..UserRights::default()
        };
        let minimal =
            serde_json::to_value(hold_for_identity(&holds_only, patron.clone()).expect("holds"))
                .expect("json");
        assert_eq!(minimal["id"], "42", "{minimal}");
        assert_eq!(minimal["cardNumber"], "CARD", "{minimal}");
        assert_eq!(minimal["initials"], "AL", "{minimal}");
        assert!(minimal.get("firstname").is_none(), "{minimal}");
        assert!(minimal.get("login").is_none(), "{minimal}");
        assert!(minimal.get("barcode").is_none(), "{minimal}");

        let holds_and_users = UserRights {
            users_rights: Rights::Read,
            loans_rights: Rights::None,
            holds_rights: Rights::Read,
            ..UserRights::default()
        };
        let staff = serde_json::to_value(
            hold_for_identity(&holds_and_users, patron).expect("holds and users read"),
        )
        .expect("json");
        assert_eq!(staff["firstname"], "Ada", "{staff}");
        assert_eq!(staff["lastname"], "Lovelace", "{staff}");
        assert_eq!(staff["login"], "ada", "{staff}");
        assert_eq!(staff["barcode"], "CARD", "{staff}");
        assert!(staff.get("cardNumber").is_none(), "{staff}");
        assert!(staff.get("email").is_none(), "{staff}");
    }

    #[test]
    fn public_copy_keeps_public_note_and_drops_internal_note() {
        let item = Item {
            id: Some(9),
            biblio_id: Some(3),
            source_id: None,
            barcode: Some("COPY".into()),
            call_number: None,
            volume_designation: None,
            place: None,
            borrowable: true,
            circulation_status: None,
            weeding_status: WeedingStatus::OnShelf,
            weeding_reason: None,
            public_note: Some("CD missing".into()),
            internal_note: Some("spine cracked".into()),
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
        };
        let public_body = serde_json::to_value(ItemPublicView::from(item.clone())).unwrap();
        assert_eq!(public_body["publicNote"], "CD missing");
        assert!(public_body.get("internalNote").is_none());
        assert!(public_body.get("notes").is_none());
        let staff_body = serde_json::to_value(ItemStaffView::from(item)).unwrap();
        assert_eq!(staff_body["publicNote"], "CD missing");
        assert_eq!(staff_body["internalNote"], "spine cracked");
        assert!(staff_body.get("notes").is_none());
    }
}
