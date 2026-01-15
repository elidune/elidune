//! Item (catalog entry) model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

use super::author::AuthorWithFunction;
use super::specimen::Specimen;

/// Item status for soft delete
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[repr(i16)]
pub enum ItemStatus {
    Active = 0,
    Unavailable = 1,
    Deleted = 2,
}

impl From<i16> for ItemStatus {
    fn from(v: i16) -> Self {
        match v {
            0 => ItemStatus::Active,
            1 => ItemStatus::Unavailable,
            2 => ItemStatus::Deleted,
            _ => ItemStatus::Active,
        }
    }
}

impl Default for ItemStatus {
    fn default() -> Self {
        ItemStatus::Active
    }
}

/// Media type codes (matching original C implementation)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    #[serde(rename = "u")]
    Unknown,
    #[serde(rename = "b")]
    PrintedText,
    #[serde(rename = "bc")]
    Comics,
    #[serde(rename = "p")]
    Periodic,
    #[serde(rename = "v")]
    Video,
    #[serde(rename = "vt")]
    VideoTape,
    #[serde(rename = "vd")]
    VideoDvd,
    #[serde(rename = "a")]
    Audio,
    #[serde(rename = "am")]
    AudioMusic,
    #[serde(rename = "amt")]
    AudioMusicTape,
    #[serde(rename = "amc")]
    AudioMusicCd,
    #[serde(rename = "an")]
    AudioNonMusic,
    #[serde(rename = "c")]
    CdRom,
    #[serde(rename = "i")]
    Images,
    #[serde(rename = "m")]
    Multimedia,
}

impl From<&str> for MediaType {
    fn from(s: &str) -> Self {
        match s {
            "b" => MediaType::PrintedText,
            "bc" => MediaType::Comics,
            "p" => MediaType::Periodic,
            "v" => MediaType::Video,
            "vt" => MediaType::VideoTape,
            "vd" => MediaType::VideoDvd,
            "a" => MediaType::Audio,
            "am" => MediaType::AudioMusic,
            "amt" => MediaType::AudioMusicTape,
            "amc" => MediaType::AudioMusicCd,
            "an" => MediaType::AudioNonMusic,
            "c" => MediaType::CdRom,
            "i" => MediaType::Images,
            "m" => MediaType::Multimedia,
            _ => MediaType::Unknown,
        }
    }
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Public type (audience)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum PublicType {
    Adult = 97,
    Children = 106,
    Unknown = 117,
}

impl From<i16> for PublicType {
    fn from(v: i16) -> Self {
        match v {
            97 => PublicType::Adult,
            106 => PublicType::Children,
            _ => PublicType::Unknown,
        }
    }
}

/// Full item model from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Item {
    pub id: i32,
    pub media_type: Option<String>,
    pub identification: Option<String>,
    pub price: Option<String>,
    pub barcode: Option<String>,
    pub dewey: Option<String>,
    pub publication_date: Option<String>,
    pub lang: Option<i16>,
    pub lang_orig: Option<i16>,
    pub title1: Option<String>,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub title4: Option<String>,
    pub genre: Option<i16>,
    pub subject: Option<String>,
    pub public_type: Option<i16>,
    pub nb_pages: Option<String>,
    pub format: Option<String>,
    pub content: Option<String>,
    pub addon: Option<String>,
    pub abstract_: Option<String>,
    pub notes: Option<String>,
    pub keywords: Option<String>,
    pub nb_specimens: Option<i16>,
    pub state: Option<String>,
    pub is_archive: Option<i16>,
    pub is_valid: Option<i16>,
    pub lifecycle_status: i16,
    pub crea_date: Option<DateTime<Utc>>,
    pub modif_date: Option<DateTime<Utc>>,
    pub archived_date: Option<DateTime<Utc>>,
    // Relations (loaded separately)
    #[sqlx(skip)]
    pub authors1: Vec<AuthorWithFunction>,
    #[sqlx(skip)]
    pub authors2: Vec<AuthorWithFunction>,
    #[sqlx(skip)]
    pub authors3: Vec<AuthorWithFunction>,
    #[sqlx(skip)]
    pub serie: Option<Serie>,
    #[sqlx(skip)]
    pub collection: Option<Collection>,
    #[sqlx(skip)]
    pub edition: Option<Edition>,
    #[sqlx(skip)]
    pub specimens: Vec<Specimen>,
}

