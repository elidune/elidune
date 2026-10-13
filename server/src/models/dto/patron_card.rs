//! Desk scan of a patron library card.

use chrono::NaiveDate;
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::models::user::User;
use crate::patron_eligibility::PatronBlockReason;

/// Circulation snapshot returned with a card scan.
///
/// `blocked` follows checkout without `force`. `overdueLoans` in `blockReasons`
/// does not set `blocked` by itself, because checkout does not refuse overdue loans.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatronLoanStatus {
    pub subscription_valid: bool,
    /// Library-local calendar date of `users.expiry_at`. Null when the patron has no expiry.
    /// Always present in JSON, including when null.
    pub subscription_expires_at: Option<NaiveDate>,
    /// True when checkout without `force` would refuse this patron.
    pub blocked: bool,
    pub block_reasons: Vec<PatronBlockReason>,
    pub active_loans: i64,
    pub overdue_loans: i64,
    /// Audience total loan cap. Null only when no total cap is configured.
    /// The settings resolver falls back to 5, so this is a number with current rules.
    pub max_loans: Option<i32>,
    /// Unpaid fine remainder in integer cents.
    /// `unpaid_fines` is set only when this is strictly above the effective threshold.
    /// Public type `amount` uses that value, `none` does not block, `inherit` follows
    /// the global mode (`none` does not block, `amount` uses its value).
    pub unpaid_fines_cents: i64,
    /// Holds in status `ready` for this patron.
    ///
    /// Filtered to the calling agent's site when the agent has one (`holds.pickup_site_id`).
    /// Users and JWTs store no site, so the filter is unset and every ready hold counts,
    /// including holds with a null pickup site.
    pub ready_holds: i64,
}

/// Loans staff without users read. No name, email, phone, address, or login.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeskCardLookup {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// `users.barcode`. Null when the stored card is blank.
    pub card_number: Option<String>,
    /// Uppercase first letter of `firstname` plus first letter of `lastname`.
    pub initials: String,
    pub loan_status: PatronLoanStatus,
}

/// Users read or write. The existing [`User`] body plus [`PatronLoanStatus`].
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaffCardLookup {
    #[serde(flatten)]
    pub user: User,
    pub loan_status: PatronLoanStatus,
}

/// `GET /users/by-card/{card_number}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(untagged)]
pub enum CardLookupResponse {
    /// `users_rights` read or write.
    Staff(Box<StaffCardLookup>),
    /// `loans_rights` read or write, without users read.
    Desk(DeskCardLookup),
}
