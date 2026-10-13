//! Desk DTOs for lost / damaged / claimed-returned workflows.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::models::{
    circulation::{CirculationStatus, ClaimsResolveOutcome, DamageDisposition},
    fine::Fine,
};

/// Optional bill when marking lost (transition effect, not a status).
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkLostRequest {
    /// When true, create a replacement charge (item price or `amount`).
    #[serde(default)]
    pub bill: bool,
    /// Explicit replacement amount. Used when `bill` is true and set.
    pub amount: Option<Decimal>,
    /// When `bill` is true and `amount` is omitted, use `items.price` (default true).
    #[serde(default = "default_true")]
    pub use_item_price: bool,
    pub notes: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Mark the copy damaged. `disposition` is required so the loan rule is explicit.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkDamagedRequest {
    pub disposition: DamageDisposition,
    /// When true (or when `amount` is set), create a damage charge.
    #[serde(default)]
    pub bill: bool,
    pub amount: Option<Decimal>,
    pub notes: Option<String>,
}

/// Flag an active loan/item for the claims-returned queue. Does not close the loan.
/// Investigation only — `bill` is rejected if true.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkClaimedReturnedRequest {
    pub notes: Option<String>,
    /// Must stay false. Claimed-returned never bills (patron may have returned the copy).
    #[serde(default)]
    pub bill: bool,
}

/// Close a claims-returned case after an inventory check. Never bills.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveClaimsReturnedRequest {
    pub outcome: ClaimsResolveOutcome,
    /// Must be true — claims-returned is never cleared silently.
    pub inventory_checked: bool,
    /// Must stay false. Escalate via `POST /loans/{id}/lost` if a replacement bill is needed.
    #[serde(default)]
    pub bill: bool,
    pub notes: Option<String>,
}

/// Result of a staff circulation-exception action.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CirculationExceptionResponse {
    pub outcome: CirculationExceptionOutcome,
    pub item_status: CirculationStatus,
    pub borrowable: bool,
    pub loan_closed: bool,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub loan_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    pub charge: Option<Fine>,
}

/// Discriminator for the desk action that just ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum CirculationExceptionOutcome {
    Lost,
    Damaged,
    ClaimedReturned,
    ClaimsResolvedFound,
    ClaimsResolvedNotFound,
}

/// Desk view of a replacement charge when a lost copy is checked in.
/// `pending` is reported as `unpaid`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LostFineDeskStatus {
    Unpaid,
    Paid,
    Partial,
}

/// How staff close a lost-copy-found alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum LostFoundDisposition {
    /// Back on the shelf and lendable. Holds and transits run as on a normal return.
    Circulate,
    /// Accepted back, but not lendable. The replacement charge stays as it is.
    Damaged,
    /// Copy stays lost. The replacement charge is left as it is.
    Leave,
}

/// What happened to the linked replacement charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum LostFineAction {
    /// No linked charge was settled. Used when the copy stays lost, or when
    /// `circulate` finds no open replacement charge.
    None,
    Cancelled,
    Refunded,
    /// Collected part refunded; unpaid remainder cancelled.
    Partial,
    /// `damaged`: the replacement charge was not cancelled, refunded, or waived.
    Unchanged,
}

/// Lost-copy facts added to a return response. Present only when the copy is lost.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LostItemOnReturn {
    pub was_lost: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fine_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_status: Option<LostFineDeskStatus>,
    /// Replacement charge in cents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_amount: Option<i64>,
    /// Amount already collected, in cents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_amount: Option<i64>,
}

/// A lost copy with no active loan (the loan was closed when it was declared lost).
#[derive(Debug, Clone)]
pub struct ClosedLostLoan {
    pub item_id: i64,
    pub notice: LostItemOnReturn,
}

/// Staff decision after the lost-copy alert.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveLostFoundRequest {
    pub disposition: LostFoundDisposition,
    pub notes: Option<String>,
}

/// Result of putting a found lost copy back, marking it damaged, or leaving it lost.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LostFoundResolution {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub item_status: CirculationStatus,
    pub borrowable: bool,
    pub disposition: LostFoundDisposition,
    pub fine_action: LostFineAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fine_id: Option<i64>,
    /// Cents refunded. Zero when nothing had been collected, and always zero for `damaged`.
    pub refunded_amount: i64,
    /// Cents of the charge that were cancelled (the uncollected remainder). Always zero for `damaged`.
    pub cancelled_amount: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<crate::models::hold::Hold>,
}

/// One row on the claims-returned work queue.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimsReturnedQueueItem {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub loan_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    pub barcode: Option<String>,
    pub title: Option<String>,
    pub loan_start: chrono::DateTime<chrono::Utc>,
    pub expiry_at: Option<chrono::DateTime<chrono::Utc>>,
    pub item_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub notes: Option<String>,
}

/// How loudly the desk should show a successful scan note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeskNoticeLevel {
    /// Information only. The scan succeeded.
    #[serde(rename = "informational")]
    Informational,
    /// The scan was accepted but did not change the copy. Staff must follow up.
    #[serde(rename = "warning")]
    Warning,
}

/// Non-blocking note on a successful check-in.
///
/// `code` `item_not_on_loan` (`level` `informational`) means an available copy
/// had no active loan. The scan still ran (hold set aside, transit, or put back
/// on the shelf).
///
/// `code` `item_not_available_status` (`level` `warning`) means the copy is
/// lost, damaged, missing (`toSearch`), or withdrawn. The scan does not change
/// that status, assign a hold, or start transit. `circulationStatus` and
/// `weedingStatus` on the same response are the current values. Staff use the
/// dedicated action afterwards (for example lost-found).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeskNotice {
    #[schema(value_type = String)]
    pub code: &'static str,
    pub level: DeskNoticeLevel,
}

impl DeskNotice {
    #[must_use]
    pub fn item_not_on_loan() -> Self {
        Self {
            code: crate::error::error_code::ITEM_NOT_ON_LOAN,
            level: DeskNoticeLevel::Informational,
        }
    }

    #[must_use]
    pub fn item_not_available_status() -> Self {
        Self {
            code: crate::error::error_code::ITEM_NOT_AVAILABLE_STATUS,
            level: DeskNoticeLevel::Warning,
        }
    }
}
