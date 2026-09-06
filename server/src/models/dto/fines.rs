//! Fine policy DTOs (shared by API and services).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Global unpaid-fine threshold that gates checkout and renew.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CirculationFinePolicy {
    /// Block new loans and renewals when unpaid fines exceed this amount.
    /// Default `0` blocks any positive unpaid balance. Paid and waived fines do not count.
    pub unpaid_fine_threshold: Decimal,
}
