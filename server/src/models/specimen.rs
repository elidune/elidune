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

/// Full specimen model from database.
/// Soft delete is tracked solely via `archived_at` (NULL = active, set = archived).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Specimen {
    pub id: i32,
    pub item_id: Option<i32>,
    pub source_id: Option<i32>,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    pub borrow_status: Option<i16>,
    pub circulation_status: Option<i16>,
    pub notes: Option<String>,
    pub price: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    #[serde(default)]
    pub source_name: Option<String>,
    #[sqlx(default)]
    #[serde(default)]
    pub availability: Option<i64>,
}

/// Create specimen request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSpecimen {
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    pub borrow_status: Option<i16>,
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
    pub volume_designation: Option<String>,
    pub place: Option<i16>,
    pub borrow_status: Option<i16>,
    pub notes: Option<String>,
    pub price: Option<String>,
    pub source_id: Option<i32>,
}

