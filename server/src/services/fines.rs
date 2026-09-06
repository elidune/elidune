//! Fine / penalty service

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::{
    error::{AppError, AppResult},
    models::{
        dto::fines::{
            AccrualBreakdown, AccrueBatchError, AccrueBatchReport, AccrueLoanResult, AccrueOutcome,
            CirculationFinePolicy,
        },
        fine::{Fine, FineAccrualLoan, FineRule},
        user::UserStatus,
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

    /// Accrue a fine for an overdue loan (calculates amount from rules).
    ///
    /// Idempotent: updates the single open fine for `loan_id` instead of inserting another.
    #[tracing::instrument(skip(self), err)]
    pub async fn accrue(
        &self,
        loan_id: i64,
        user_id: i64,
        media_type: Option<&str>,
        overdue_days: i64,
    ) -> AppResult<Fine> {
        let result = self
            .accrue_computed(loan_id, user_id, media_type, overdue_days)
            .await?;
        match result.outcome {
            AccrueOutcome::SkippedGrace => Err(AppError::BusinessRule(
                "Fine amount is zero — within grace period".to_string(),
            )),
            AccrueOutcome::Created | AccrueOutcome::Updated | AccrueOutcome::Unchanged => result
                .fine
                .ok_or_else(|| AppError::Internal("Accrue succeeded without a fine".to_string())),
            other => Err(AppError::BusinessRule(format!(
                "Cannot accrue fine for loan {loan_id}: {other:?}"
            ))),
        }
    }

    /// Accrue (or refresh) the open fine for one loan from its current due date.
    #[tracing::instrument(skip(self), err)]
    pub async fn accrue_loan(&self, loan_id: i64) -> AppResult<AccrueLoanResult> {
        let loan = self.repository.fines_get_accrual_loan(loan_id).await?;
        self.accrue_loan_row(&loan, Utc::now()).await
    }

    /// Accrue overdue loans for one patron (`Some`) or every eligible loan (`None`).
    #[tracing::instrument(skip(self), err)]
    pub async fn accrue_overdue(&self, user_id: Option<i64>) -> AppResult<AccrueBatchReport> {
        let loans = self.repository.fines_list_accrual_loans(user_id).await?;
        let now = Utc::now();
        let mut report = AccrueBatchReport {
            results: Vec::with_capacity(loans.len()),
            ..AccrueBatchReport::default()
        };

        for loan in loans {
            match self.accrue_loan_row(&loan, now).await {
                Ok(result) => {
                    match result.outcome {
                        AccrueOutcome::Created => report.created += 1,
                        AccrueOutcome::Updated => report.updated += 1,
                        AccrueOutcome::Unchanged => report.unchanged += 1,
                        AccrueOutcome::SkippedGrace
                        | AccrueOutcome::SkippedNotOverdue
                        | AccrueOutcome::SkippedReturned
                        | AccrueOutcome::SkippedDeletedUser => report.skipped += 1,
                    }
                    report.results.push(result);
                }
                Err(e) => {
                    report.errors.push(AccrueBatchError {
                        loan_id: loan.loan_id,
                        error_message: e.to_string(),
                    });
                }
            }
        }

        Ok(report)
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

    async fn accrue_loan_row(
        &self,
        loan: &FineAccrualLoan,
        now: DateTime<Utc>,
    ) -> AppResult<AccrueLoanResult> {
        if loan.user_status == UserStatus::Deleted {
            return Ok(skipped(loan, AccrueOutcome::SkippedDeletedUser));
        }
        if loan.returned_at.is_some() {
            return Ok(skipped(loan, AccrueOutcome::SkippedReturned));
        }
        let Some(expiry_at) = loan.expiry_at else {
            return Ok(skipped(loan, AccrueOutcome::SkippedNotOverdue));
        };
        let overdue_days = (now - expiry_at).num_days();
        if overdue_days <= 0 {
            return Ok(skipped(loan, AccrueOutcome::SkippedNotOverdue));
        }

        self.accrue_computed(
            loan.loan_id,
            loan.user_id,
            loan.media_type.as_deref(),
            overdue_days,
        )
        .await
    }

    async fn accrue_computed(
        &self,
        loan_id: i64,
        user_id: i64,
        media_type: Option<&str>,
        overdue_days: i64,
    ) -> AppResult<AccrueLoanResult> {
        let rules = self.repository.fines_list_rules().await?;
        let rule = select_rule(&rules, media_type)
            .ok_or_else(|| AppError::Internal("No fine rule configured".to_string()))?;
        let breakdown = compute_accrual(rule, overdue_days, media_type);

        if breakdown.amount <= Decimal::ZERO {
            return Ok(AccrueLoanResult {
                outcome: AccrueOutcome::SkippedGrace,
                fine: None,
                breakdown: Some(breakdown),
                loan_id,
                user_id,
            });
        }

        let notes = breakdown.to_notes();
        let (fine, outcome) = self
            .repository
            .fines_upsert_open(loan_id, user_id, breakdown.amount, Some(notes.as_str()))
            .await?;

        Ok(AccrueLoanResult {
            outcome,
            fine: Some(fine),
            breakdown: Some(breakdown),
            loan_id,
            user_id,
        })
    }
}

fn skipped(loan: &FineAccrualLoan, outcome: AccrueOutcome) -> AccrueLoanResult {
    AccrueLoanResult {
        outcome,
        fine: None,
        breakdown: None,
        loan_id: loan.loan_id,
        user_id: loan.user_id,
    }
}

/// Media-type rule first, then the default (`media_type` IS NULL).
pub fn select_rule<'a>(rules: &'a [FineRule], media_type: Option<&str>) -> Option<&'a FineRule> {
    rules
        .iter()
        .find(|r| r.media_type.as_deref() == media_type)
        .or_else(|| rules.iter().find(|r| r.media_type.is_none()))
}

