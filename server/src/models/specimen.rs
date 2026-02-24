//! Specimen (physical copy) model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Specimen borrow status (can it be borrowed?)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum SpecimenBorrowStatus {
    Borrowable = 98,
    NotBorrowable = 110,
}

impl From<i16> for SpecimenBorrowStatus {
    fn from(v: i16) -> Self {
        match v {
            98 => SpecimenBorrowStatus::Borrowable,
            _ => SpecimenBorrowStatus::NotBorrowable,
        }
    }
}

/// Specimen lifecycle status for soft delete
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[repr(i16)]
pub enum SpecimenStatus {
    Active = 0,
    Unavailable = 1,
    Deleted = 2,
}

impl From<i16> for SpecimenStatus {
    fn from(v: i16) -> Self {
        match v {
            0 => SpecimenStatus::Active,
            1 => SpecimenStatus::Unavailable,
            2 => SpecimenStatus::Deleted,
            _ => SpecimenStatus::Active,
        }
    }
}

impl Default for SpecimenStatus {
    fn default() -> Self {
        SpecimenStatus::Active
    }
}

/// Full specimen model from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Specimen {
    pub id: i32,
    pub id_item: Option<i32>,
    pub source_id: Option<i32>,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub place: Option<i16>,
    pub status: Option<i16>,  // Borrow status: 98=Borrowable, 110=NotBorrowable
    pub codestat: Option<i16>,
    pub notes: Option<String>,
    pub price: Option<String>,
    pub crea_date: Option<DateTime<Utc>>,
    pub modif_date: Option<DateTime<Utc>>,
    pub is_archive: Option<i32>,
    pub archive_date: Option<DateTime<Utc>>,
    pub lifecycle_status: i16,  // 0=Active, 1=Unavailable, 2=Deleted
    // Computed fields (populated when queried with JOINs, None otherwise)
    #[sqlx(default)]
    #[serde(default)]
    pub source_name: Option<String>,
    #[sqlx(default)]
    #[serde(default)]
    pub availability: Option<i64>,  // 0 = available, >0 = borrowed
}

/// Create specimen request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSpecimen {
    /// Barcode (optional). When set, must be unique across specimens.
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub place: Option<i16>,
    pub status: Option<i16>,
    pub notes: Option<String>,
    pub price: Option<String>,
    pub source_id: Option<i32>,
    pub source_name: Option<String>,
}

/// Update specimen request  
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSpecimen {
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub place: Option<i16>,
    pub status: Option<i16>,  // Borrow status
    pub notes: Option<String>,
    pub price: Option<String>,
    pub source_id: Option<i32>,
    pub is_archive: Option<i32>,
    pub lifecycle_status: Option<SpecimenStatus>,
}

