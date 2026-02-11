//! Equipment model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Equipment record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Equipment {
    pub id: i32,
    /// Equipment name / description
    pub name: String,
    /// Type (0=computer, 1=tablet, 2=ereader, 3=other)
    pub equipment_type: i16,
    /// Whether the equipment has internet access
    pub has_internet: Option<bool>,
    /// Whether the equipment is public-facing (vs staff-only)
    pub is_public: Option<bool>,
    /// Number of units
    pub quantity: Option<i32>,
    /// Status (0=active, 1=maintenance, 2=retired)
    pub status: Option<i16>,
    pub notes: Option<String>,
    pub crea_date: Option<DateTime<Utc>>,
    pub modif_date: Option<DateTime<Utc>>,
}

/// Create equipment request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEquipment {
    pub name: String,
    /// Type (0=computer, 1=tablet, 2=ereader, 3=other)
    pub equipment_type: Option<i16>,
    pub has_internet: Option<bool>,
    pub is_public: Option<bool>,
    pub quantity: Option<i32>,
    pub notes: Option<String>,
}

/// Update equipment request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEquipment {
    pub name: Option<String>,
    pub equipment_type: Option<i16>,
    pub has_internet: Option<bool>,
    pub is_public: Option<bool>,
    pub quantity: Option<i32>,
    pub status: Option<i16>,
    pub notes: Option<String>,
}
