//! Deposit MARC import duplicate detection and resolution.

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

/// Why a lender barcode conflicts with existing stock during deposit import.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DepositDuplicateConflictKind {
    /// An active copy already uses the lender barcode as desk barcode.
    ActiveBarcode,
    /// A returned archived deposit copy can be reactivated for this lender barcode.
    Reactivation,
    /// The archived copy was lost and billed; import cannot proceed without staff action.
    WasLost,
}

/// User-chosen resolution for a duplicate lender barcode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DepositDuplicateAction {
    /// Update the existing copy with MARC fields and attach to the current deposit batch.
    Overwrite,
    /// Keep the lender barcode on the copy record but assign a new local desk barcode.
    Relabel,
    /// Skip this copy and continue the batch.
    Ignore,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositDuplicateConflict {
    pub record_key: String,
    pub lender_barcode: String,
    pub kind: DepositDuplicateConflictKind,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub existing_item_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing_barcode: Option<String>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositDuplicateScanReport {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<DepositDuplicateConflict>,
}

/// Query params for `POST /biblios/import-marc-batch/scan-deposit-duplicates`.
#[serde_as]
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScanDepositDuplicatesQuery {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    #[serde(default, rename = "deposit_batch_id", alias = "depositBatchId")]
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub deposit_batch_id: i64,
    pub record_id: Option<usize>,
}

/// Runtime policy applied while importing a deposit batch (from query params).
#[derive(Debug, Clone, Copy)]
pub struct DepositDuplicatePolicy {
    pub action: DepositDuplicateAction,
    pub apply_to_remaining: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DepositDuplicatePolicyRuntime {
    action: DepositDuplicateAction,
    apply_to_remaining: bool,
    first_conflict_consumed: bool,
}

impl DepositDuplicatePolicyRuntime {
    pub fn new(policy: DepositDuplicatePolicy) -> Self {
        Self {
            action: policy.action,
            apply_to_remaining: policy.apply_to_remaining,
            first_conflict_consumed: false,
        }
    }

    pub fn resolve(&mut self) -> DepositDuplicateAction {
        if !self.apply_to_remaining && self.first_conflict_consumed {
            return DepositDuplicateAction::Ignore;
        }
        if !self.apply_to_remaining {
            self.first_conflict_consumed = true;
        }
        self.action
    }
}
