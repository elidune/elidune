//! Fine / penalty service

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    error::{AppError, AppResult},
    models::{
        dto::fines::CirculationFinePolicy,
        fine::{Fine, FineRule},
    },
    repository::FinesRepository,
};

/// Result of comparing a patron's unpaid balance to the configured threshold.
///
/// `#18` writes pending fines through [`FinesService::accrue`] (grace-period and
/// zero-amount cases never create an unpaid row). Checkout/renew (#10) only read
/// [`FinesService::total_unpaid`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpaidThresholdCheck {
    pub unpaid: Decimal,
    pub threshold: Decimal,
}

impl UnpaidThresholdCheck {
    /// Block when the unpaid remainder is strictly above the threshold.
    /// A zero unpaid balance never blocks (including grace-period / zero-amount cases).
    #[must_use]
    pub fn blocks_circulation(&self) -> bool {
        self.unpaid > Decimal::ZERO && self.unpaid > self.threshold
    }

    /// Desk-facing refusal: amount due and that staff can force.
    #[must_use]
    pub fn desk_block_message(&self) -> String {
        format!(
            "Unpaid fines of {} exceed the {} threshold — pay or waive the balance, or use force=true to override",
            self.unpaid, self.threshold
        )
    }
}

#[derive(Clone)]
pub struct FinesService {
    repository: Arc<dyn FinesRepository>,
}

impl FinesService {
    pub fn new(repository: Arc<dyn FinesRepository>) -> Self {
        Self { repository }
    }

    /// List all fines for a user
    #[tracing::instrument(skip(self), err)]
    pub async fn list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
        self.repository.fines_list_for_user(user_id).await
    }

    /// Get a specific fine
    #[tracing::instrument(skip(self), err)]
    pub async fn get(&self, id: i64) -> AppResult<Fine> {
        self.repository.fines_get_by_id(id).await
    }

    /// Accrue a fine for an overdue loan (calculates amount from rules)
    #[tracing::instrument(skip(self), err)]
    pub async fn accrue(
        &self,
        loan_id: i64,
        user_id: i64,
        media_type: Option<&str>,
        overdue_days: i64,
    ) -> AppResult<Fine> {
        let rules = self.repository.fines_list_rules().await?;
        // Look for media-type specific rule first, then default
        let rule = rules
            .iter()
            .find(|r| r.media_type.as_deref() == media_type)
            .or_else(|| rules.iter().find(|r| r.media_type.is_none()))
            .ok_or_else(|| AppError::Internal("No fine rule configured".to_string()))?;

        let effective_days = (overdue_days - rule.grace_days as i64).max(0);
        let mut amount = rule.daily_rate * Decimal::from(effective_days);
        if let Some(max) = rule.max_amount {
            amount = amount.min(max);
        }

        if amount <= Decimal::ZERO {
            return Err(AppError::BusinessRule(
                "Fine amount is zero — within grace period".to_string(),
            ));
        }

        self.repository
            .fines_create(loan_id, user_id, amount, None)
            .await
    }

    /// Apply a payment to a fine
    #[tracing::instrument(skip(self), err)]
    pub async fn pay(&self, id: i64, amount: Decimal, notes: Option<&str>) -> AppResult<Fine> {
        if amount <= Decimal::ZERO {
            return Err(AppError::Validation(
                "Payment amount must be positive".to_string(),
            ));
        }
        self.repository.fines_pay(id, amount, notes).await
    }

    /// Waive (write off) a fine
    #[tracing::instrument(skip(self), err)]
    pub async fn waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        self.repository.fines_waive(id, notes).await
    }

    /// Get total unpaid fines for a user
    #[tracing::instrument(skip(self), err)]
    pub async fn total_unpaid(&self, user_id: i64) -> AppResult<Decimal> {
        self.repository.fines_total_unpaid(user_id).await
    }

    /// Global unpaid-fine threshold (public-type overrides are applied in
    /// [`Self::check_unpaid_threshold`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn get_policy(&self) -> AppResult<CirculationFinePolicy> {
        Ok(CirculationFinePolicy {
            unpaid_fine_threshold: self.repository.fines_get_global_unpaid_threshold().await?,
        })
    }

    /// Replace the global unpaid-fine threshold.
    #[tracing::instrument(skip(self), err)]
    pub async fn set_policy(&self, threshold: Decimal) -> AppResult<CirculationFinePolicy> {
        if threshold < Decimal::ZERO {
            return Err(AppError::Validation(
                "Unpaid fine threshold cannot be negative".to_string(),
            ));
        }
        Ok(CirculationFinePolicy {
            unpaid_fine_threshold: self
                .repository
                .fines_set_global_unpaid_threshold(threshold)
                .await?,
        })
    }

    /// Compare this patron's unpaid balance to the effective threshold
    /// (public-type override, else global).
    #[tracing::instrument(skip(self), err)]
    pub async fn check_unpaid_threshold(
        &self,
        user_id: i64,
        public_type_id: Option<i64>,
    ) -> AppResult<UnpaidThresholdCheck> {
        let (unpaid, threshold) = tokio::try_join!(
            self.repository.fines_total_unpaid(user_id),
            self.repository.fines_get_unpaid_threshold(public_type_id),
        )?;
        Ok(UnpaidThresholdCheck { unpaid, threshold })
    }

    /// List fine rules
    #[tracing::instrument(skip(self), err)]
    pub async fn list_rules(&self) -> AppResult<Vec<FineRule>> {
        self.repository.fines_list_rules().await
    }

    /// Upsert a fine rule
    #[tracing::instrument(skip(self), err)]
    pub async fn upsert_rule(
        &self,
        media_type: Option<&str>,
        daily_rate: Decimal,
        max_amount: Option<Decimal>,
        grace_days: i32,
    ) -> AppResult<FineRule> {
        if daily_rate < Decimal::ZERO {
            return Err(AppError::Validation(
                "Daily rate cannot be negative".to_string(),
            ));
        }
        self.repository
            .fines_upsert_rule(media_type, daily_rate, max_amount, grace_days)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).expect("decimal")
    }

    fn check(unpaid: &str, threshold: &str) -> UnpaidThresholdCheck {
        UnpaidThresholdCheck {
            unpaid: d(unpaid),
            threshold: d(threshold),
        }
    }

    #[test]
    fn zero_unpaid_never_blocks() {
        assert!(!check("0", "0").blocks_circulation());
        assert!(!check("0", "10").blocks_circulation());
    }

    #[test]
    fn unpaid_at_threshold_does_not_block() {
        assert!(!check("10", "10").blocks_circulation());
    }

    #[test]
    fn unpaid_above_threshold_blocks() {
        let over = check("10.01", "10");
        assert!(over.blocks_circulation());
        assert!(over.desk_block_message().contains("10.01"));
        assert!(over.desk_block_message().contains("force=true"));
    }

    #[test]
    fn unpaid_under_threshold_does_not_block() {
        assert!(!check("4.99", "5").blocks_circulation());
    }
}