/// Short item representation for lists
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ItemShort {
    pub id: i32,
    pub media_type: Option<String>,
    pub identification: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub status: Option<i16>,
    pub is_local: Option<i16>,
    pub is_archive: Option<i16>,
    pub is_valid: Option<i16>,
    #[sqlx(skip)]
    pub authors: Vec<AuthorWithFunction>,
    #[sqlx(skip)]
    pub source_name: Option<String>,
}

/// Serie model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Serie {
    pub id: i32,
    pub name: Option<String>,
    pub volume_number: Option<i16>,
}

/// Collection model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Collection {
    pub id: i32,
    pub title1: Option<String>,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub issn: Option<String>,
    pub number_sub: Option<i16>,
    pub volume_number: Option<i16>,
}

/// Edition (publisher) model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Edition {
    pub id: i32,
    pub name: Option<String>,
    pub place: Option<String>,
    pub date: Option<String>,
}

/// Item query parameters
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ItemQuery {
    pub media_type: Option<String>,
    pub identification: Option<String>,
    pub barcode: Option<String>,
    pub author: Option<String>,
    pub title: Option<String>,
    pub editor: Option<String>,
    pub lang: Option<String>,
    pub subject: Option<String>,
    pub content: Option<String>,
    pub keywords: Option<String>,
    pub freesearch: Option<String>,
    pub genre: Option<String>,
    pub public_type: Option<String>,
    pub archive: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Create item request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItem {
    pub media_type: Option<String>,
    pub identification: Option<String>,
    pub price: Option<String>,
    pub barcode: Option<String>,
    pub dewey: Option<String>,
    pub publication_date: Option<String>,
    pub lang: Option<i16>,
    pub lang_orig: Option<i16>,
    pub title1: String,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub title4: Option<String>,
    pub genre: Option<i16>,
    pub subject: Option<String>,
    pub public_type: Option<i16>,
    pub nb_pages: Option<String>,
    pub format: Option<String>,
    pub content: Option<String>,
    pub addon: Option<String>,
    pub abstract_: Option<String>,
    pub notes: Option<String>,
    pub keywords: Option<String>,
    pub is_valid: Option<i16>,
    // Authors
    pub authors1: Option<Vec<CreateItemAuthor>>,
    pub authors2: Option<Vec<CreateItemAuthor>>,
    pub authors3: Option<Vec<CreateItemAuthor>>,
    // Related entities
    pub serie: Option<CreateSerie>,
    pub collection: Option<CreateCollection>,
    pub edition: Option<CreateEdition>,
    // Specimens
    pub specimens: Option<Vec<super::specimen::CreateSpecimen>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItemAuthor {
    pub id: Option<i32>,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub function: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSerie {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub volume_number: Option<i16>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCollection {
    pub id: Option<i32>,
    pub title1: Option<String>,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub issn: Option<String>,
    pub number_sub: Option<i16>,
    pub volume_number: Option<i16>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEdition {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub place: Option<String>,
    pub date: Option<String>,
}

/// Update item request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateItem {
    pub media_type: Option<String>,
    pub identification: Option<String>,
    pub price: Option<String>,
    pub barcode: Option<String>,
    pub dewey: Option<String>,
    pub publication_date: Option<String>,
    pub lang: Option<i16>,
    pub lang_orig: Option<i16>,
    pub title1: Option<String>,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub title4: Option<String>,
    pub genre: Option<i16>,
    pub subject: Option<String>,
    pub public_type: Option<i16>,
    pub nb_pages: Option<String>,
    pub format: Option<String>,
    pub content: Option<String>,
    pub addon: Option<String>,
    pub abstract_: Option<String>,
    pub notes: Option<String>,
    pub keywords: Option<String>,
    pub is_archive: Option<i16>,
    pub is_valid: Option<i16>,
    pub lifecycle_status: Option<ItemStatus>,
    pub authors1: Option<Vec<CreateItemAuthor>>,
    pub authors2: Option<Vec<CreateItemAuthor>>,
    pub authors3: Option<Vec<CreateItemAuthor>>,
    pub serie: Option<CreateSerie>,
    pub collection: Option<CreateCollection>,
    pub edition: Option<CreateEdition>,
}

