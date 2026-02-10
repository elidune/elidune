//! Loan (borrow) model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::item::ItemShort;
use super::user::UserShort;

/// Loan model from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Loan {
    pub id: i32,
    pub user_id: i32,
    pub specimen_id: i32,
    pub item_id: Option<i32>,
    pub date: DateTime<Utc>,
    pub renew_date: Option<DateTime<Utc>>,
    pub nb_renews: Option<i16>,
    pub issue_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub returned_date: Option<DateTime<Utc>>,
}

/// Loan with full details for display
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoanDetails {
    pub id: i32,
    pub start_date: DateTime<Utc>,
    pub issue_date: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub item: ItemShort,
    pub user: Option<UserShort>,
    pub specimen_identification: Option<String>,
    pub is_overdue: bool,
}

/// Create loan request
#[derive(Debug, Deserialize)]
pub struct CreateLoan {
    pub user_id: i32,
    pub specimen_id: Option<i32>,
    pub specimen_identification: Option<String>,
    pub force: bool,
}

/// Loan settings by media type
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoanSettings {
    pub id: i32,
    pub media_type: Option<String>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    pub duration: Option<i16>,
    pub notes: Option<String>,
    pub account_type: Option<String>,
}

/// Archived loan for statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LoanArchive {
    pub id: i32,
    pub item_id: i32,
    pub specimen_id: Option<i32>,
    pub date: DateTime<Utc>,
    pub nb_renews: Option<i16>,
    pub issue_date: Option<DateTime<Utc>>,
    pub returned_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub borrower_public_type: Option<i32>,
    pub addr_city: Option<String>,
    pub account_type: Option<String>,
}

