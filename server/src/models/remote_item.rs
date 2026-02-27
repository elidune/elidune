//! Remote item (cached Z39.50 record) models

use chrono::{DateTime, Utc};
use z3950_rs::marc_rs::Record as MarcRecord;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::{author::AuthorWithFunction, item::Item};

/// Full remote item model matching `remote_items` table
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ItemRemote {
    #[serde(default)]
    pub id: Option<i32>,
    pub media_type: Option<String>,
    pub isbn: Option<String>,
    pub price: Option<String>,
    pub barcode: Option<String>,
    pub publication_date: Option<String>,
    pub lang: Option<i16>,
    pub lang_orig: Option<i16>,
    pub title1: Option<String>,
    pub title2: Option<String>,
    pub title3: Option<String>,
    pub title4: Option<String>,

    // Legacy author linkage columns (kept for compatibility)
    pub author1_ids: Option<Vec<i32>>,
    pub author1_functions: Option<String>,
    pub author2_ids: Option<Vec<i32>>,
    pub author2_functions: Option<String>,
    pub author3_ids: Option<Vec<i32>>,
    pub author3_functions: Option<String>,

    pub serie_id: Option<i32>,
    pub serie_vol_number: Option<i16>,
    pub collection_id: Option<i32>,
    pub collection_number_sub: Option<i16>,
    pub collection_vol_number: Option<i16>,
    pub source_id: Option<i32>,
   

    pub genre: Option<i16>,
    pub subject: Option<String>,
    pub public_type: Option<i16>,

    pub edition_id: Option<i32>,
    pub edition_date: Option<String>,

    pub nb_pages: Option<String>,
    pub format: Option<String>,
    pub content: Option<String>,
    pub addon: Option<String>,

    #[serde(rename = "abstract")]
    pub abstract_: Option<String>,

    pub notes: Option<String>,
    pub keywords: Option<String>,

    /// Source name (historically stored in `state`)
    pub state: Option<String>,

    pub is_archive: Option<i16>,
    pub archived_timestamp: Option<i64>,
    pub is_valid: Option<i16>,

    pub modif_date: Option<DateTime<Utc>>,
    pub crea_date: Option<DateTime<Utc>>,

    // New JSON author columns (stored as JSONB)
    pub authors1_json: Option<serde_json::Value>,
    pub authors2_json: Option<serde_json::Value>,
    pub authors3_json: Option<serde_json::Value>,
}

/// Short remote item representation (list-friendly)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ItemRemoteShort {
    pub id: i32,
    pub media_type: Option<String>,
    pub isbn: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub is_archive: Option<i16>,
    pub is_valid: Option<i16>,
    pub nb_available: Option<i16>,
    pub authors: Vec<AuthorWithFunction>,
    pub source_name: Option<String>,
}

impl From<MarcRecord> for ItemRemote {
    fn from(record: MarcRecord) -> Self {
        let item: Item = record.into();
        item.into()
    }
}

impl From<Item> for ItemRemote {
    fn from(item: Item) -> Self {
        let authors1_json = if !item.authors.is_empty() {
            serde_json::to_value(&item.authors).ok()
        } else {
            None
        };

        Self {
            id: None,
            media_type: item.media_type,
            isbn: item.isbn,
            price: item.price,
            barcode: item.barcode,
            publication_date: item.publication_date,
            lang: item.lang,
            lang_orig: item.lang_orig,
            title1: item.title,
            title2: None,
            title3: None,
            title4: None,
            author1_ids: None,
            author1_functions: None,
            author2_ids: None,
            author2_functions: None,
            author3_ids: None,
            author3_functions: None,
            serie_id: None,
            serie_vol_number: item.series_volume_number,
            collection_id: None,
            collection_number_sub: item.collection_sequence_number,
            collection_vol_number: item.collection_volume_number,
            source_id: None,
            genre: item.genre,
            subject: item.subject,
            public_type: item.audience_type,
            edition_id: None,
            edition_date: None,
            nb_pages: item.page_extent,
            format: item.format,
            content: item.table_of_contents,
            addon: item.accompanying_material,
            abstract_: item.abstract_,
            notes: item.notes,
            keywords: item.keywords,
            state: item.state,
            is_archive: item.archived_at.map(|_| 1i16),
            archived_timestamp: item.archived_at.map(|d| d.timestamp()),
            is_valid: item.is_valid,
            modif_date: item.updated_at,
            crea_date: item.created_at,
            authors1_json,
            authors2_json: None,
            authors3_json: None,
        }
    }
}

impl From<ItemRemote> for ItemRemoteShort {
    fn from(item: ItemRemote) -> Self {
        let mut authors = Vec::new();

        if let Some(json) = item.authors1_json {
            if let Ok(auths) = serde_json::from_value::<Vec<AuthorWithFunction>>(json) {
                authors.extend(auths);
            }
        }
        if let Some(json) = item.authors2_json {
            if let Ok(auths) = serde_json::from_value::<Vec<AuthorWithFunction>>(json) {
                authors.extend(auths);
            }
        }
        if let Some(json) = item.authors3_json {
            if let Ok(auths) = serde_json::from_value::<Vec<AuthorWithFunction>>(json) {
                authors.extend(auths);
            }
        }

        Self {
            id: item.id.unwrap_or_default(),
            media_type: item.media_type,
            isbn: item.isbn,
            title: item.title1,
            date: item.publication_date,
            is_archive: item.is_archive,
            is_valid: item.is_valid,
            nb_available: None, // Remote items don't have local availability
            authors,
            source_name: item.state,
        }
    }
}

impl From<ItemRemoteShort> for super::item::ItemShort {
    fn from(item: ItemRemoteShort) -> Self {
        Self {
            id: item.id,
            media_type: item.media_type,
            isbn: item.isbn,
            title: item.title,
            date: item.date,
            status: Some(0),
            is_local: Some(0),
            is_valid: item.is_valid,
            archived_at: None,
            nb_specimens: None, // Remote items don't have local specimens
            nb_available: item.nb_available,
            author: item.authors.first().cloned(),
            source_name: item.source_name,
        }
    }
}