/// Source of truth for grace days, daily rate, and cap (used by accrue + tests).
pub fn compute_accrual(
    rule: &FineRule,
    overdue_days: i64,
    media_type: Option<&str>,
) -> AccrualBreakdown {
    let billable_days = (overdue_days - i64::from(rule.grace_days)).max(0);
    let raw = rule.daily_rate * Decimal::from(billable_days);
    let (amount, capped) = match rule.max_amount {
        Some(max) if raw > max => (max, true),
        _ => (raw, false),
    };
    AccrualBreakdown {
        overdue_days,
        grace_days: rule.grace_days,
        billable_days,
        daily_rate: rule.daily_rate,
        max_amount: rule.max_amount,
        amount,
        capped,
        media_type: media_type.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::fine::FineStatus;
    use async_trait::async_trait;
    use std::sync::Mutex;

    fn dec(units: i64, scale: u32) -> Decimal {
        Decimal::new(units, scale)
    }

    fn rule(
        media_type: Option<&str>,
        daily_rate: Decimal,
        max: Option<Decimal>,
        grace: i32,
    ) -> FineRule {
        FineRule {
            id: 1,
            media_type: media_type.map(str::to_string),
            daily_rate,
            max_amount: max,
            grace_days: grace,
            notes: None,
        }
    }

    #[test]
    fn compute_accrual_within_grace_is_zero() {
        let r = rule(None, dec(50, 2), Some(dec(10, 0)), 3);
        let b = compute_accrual(&r, 3, None);
        assert_eq!(b.billable_days, 0);
        assert_eq!(b.amount, Decimal::ZERO);
        assert!(!b.capped);
    }

    #[test]
    fn compute_accrual_applies_daily_rate_after_grace() {
        let r = rule(None, dec(50, 2), Some(dec(10, 0)), 3);
        let b = compute_accrual(&r, 10, Some("printedText"));
        assert_eq!(b.billable_days, 7);
        assert_eq!(b.amount, dec(350, 2));
        assert!(!b.capped);
        assert_eq!(b.media_type.as_deref(), Some("printedText"));
    }

    #[test]
    fn compute_accrual_caps_at_max() {
        let r = rule(None, dec(100, 2), Some(dec(500, 2)), 0);
        let b = compute_accrual(&r, 20, None);
        assert_eq!(b.amount, dec(500, 2));
        assert!(b.capped);
    }

    #[test]
    fn select_rule_prefers_media_type_then_default() {
        let rules = vec![
            rule(None, dec(20, 2), None, 3),
            rule(Some("videoDvd"), dec(100, 2), Some(dec(8, 0)), 1),
        ];
        let dvd = select_rule(&rules, Some("videoDvd")).expect("dvd");
        assert_eq!(dvd.daily_rate, dec(100, 2));
        let book = select_rule(&rules, Some("printedText")).expect("default");
        assert_eq!(book.daily_rate, dec(20, 2));
    }

    struct FakeState {
        rules: Vec<FineRule>,
        fines: Vec<Fine>,
        loans: Vec<FineAccrualLoan>,
        next_id: i64,
    }

    struct FakeRepo {
        state: Mutex<FakeState>,
    }

    impl FakeRepo {
        fn with_default_rule() -> Self {
            Self {
                state: Mutex::new(FakeState {
                    rules: vec![rule(None, dec(50, 2), Some(dec(10, 0)), 3)],
                    fines: vec![],
                    loans: vec![],
                    next_id: 1,
                }),
            }
        }
    }

    fn sample_fine(id: i64, loan_id: i64, user_id: i64, amount: Decimal) -> Fine {
        Fine {
            id,
            loan_id,
            user_id,
            amount,
            paid_amount: Decimal::ZERO,
            created_at: Utc::now(),
            paid_at: None,
            status: FineStatus::Pending,
            notes: None,
        }
    }

    #[async_trait]
    impl FinesRepository for FakeRepo {
        async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
            let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            Ok(g.fines
                .iter()
                .filter(|f| f.user_id == user_id)
                .cloned()
                .collect())
        }
        async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine> {
            let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            g.fines
                .iter()
                .find(|f| f.id == id)
                .cloned()
                .ok_or_else(|| AppError::NotFound("fine".into()))
        }
        async fn fines_create(
            &self,
            loan_id: i64,
            user_id: i64,
            amount: Decimal,
            notes: Option<&str>,
        ) -> AppResult<Fine> {
            let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let id = g.next_id;
            g.next_id += 1;
            let mut fine = sample_fine(id, loan_id, user_id, amount);
            fine.notes = notes.map(str::to_string);
            g.fines.push(fine.clone());
            Ok(fine)
        }
        async fn fines_pay(&self, _: i64, _: Decimal, _: Option<&str>) -> AppResult<Fine> {
            unimplemented!()
        }
        async fn fines_waive(&self, _: i64, _: Option<&str>) -> AppResult<Fine> {
            unimplemented!()
        }
        async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>> {
            Ok(self
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .rules
                .clone())
        }
        async fn fines_upsert_rule(
            &self,
            _: Option<&str>,
            _: Decimal,
            _: Option<Decimal>,
            _: i32,
        ) -> AppResult<FineRule> {
            unimplemented!()
        }
        async fn fines_total_unpaid(&self, _: i64) -> AppResult<Decimal> {
            Ok(Decimal::ZERO)
        }
        async fn fines_get_unpaid_threshold(&self, _: Option<i64>) -> AppResult<Decimal> {
            Ok(Decimal::ZERO)
        }
        async fn fines_get_global_unpaid_threshold(&self) -> AppResult<Decimal> {
            Ok(Decimal::ZERO)
        }
        async fn fines_set_global_unpaid_threshold(
            &self,
            threshold: Decimal,
        ) -> AppResult<Decimal> {
            Ok(threshold)
        }
        async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>> {
            let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            Ok(g.fines
                .iter()
                .find(|f| f.loan_id == loan_id && f.status.is_open())
                .cloned())
        }
        async fn fines_upsert_open(
            &self,
            loan_id: i64,
            user_id: i64,
            amount: Decimal,
            notes: Option<&str>,
        ) -> AppResult<(Fine, AccrueOutcome)> {
            let mut g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = g
                .fines
                .iter_mut()
                .find(|f| f.loan_id == loan_id && f.status.is_open())
            {
                let unchanged = existing.amount == amount;
                existing.amount = amount;
                existing.notes = notes.map(str::to_string);
                let outcome = if unchanged {
                    AccrueOutcome::Unchanged
                } else {
                    AccrueOutcome::Updated
                };
                return Ok((existing.clone(), outcome));
            }
            let id = g.next_id;
            g.next_id += 1;
            let mut fine = sample_fine(id, loan_id, user_id, amount);
            fine.notes = notes.map(str::to_string);
            g.fines.push(fine.clone());
            Ok((fine, AccrueOutcome::Created))
        }
        async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan> {
            let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            g.loans
                .iter()
                .find(|l| l.loan_id == loan_id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("Loan {loan_id} not found")))
        }
        async fn fines_list_accrual_loans(
            &self,
            user_id: Option<i64>,
        ) -> AppResult<Vec<FineAccrualLoan>> {
            let g = self.state.lock().unwrap_or_else(|e| e.into_inner());
            Ok(g.loans
                .iter()
                .filter(|l| user_id.is_none_or(|id| l.user_id == id))
                .cloned()
                .collect())
        }
    }

    #[tokio::test]
    async fn accrue_is_idempotent_one_open_fine_per_loan() {
        let svc = FinesService::new(Arc::new(FakeRepo::with_default_rule()));
        let first = svc.accrue(10, 20, None, 10).await.expect("first");
        let second = svc.accrue(10, 20, None, 10).await.expect("second");
        assert_eq!(first.id, second.id);
        assert_eq!(second.amount, dec(350, 2));
        let listed = svc.list_for_user(20).await.expect("list");
        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn accrue_updates_amount_when_overdue_days_grow() {
        let svc = FinesService::new(Arc::new(FakeRepo::with_default_rule()));
        let first = svc.accrue(11, 21, None, 10).await.expect("first");
        assert_eq!(first.amount, dec(350, 2));
        let second = svc.accrue(11, 21, None, 13).await.expect("second");
        assert_eq!(first.id, second.id);
        assert_eq!(second.amount, dec(500, 2));
        let listed = svc.list_for_user(21).await.expect("list");
        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn accrue_rejects_grace_period() {
        let svc = FinesService::new(Arc::new(FakeRepo::with_default_rule()));
        let err = svc.accrue(12, 22, None, 2).await.expect_err("grace");
        assert!(matches!(err, AppError::BusinessRule(_)));
        let listed = svc.list_for_user(22).await.expect("list");
        assert!(listed.is_empty());
    }

    #[tokio::test]
    async fn accrue_respects_cap() {
        let svc = FinesService::new(Arc::new(FakeRepo::with_default_rule()));
        let fine = svc.accrue(13, 23, None, 100).await.expect("capped");
        assert_eq!(fine.amount, dec(10, 0));
    }

    #[tokio::test]
    async fn accrue_loan_skips_deleted_patron() {
        let repo = FakeRepo::with_default_rule();
        {
            let mut g = repo.state.lock().unwrap_or_else(|e| e.into_inner());
            g.loans.push(FineAccrualLoan {
                loan_id: 40,
                user_id: 50,
                expiry_at: Some(Utc::now() - chrono::Duration::days(10)),
                returned_at: None,
                media_type: None,
                user_status: UserStatus::Deleted,
            });
        }
        let svc = FinesService::new(Arc::new(repo));
        let result = svc.accrue_loan(40).await.expect("skip");
        assert_eq!(result.outcome, AccrueOutcome::SkippedDeletedUser);
        assert!(result.fine.is_none());
    }

    #[tokio::test]
    async fn accrue_overdue_batch_is_idempotent() {
        let repo = FakeRepo::with_default_rule();
        {
            let mut g = repo.state.lock().unwrap_or_else(|e| e.into_inner());
            g.loans.push(FineAccrualLoan {
                loan_id: 60,
                user_id: 70,
                expiry_at: Some(Utc::now() - chrono::Duration::days(10)),
                returned_at: None,
                media_type: None,
                user_status: UserStatus::Active,
            });
        }
        let svc = FinesService::new(Arc::new(repo));
        let first = svc.accrue_overdue(Some(70)).await.expect("batch1");
        let second = svc.accrue_overdue(Some(70)).await.expect("batch2");
        assert_eq!(first.created, 1);
        assert_eq!(second.created, 0);
        assert_eq!(second.unchanged, 1);
        assert_eq!(svc.list_for_user(70).await.expect("list").len(), 1);
    }

    fn d(s: &str) -> Decimal {
        s.parse().expect("decimal")
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
