//! Fine policy and accrual DTOs (shared by API and services).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::models::fine::Fine;

/// Global unpaid-fine threshold that gates checkout and renew.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CirculationFinePolicy {
    /// Block new loans and renewals when unpaid fines exceed this amount.
    /// Default `0` blocks any positive unpaid balance. Paid and waived fines do not count.
    pub unpaid_fine_threshold: Decimal,
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
    pub daily_rate: Decimal,
    pub max_amount: Option<Decimal>,
    pub amount: Decimal,
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
