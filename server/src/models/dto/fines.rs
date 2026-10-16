//! Fine policy and accrual DTOs (shared by API and services).

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::models::fine::{euro_string, Cents, Fine, GlobalUnpaidThresholdMode};
use crate::models::serde_util::reject_null;

/// Global unpaid-fine gate for checkout, renew, and the card-scan `unpaid_fines` reason.
///
/// `unpaidThresholdMode` is `none` (no amount-based block) or `amount`.
/// `unpaidThresholdCents` is present only for `amount`. `0` blocks any positive unpaid balance.
/// Blocking is strict: the unpaid remainder must be above the amount.
/// Paid and waived fines do not count.
/// A public type uses its own mode: `amount` uses that type's cents, `none` never blocks
/// on amount, and `inherit` uses this global mode.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CirculationFinePolicy {
    pub unpaid_threshold_mode: GlobalUnpaidThresholdMode,
    /// Integer cents. Required with mode `amount`. Omitted for mode `none`.
    /// `0` blocks any positive unpaid balance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpaid_threshold_cents: Option<i64>,
}

/// PUT `/fines/policy`. An omitted `unpaidThresholdMode` leaves the stored policy unchanged.
/// JSON `null` is rejected. Mode `amount` requires `unpaidThresholdCents`.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateCirculationFinePolicy {
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_mode: Option<GlobalUnpaidThresholdMode>,
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_cents: Option<i64>,
}

/// How an accrue attempt resolved for one loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum AccrueOutcome {
    Created,
    Updated,
    Unchanged,
    SkippedGrace,
    SkippedNotOverdue,
    SkippedReturned,
    SkippedDeletedUser,
}

/// Desk-visible explanation of the calculated amount (days × rate, grace, cap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccrualBreakdown {
    pub overdue_days: i64,
    pub grace_days: i32,
    pub billable_days: i64,
    /// Daily rate as a decimal-euro string (`"0.20"`). Stored as integer cents.
    #[schema(value_type = String, example = "0.20")]
    #[serde(with = "euro_string")]
    pub daily_rate: Cents,
    /// Cap as a decimal-euro string. Null means no cap. Stored as integer cents.
    #[schema(value_type = Option<String>, example = "15.00")]
    #[serde(with = "euro_string::opt")]
    pub max_amount: Option<Cents>,
    /// Accrued charge as a decimal-euro string. Stored as integer cents.
    #[schema(value_type = String, example = "3.50")]
    #[serde(with = "euro_string")]
    pub amount: Cents,
    pub capped: bool,
    pub media_type: Option<String>,
}

impl AccrualBreakdown {
    /// Human-readable note stored on the fine row for desk review.
    pub fn to_notes(&self) -> String {
        let media = self.media_type.as_deref().unwrap_or("default");
        let cap = match self.max_amount {
            Some(max) if self.capped => format!(" (capped at {max})"),
            Some(max) => format!(" (cap {max})"),
            None => String::new(),
        };
        format!(
            "Accrual: {overdue} overdue days − {grace} grace = {billable} × {rate}{cap} = {amount} [{media}]",
            overdue = self.overdue_days,
            grace = self.grace_days,
            billable = self.billable_days,
            rate = self.daily_rate,
            amount = self.amount,
        )
    }
}

/// Result of accruing one loan (staff API and scheduler).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccrueLoanResult {
    pub outcome: AccrueOutcome,
    pub fine: Option<Fine>,
    pub breakdown: Option<AccrualBreakdown>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub loan_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
}

/// Summary of a batch accrue (patron or all overdue loans).
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccrueBatchReport {
    pub created: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub skipped: u32,
    pub errors: Vec<AccrueBatchError>,
    pub results: Vec<AccrueLoanResult>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccrueBatchError {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub loan_id: i64,
    pub error_message: String,
}

/// Optional body for `POST /fines/accrue` (omit `userId` to accrue all overdue loans).
#[serde_as]
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AccrueFinesRequest {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub user_id: Option<i64>,
}
