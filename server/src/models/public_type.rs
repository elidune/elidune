//! Public type model (borrower audience: child, adult, school, staff, senior)

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sqlx::FromRow;
use utoipa::ToSchema;

use super::loan::LoanSettingsRenewAt;

/// Public type from database (borrower audience category)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicType {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub name: String,
    pub label: String,
    pub subscription_duration_days: Option<i32>,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
    /// Subscription price in cents (e.g. 1500 = 15.00€)
    pub subscription_price: Option<i32>,
}

/// Per-media loan settings for a public type: on the default row (`media_type` IS NULL), `nb_max` caps total active loans;
/// on a medium-specific row, `nb_max` caps loans for that medium.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicTypeLoanSettings {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub public_type_id: i64,
    /// `None` = default loan rules row for this public type (`media_type` IS NULL in DB).
    pub media_type: Option<String>,
    pub duration: Option<i16>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    /// When set, overrides [`crate::models::loan::LoanSettings`] `renew_at` for this public type and media type.
    pub renew_at: Option<LoanSettingsRenewAt>,
}

/// Create public type request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePublicType {
    pub name: String,
    pub label: String,
    pub subscription_duration_days: Option<i32>,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
    pub subscription_price: Option<i32>,
}

/// One row when replacing all loan settings for a public type (`mediaType` null or omitted = default row).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicTypeLoanSettingInput {
    /// `None` = audience-wide default row (`media_type` IS NULL in DB). On that row, `nbMax` caps total active loans.
    pub media_type: Option<String>,
    pub duration: Option<i16>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    /// `None` = inherit from global `loans_settings` for that media type.
    pub renew_at: Option<LoanSettingsRenewAt>,
}

/// Replaces every `public_type_loan_settings` row for this audience with the given list (full snapshot).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplacePublicTypeLoanSettingsRequest {
    pub settings: Vec<PublicTypeLoanSettingInput>,
}

/// Update public type request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePublicType {
    pub name: Option<String>,
    pub label: Option<String>,
    pub subscription_duration_days: Option<i32>,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
    pub subscription_price: Option<i32>,
}
