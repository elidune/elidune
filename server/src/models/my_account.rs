//! Patron account payloads for the caller's own loans and holds.
//!
//! These types do not embed catalog staff or public views. `GET /me/loans` and
//! `GET /me/holds` always return them for the authenticated user.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use super::biblio::MediaType;
use super::loan::LoanClosureReason;
use crate::renewal::RenewalRefusal;

/// One loan on the patron's own account.
///
/// Open loans omit [`MyLoanView::closure_reason`] and [`MyLoanView::found_at`].
/// `GET /me/loans?archived=true` returns closed loans and includes both.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyLoanView {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub barcode: Option<String>,
    pub title: Option<String>,
    pub media_type: MediaType,
    pub start_date: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub renewal_count: i16,
    pub can_renew: bool,
    /// [`RenewalRefusal`] from [`crate::renewal::renewal_eligibility`]. `card_lost`
    /// comes first and is not skipped by staff `force`. Account reasons
    /// (`account_blocked`, `subscription_expired`, `fines_over_threshold`) come
    /// before loan reasons. Staff `force` skips those three.
    pub renewal_refusal_code: Option<RenewalRefusal>,
    /// Library-local `YYYY-MM-DD` when `renewal_refusal_code` is `renewal_too_early`.
    pub renewal_opens_at: Option<String>,
    pub is_overdue: bool,
    /// Set on closed loans (`returned` or `lost`). Omitted on open loans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure_reason: Option<LoanClosureReason>,
    /// When a loan closed as lost had its copy found. Omitted on open loans.
    /// JSON null until the copy is found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub found_at: Option<Option<DateTime<Utc>>>,
}

/// Patron-facing hold status. `inTransit` is not stored on `holds`; it is set when an
/// active transit (`requested` or `in_transit`) is linked to the hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum MyHoldStatus {
    Pending,
    #[serde(rename = "to_pick")]
    ToPick,
    Ready,
    InTransit,
    Fulfilled,
    Cancelled,
    Expired,
}

/// One hold on the patron's own account.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyHoldView {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    pub title: Option<String>,
    pub status: MyHoldStatus,
    /// Pickup deadline. Set only when [`MyHoldView::status`] is `ready`.
    pub pickup_deadline: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub pickup_site_name: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    /// Note the patron and staff share. The staff-only note is never on this view.
    pub patron_note: Option<String>,
}
