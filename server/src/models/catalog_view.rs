//! Staff and public JSON shapes for catalog copies and the user summaries embedded with them.
//!
//! Handlers pick one type with [`crate::models::user::UserClaims::is_librarian`] (the same
//! check used for weeding visibility). The public structs do not contain staff fields, so a
//! reader response cannot include them as blank options.
//!
//! A patron's own loans and holds are [`crate::models::my_account::MyLoanView`] and
//! [`crate::models::my_account::MyHoldView`] from `GET /me/loans` and `GET /me/holds`.
//! Those payloads are not these catalog embeds.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
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
use super::user::{AccountTypeSlug, Rights, UserClaims, UserShort, UserStatus};
use super::Language;

/// Availability returned to readers and the OPAC.
///
/// Staff [`ItemAvailability::ToSearch`] and [`ItemAvailability::Unavailable`] both become
/// [`Self::Unavailable`]. The public value does not say why the copy is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ItemPublicAvailability {
    Available,
    OnHold,
    InTransit,
    OnLoan,
    Unavailable,
}

impl From<ItemAvailability> for ItemPublicAvailability {
    fn from(value: ItemAvailability) -> Self {
        match value {
            ItemAvailability::Available => Self::Available,
            ItemAvailability::OnHold => Self::OnHold,
            ItemAvailability::InTransit => Self::InTransit,
            ItemAvailability::OnLoan => Self::OnLoan,
            ItemAvailability::ToSearch | ItemAvailability::Unavailable => Self::Unavailable,
        }
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
/// Includes weeding, copy notes, circulation status, the active loan id, `dueDate`,
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
    pub notes: Option<String>,
    pub price: Option<String>,
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
    /// `onHold` and the caller may see a hold patron.
    ///
    /// `users_rights` read or write: [`UserShortStaff`]. `loans_rights` read or
    /// write without users read: [`LoanBorrowerMinimized`] (`id`, `cardNumber`,
    /// `initials`). Omitted for patrons, the OPAC, and staff with neither right.
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
            notes: item.notes,
            price: item.price,
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
/// No hold patron, notes, weeding, circulation status, loan id, borrower, or other staff field.
/// [`Self::due_date`] and [`Self::on_hold`] are safe for patrons and the OPAC.
/// [`Self::availability`] uses [`ItemPublicAvailability`]. [`Self::can_hold_item`] matches
/// the staff rule.
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
    pub price: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub source_name: Option<String>,
    pub borrowed: bool,
    /// Active loan `expiry_at`. Null when the copy is not on loan.
    pub due_date: Option<DateTime<Utc>>,
    pub availability: ItemPublicAvailability,
    pub can_hold_item: bool,
    /// This copy is assigned to a hold, or in transit for one. See [`Item::on_hold`].
    pub on_hold: bool,
}

impl From<Item> for ItemPublicView {
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
            price: item.price,
            created_at: item.created_at,
            updated_at: item.updated_at,
            archived_at: item.archived_at,
            source_name: item.source_name,
            borrowed: item.borrowed,
            due_date: item.due_date,
            availability: ItemPublicAvailability::from(item.availability),
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
            availability: ItemPublicAvailability::from(item.availability),
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
                .filter(|item| item.weeding_status != WeedingStatus::Withdrawn)
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
                .filter(|item| item.weeding_status != WeedingStatus::Withdrawn)
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

/// `holdFor` on a staff catalog copy.
///
/// Uses [`project_borrower`] with `own_identity` false. `users_rights` read or
/// write selects [`UserShortStaff`]. `loans_rights` read or write without users
/// read selects [`LoanBorrowerMinimized`]. Anyone else receives `None`, including
/// patrons and the OPAC.
#[must_use]
pub fn project_hold_for(
    source: HoldPatronSource,
    claims: &UserClaims,
    full: &HashMap<i64, UserShort>,
) -> Option<LoanBorrowerResponse> {
    let users_read = claims.rights.users_rights.rank() >= Rights::Read.rank();
    let loans_read = claims.rights.loans_rights.rank() >= Rights::Read.rank();
    if !users_read && !loans_read {
        return None;
    }
    let user = full.get(&source.id).cloned().unwrap_or_else(|| {
        partial_user_short(source.id, source.barcode, source.firstname, source.lastname)
    });
    Some(project_borrower(user, claims, false))
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

/// Hold list row for staff. Embedded copies are [`BiblioShortStaff`].
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
    pub notes: Option<String>,
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
            notes: hold.notes,
        }
    }
}

/// Hold list row for a reader. Embedded copies are [`BiblioShortPublic`].
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
    pub notes: Option<String>,
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
            notes: hold.notes,
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
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
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
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
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
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
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
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
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
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
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
            start_date: loan.start_date,
            expiry_at: loan.expiry_at,
            renewal_date: loan.renewal_date,
            nb_renews: loan.nb_renews,
            returned_at: loan.returned_at,
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

    #[test]
    fn public_availability_hides_to_search_and_reason() {
        assert_eq!(
            ItemPublicAvailability::from(ItemAvailability::ToSearch),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::from(ItemAvailability::Unavailable),
            ItemPublicAvailability::Unavailable
        );
        assert_eq!(
            ItemPublicAvailability::from(ItemAvailability::OnHold),
            ItemPublicAvailability::OnHold
        );

        let item = ItemShort {
            id: 7,
            barcode: Some("B".into()),
            call_number: None,
            borrowable: true,
            weeding_status: WeedingStatus::Candidate,
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
        assert!(body.get("holdUserId").is_none());
        assert!(body.get("holdUserName").is_none());
        assert!(body.get("holdFor").is_none());
        assert!(body.get("firstname").is_none());
        assert!(body.get("lastname").is_none());
        assert!(body.get("notes").is_none());
        assert!(body.get("login").is_none());
    }
}
