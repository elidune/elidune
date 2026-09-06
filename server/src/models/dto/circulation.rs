//! Desk DTOs for lost / damaged / claimed-returned workflows.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::models::{
    circulation::{CirculationStatus, ClaimsResolveOutcome, DamageDisposition},
    fine::Fine,
};

/// Optional bill when marking lost or resolving a claim as not found.
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
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkClaimedReturnedRequest {
    pub notes: Option<String>,
}

/// Close a claims-returned case after an inventory check.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveClaimsReturnedRequest {
    pub outcome: ClaimsResolveOutcome,
    /// Must be true — claims-returned is never cleared silently.
    pub inventory_checked: bool,
    /// Replacement bill when `outcome` is `notFound`.
    #[serde(default)]
    pub bill: bool,
    pub amount: Option<Decimal>,
    #[serde(default = "default_true")]
    pub use_item_price: bool,
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
