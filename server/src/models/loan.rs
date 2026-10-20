//! Loan (borrow) model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use super::biblio::{Biblio, BiblioShort, MediaType};
use super::user::UserShort;

/// Loan model from database
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Loan {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub item_id: i64,
    pub date: DateTime<Utc>,
    pub renew_at: Option<DateTime<Utc>>,
    pub nb_renews: Option<i16>,
    pub expiry_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub returned_at: Option<DateTime<Utc>>,
    pub last_reminder_sent_at: Option<DateTime<Utc>>,
    pub reminder_count: Option<i32>,
    /// Desk site at checkout (`sources.id`). Null on loans that predate the column.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub site_id: Option<i64>,
}

/// Why a loan was closed. Finding a lost copy does not change this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoanClosureReason {
    /// Physical return, damaged return, or claims-returned resolved as found.
    Returned,
    /// Declared lost, or claims-returned resolved as not found.
    Lost,
}

impl LoanClosureReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Returned => "returned",
            Self::Lost => "lost",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "returned" => Some(Self::Returned),
            "lost" => Some(Self::Lost),
            _ => None,
        }
    }
}

/// Loan with full details for display
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanDetails {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// Borrowed specimen (`items.id`).
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    /// Desk site at checkout. Null when the loan has no recorded site.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub renewal_date: Option<DateTime<Utc>>,
    pub nb_renews: i16,
    pub returned_at: Option<DateTime<Utc>>,
    /// Set on closed loans. Omitted on open loans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reason: Option<LoanClosureReason>,
    /// When a loan closed as [`LoanClosureReason::Lost`] had its copy found.
    /// `None` omits the field (open loans). `Some(None)` is JSON null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub found_at: Option<Option<DateTime<Utc>>>,
    pub biblio: BiblioShort,
    pub user: Option<UserShort>,
    pub item_identification: Option<String>,
    pub is_overdue: bool,
}

/// Result of [`crate::repository::Repository::loans_return`]: archived loan details and optional hold advanced to `ready`.
#[derive(Debug, Clone)]
pub struct LoanReturnOutcome {
    pub details: LoanDetails,
    pub readied_hold: Option<crate::models::hold::Hold>,
    /// Outcome of the hold-ready email attempt (when a hold was advanced to `ready`).
    pub hold_ready_email: Option<HoldReadyEmailOutcome>,
    /// Set when the copy was lost. The return does not change that status.
    pub lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
    /// True when this return checked in a deposit copy whose batch is `returning`.
    pub return_to_lender: bool,
}

/// Hold-ready notification email attempt after a loan return.
#[derive(Debug, Clone)]
pub struct HoldReadyEmailOutcome {
    pub email: Option<String>,
    pub send_error: Option<String>,
}

/// Result of [`crate::repository::Repository::loans_create`].
#[derive(Debug, Clone)]
pub struct LoanCreateOutcome {
    pub loan_id: i64,
    pub expiry_at: DateTime<Utc>,
    /// Desk site stored on the loan. Null when checkout did not send one.
    pub site_id: Option<i64>,
    /// Active hold fulfilled when the patron checked out (not when `force` clears holds).
    pub fulfilled_hold_id: Option<i64>,
    /// Hold left in the queue because `force` skipped `item_held_for_other`.
    pub preserved_hold_id: Option<i64>,
    /// True when the due date was lowered to the deposit batch `returnDueOn`.
    pub due_date_capped_by_deposit: bool,
}

/// How the new due date is computed when a loan is renewed (`loans_settings.renew_at`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum LoanSettingsRenewAt {
    /// New due date = instant of renewal + loan duration.
    Now,
    /// New due date = current due date + loan duration.
    #[default]
    AtDueDate,
}

impl LoanSettingsRenewAt {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::AtDueDate => "at_due_date",
        }
    }
}

impl From<&str> for LoanSettingsRenewAt {
    fn from(s: &str) -> Self {
        match s {
            "now" => Self::Now,
            _ => Self::AtDueDate,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for LoanSettingsRenewAt {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for LoanSettingsRenewAt {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let s: String = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(Self::from(s.as_str()))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for LoanSettingsRenewAt {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str().to_string(), buf)
    }
}

/// Create loan request
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLoan {
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub item_id: Option<i64>,
    pub item_identification: Option<String>,
    pub force: bool,
    /// Desk site (`sources.id`). Null leaves `loans.site_id` null; the calendar
    /// then follows the library default site.
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub site_id: Option<i64>,
    /// Staff reason for forcing `subscription_expired` or `item_held_for_other`.
    pub override_reason: Option<String>,
}

/// Loan settings: `nb_max` on the default row (`media_type` IS NULL) caps **all** active loans;
/// on a per-media row, `nb_max` caps loans for that medium only.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LoanSettings {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    pub media_type: Option<MediaType>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    pub duration: Option<i16>,
    pub renew_at: Option<LoanSettingsRenewAt>,
    /// Days before the due date when renewal becomes allowed. `0` = on the due date or later.
    pub renew_window_days: i16,
    pub notes: Option<String>,
}

/// One loan row for MARC export (full list, no pagination).
#[derive(Debug, Clone)]
pub struct LoanMarcExportRow {
    pub biblio: Biblio,
    pub start_date: DateTime<Utc>,
    pub expiry_at: DateTime<Utc>,
    pub returned_at: Option<DateTime<Utc>>,
}

/// Maximum loans included in a single MARC export response (safety cap).
pub const LOANS_MARC_EXPORT_MAX: usize = 2000;

/// File format for [`crate::services::loans::LoansService::export_user_loans_marc_file`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LoanMarcExportFormat {
    #[default]
    Json,
    Marc21,
    Unimarc,
    Marcxml,
}

/// Binary MARC encoding for ISO2709 export (query param `encoding`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LoanMarcExportEncoding {
    #[default]
    Utf8,
    Marc8,
}

/// Archived loan for statistics
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LoanArchive {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub item_id: Option<i64>,
    pub date: DateTime<Utc>,
    pub nb_renews: Option<i16>,
    pub expiry_at: Option<DateTime<Utc>>,
    pub returned_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub borrower_public_type: Option<i64>,
    pub addr_city: Option<String>,
    pub account_type: Option<String>,
}

#[cfg(test)]
mod renew_at_tests {
    use super::LoanSettingsRenewAt;

    #[test]
    fn default_renew_anchor_is_the_current_due_date() {
        assert_eq!(LoanSettingsRenewAt::default(), LoanSettingsRenewAt::AtDueDate);
        assert_eq!(LoanSettingsRenewAt::from("now"), LoanSettingsRenewAt::Now);
        assert_eq!(LoanSettingsRenewAt::from("at_due_date"), LoanSettingsRenewAt::AtDueDate);
        assert_eq!(LoanSettingsRenewAt::from("unexpected"), LoanSettingsRenewAt::AtDueDate);
    }
}

#[cfg(test)]
mod closure_reason_tests {
    use super::LoanClosureReason;

    #[test]
    fn closure_reason_serializes_as_returned_or_lost() {
        assert_eq!(serde_json::to_value(LoanClosureReason::Returned).unwrap(), serde_json::json!("returned"));
        assert_eq!(serde_json::to_value(LoanClosureReason::Lost).unwrap(), serde_json::json!("lost"));
        assert_eq!(LoanClosureReason::parse("returned"), Some(LoanClosureReason::Returned));
        assert_eq!(LoanClosureReason::parse("lost"), Some(LoanClosureReason::Lost));
        assert_eq!(LoanClosureReason::parse("claimed"), None);
    }
}
