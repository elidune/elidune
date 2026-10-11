//! Staff and public JSON shapes for catalog copies and the user summaries embedded with them.
//!
//! Handlers pick one type with [`crate::models::user::UserClaims::is_librarian`] (the same
//! check used for weeding visibility). The public structs do not contain staff fields, so a
//! reader response cannot include them as blank options.

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
use super::user::{AccountTypeSlug, UserShort, UserStatus};
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
/// Includes weeding, copy notes, circulation status, the active loan id, and the
/// patron assigned when [`ItemAvailability`] is `onHold`.
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
    pub availability: ItemAvailability,
    pub can_hold_item: bool,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_user_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_user_name: Option<String>,
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
            availability: item.availability,
            can_hold_item: item.can_hold_item,
            hold_user_id: item.hold_user_id,
            hold_user_name: item.hold_user_name,
        }
    }
}

/// Full physical copy returned to readers and the OPAC.
///
/// No hold patron, notes, weeding, circulation status, loan id, or other staff field.
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
    pub availability: ItemPublicAvailability,
    pub can_hold_item: bool,
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
            availability: ItemPublicAvailability::from(item.availability),
            can_hold_item: item.can_hold_item,
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
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_user_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold_user_name: Option<String>,
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
            hold_user_id: item.hold_user_id,
            hold_user_name: item.hold_user_name,
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
            authors: biblio.authors.into_iter().map(AuthorPublic::from).collect(),
            series: biblio.series,
            collections: biblio.collections,
            edition: biblio.edition,
            items: biblio.items.into_iter().map(ItemPublicView::from).collect(),
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
                .map(ItemShortPublic::from)
                .collect(),
        }
    }
}

/// Patron summary returned to staff. `login` exists only on this type.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserShortStaff {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub login: Option<String>,
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
    pub user: Option<UserShortStaff>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    pub notes: Option<String>,
}

impl From<HoldDetails> for HoldDetailsStaff {
    fn from(hold: HoldDetails) -> Self {
        Self {
            id: hold.id,
            user_id: hold.user_id,
            biblio_id: hold.biblio_id,
            item_id: hold.item_id,
            pickup_site_id: hold.pickup_site_id,
            biblio: BiblioShortStaff::from(hold.biblio),
            user: hold.user.map(UserShortStaff::from),
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
            hold_user_name: Some("Ada".into()),
        };
        let body = serde_json::to_value(ItemShortPublic::from(item)).unwrap();
        assert_eq!(body["availability"], "unavailable");
        assert_eq!(body["canHoldItem"], false);
        assert!(body.get("weedingStatus").is_none());
        assert!(body.get("holdUserId").is_none());
        assert!(body.get("holdUserName").is_none());
        assert!(body.get("notes").is_none());
        assert!(body.get("login").is_none());
    }
}
