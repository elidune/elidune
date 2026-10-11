//! Loan settings DTOs (shared by API and services).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{biblio::MediaType, loan::LoanSettingsRenewAt};

/// Loan rules (`loans_settings`): per-document-type overrides plus one global default row.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanSettingsDto {
    /// `null` = global default row (`media_type` IS NULL in DB).
    pub media_type: Option<MediaType>,
    pub max_loans: i16,
    pub max_renewals: i16,
    pub duration_days: i16,
    #[serde(default)]
    pub renew_at: LoanSettingsRenewAt,
    /// Days before the due date when a renewal is allowed. Default 7.
    /// `0` allows renewal only on the due date or once the loan is overdue.
    #[serde(default = "default_renew_window_days")]
    pub renew_window_days: i16,
}

fn default_renew_window_days() -> i16 {
    7
}

/// Partial update of global loan rules.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLoanSettingsRequest {
    pub loan_settings: Option<Vec<LoanSettingsDto>>,
}
