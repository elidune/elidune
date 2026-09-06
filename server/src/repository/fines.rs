//! Fine domain methods on Repository

use async_trait::async_trait;
use rust_decimal::Decimal;
use snowflaked::Generator;
use sqlx::Row;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        dto::fines::AccrueOutcome,
        fine::{Fine, FineAccrualLoan, FineRule},
        user::UserStatus,
    },
};

#[async_trait]
pub trait FinesRepository: Send + Sync {
    async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>>;
    async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine>;
    async fn fines_create(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<Fine>;
    async fn fines_pay(&self, id: i64, payment: Decimal, notes: Option<&str>) -> AppResult<Fine>;
    async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine>;
    async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>>;
    async fn fines_upsert_rule(
        &self,
        media_type: Option<&str>,
        daily_rate: Decimal,
        max_amount: Option<Decimal>,
        grace_days: i32,
    ) -> AppResult<FineRule>;
    async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Decimal>;
    async fn fines_get_unpaid_threshold(&self, public_type_id: Option<i64>) -> AppResult<Decimal>;
    async fn fines_get_global_unpaid_threshold(&self) -> AppResult<Decimal>;
    async fn fines_set_global_unpaid_threshold(&self, threshold: Decimal) -> AppResult<Decimal>;
    async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>>;
    async fn fines_upsert_open(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<(Fine, AccrueOutcome)>;
    async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan>;
    async fn fines_list_accrual_loans(
        &self,
        user_id: Option<i64>,
    ) -> AppResult<Vec<FineAccrualLoan>>;
}

#[async_trait::async_trait]
impl FinesRepository for Repository {
    async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
        Repository::fines_list_for_user(self, user_id).await
    }
    async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine> {
        Repository::fines_get_by_id(self, id).await
    }
    async fn fines_create(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<Fine> {
        Repository::fines_create(self, loan_id, user_id, amount, notes).await
    }
    async fn fines_pay(&self, id: i64, payment: Decimal, notes: Option<&str>) -> AppResult<Fine> {
        Repository::fines_pay(self, id, payment, notes).await
    }
    async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        Repository::fines_waive(self, id, notes).await
    }
    async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>> {
        Repository::fines_list_rules(self).await
    }
    async fn fines_upsert_rule(
        &self,
        media_type: Option<&str>,
        daily_rate: Decimal,
        max_amount: Option<Decimal>,
        grace_days: i32,
    ) -> AppResult<FineRule> {
        Repository::fines_upsert_rule(self, media_type, daily_rate, max_amount, grace_days).await
    }
    async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Decimal> {
        Repository::fines_total_unpaid(self, user_id).await
    }
    async fn fines_get_unpaid_threshold(&self, public_type_id: Option<i64>) -> AppResult<Decimal> {
        Repository::fines_get_unpaid_threshold(self, public_type_id).await
    }
    async fn fines_get_global_unpaid_threshold(&self) -> AppResult<Decimal> {
        Repository::fines_get_global_unpaid_threshold(self).await
    }
    async fn fines_set_global_unpaid_threshold(&self, threshold: Decimal) -> AppResult<Decimal> {
        Repository::fines_set_global_unpaid_threshold(self, threshold).await
    }
    async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>> {
        Repository::fines_get_open_for_loan(self, loan_id).await
    }
    async fn fines_upsert_open(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<(Fine, AccrueOutcome)> {
        Repository::fines_upsert_open(self, loan_id, user_id, amount, notes).await
    }
    async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan> {
        Repository::fines_get_accrual_loan(self, loan_id).await
    }
    async fn fines_list_accrual_loans(
        &self,
        user_id: Option<i64>,
    ) -> AppResult<Vec<FineAccrualLoan>> {
        Repository::fines_list_accrual_loans(self, user_id).await
    }
}

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(2)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate::<i64>()
}

impl Repository {
    /// List fines for a user
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
        let rows = sqlx::query_as::<_, Fine>(
            "SELECT * FROM fines WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get a fine by ID
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine> {
        sqlx::query_as::<_, Fine>("SELECT * FROM fines WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Fine {id} not found")))
    }

    /// Create a fine for a loan
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_create(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<Fine> {
        let id = next_id();
        let row = sqlx::query_as::<_, Fine>(
            r#"
            INSERT INTO fines (id, loan_id, user_id, amount, notes)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(loan_id)
        .bind(user_id)
        .bind(amount)
        .bind(notes)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Apply a payment to a fine
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_pay(
        &self,
        id: i64,
        payment: Decimal,
        notes: Option<&str>,
    ) -> AppResult<Fine> {
        sqlx::query_as::<_, Fine>(
            r#"
            UPDATE fines SET
                paid_amount = paid_amount + $2,
                notes       = COALESCE($3, notes),
                paid_at     = CASE WHEN paid_amount + $2 >= amount THEN NOW() ELSE NULL END,
                status      = CASE
                    WHEN paid_amount + $2 >= amount THEN 'paid'
                    ELSE 'partial'
                END
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(payment)
        .bind(notes)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Fine {id} not found")))
    }

    /// Waive a fine (write off)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        sqlx::query_as::<_, Fine>(
            "UPDATE fines SET status = 'waived', paid_at = NOW(), notes = COALESCE($2, notes)
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(notes)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Fine {id} not found")))
    }

    /// Get fine rules (per media type + default)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>> {
        let rows = sqlx::query_as::<_, FineRule>(
            "SELECT * FROM fine_rules ORDER BY media_type NULLS FIRST",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a fine rule for a media type (or default)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_upsert_rule(
        &self,
        media_type: Option<&str>,
        daily_rate: Decimal,
        max_amount: Option<Decimal>,
        grace_days: i32,
    ) -> AppResult<FineRule> {
        let updated = if media_type.is_none() {
            sqlx::query_as::<_, FineRule>(
                r#"
                UPDATE fine_rules
                SET daily_rate = $1, max_amount = $2, grace_days = $3
                WHERE media_type IS NULL
                RETURNING *
                "#,
            )
            .bind(daily_rate)
            .bind(max_amount)
            .bind(grace_days)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, FineRule>(
                r#"
                UPDATE fine_rules
                SET daily_rate = $2, max_amount = $3, grace_days = $4
                WHERE media_type = $1
                RETURNING *
                "#,
            )
            .bind(media_type)
            .bind(daily_rate)
            .bind(max_amount)
            .bind(grace_days)
            .fetch_optional(&self.pool)
            .await?
        };

        if let Some(rule) = updated {
            return Ok(rule);
        }

        sqlx::query_as::<_, FineRule>(
            r#"
            INSERT INTO fine_rules (media_type, daily_rate, max_amount, grace_days)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(media_type)
        .bind(daily_rate)
        .bind(max_amount)
        .bind(grace_days)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Sum of unpaid fines for a user.
    ///
    /// Only `pending` / `partial` rows with a positive remainder count. Paid, waived,
    /// and zero-amount fines are excluded (grace-period cases never become unpaid).
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Decimal> {
        let total: Option<Decimal> = sqlx::query_scalar(
            "SELECT SUM(amount - paid_amount) FROM fines
             WHERE user_id = $1
               AND status IN ('pending','partial')
               AND amount > paid_amount",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(total.unwrap_or(Decimal::ZERO))
    }

    /// Effective unpaid-fine threshold: public-type override when set, else global default.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_unpaid_threshold(
        &self,
        public_type_id: Option<i64>,
    ) -> AppResult<Decimal> {
        let total: Option<Decimal> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(
                (
                    SELECT unpaid_fine_threshold
                    FROM public_types
                    WHERE id = $1 AND unpaid_fine_threshold IS NOT NULL
                ),
                (SELECT unpaid_fine_threshold FROM circulation_settings WHERE id = 1),
                0
            )
            "#,
        )
        .bind(public_type_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(total.unwrap_or(Decimal::ZERO))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_global_unpaid_threshold(&self) -> AppResult<Decimal> {
        let total: Option<Decimal> = sqlx::query_scalar(
            "SELECT unpaid_fine_threshold FROM circulation_settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(total.unwrap_or(Decimal::ZERO))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn fines_set_global_unpaid_threshold(
        &self,
        threshold: Decimal,
    ) -> AppResult<Decimal> {
        let total: Decimal = sqlx::query_scalar(
            r#"
            INSERT INTO circulation_settings (id, unpaid_fine_threshold)
            VALUES (1, $1)
            ON CONFLICT (id) DO UPDATE SET unpaid_fine_threshold = EXCLUDED.unpaid_fine_threshold
            RETURNING unpaid_fine_threshold
            "#,
        )
        .bind(threshold)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    /// Open (pending/partial) fine for a loan, if any.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>> {
        let row = sqlx::query_as::<_, Fine>(
            "SELECT * FROM fines WHERE loan_id = $1 AND status IN ('pending','partial') LIMIT 1",
        )
        .bind(loan_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert or refresh the single open fine for a loan.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_upsert_open(
        &self,
        loan_id: i64,
        user_id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<(Fine, AccrueOutcome)> {
        if let Some(existing) = self.fines_get_open_for_loan(loan_id).await? {
            if existing.amount == amount && existing.notes.as_deref() == notes {
                return Ok((existing, AccrueOutcome::Unchanged));
            }
            let updated = self
                .fines_update_open_amount(existing.id, amount, notes)
                .await?;
            return Ok((updated, AccrueOutcome::Updated));
        }

        match self.fines_create(loan_id, user_id, amount, notes).await {
            Ok(created) => Ok((created, AccrueOutcome::Created)),
            Err(AppError::Database(e)) if is_open_fine_unique_violation(&e) => {
                let existing = self
                    .fines_get_open_for_loan(loan_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Conflict(format!(
                            "Open fine for loan {loan_id} raced and then disappeared"
                        ))
                    })?;
                if existing.amount == amount && existing.notes.as_deref() == notes {
                    return Ok((existing, AccrueOutcome::Unchanged));
                }
                let updated = self
                    .fines_update_open_amount(existing.id, amount, notes)
                    .await?;
                Ok((updated, AccrueOutcome::Updated))
            }
            Err(e) => Err(e),
        }
    }

    async fn fines_update_open_amount(
        &self,
        id: i64,
        amount: Decimal,
        notes: Option<&str>,
    ) -> AppResult<Fine> {
        sqlx::query_as::<_, Fine>(
            r#"
            UPDATE fines SET
                amount = $2,
                notes  = COALESCE($3, notes),
                status = CASE
                    WHEN paid_amount >= $2 THEN 'paid'
                    WHEN paid_amount > 0 THEN 'partial'
                    ELSE 'pending'
                END,
                paid_at = CASE WHEN paid_amount >= $2 THEN NOW() ELSE NULL END
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(amount)
        .bind(notes)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Fine {id} not found")))
    }

    /// Loan + media type + patron status for a single accrue.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan> {
        let row = sqlx::query(
            r#"
            SELECT
                l.id AS loan_id,
                l.user_id,
                l.expiry_at,
                l.returned_at,
                b.media_type,
                u.status AS user_status
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            JOIN users u ON l.user_id = u.id
            WHERE l.id = $1
            "#,
        )
        .bind(loan_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Loan {loan_id} not found")))?;

        Ok(accrual_loan_from_row(&row))
    }

    /// Unreturned overdue loans eligible for accrual (skips deleted patrons).
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_accrual_loans(
        &self,
        user_id: Option<i64>,
    ) -> AppResult<Vec<FineAccrualLoan>> {
        let rows = sqlx::query(
            r#"
            SELECT
                l.id AS loan_id,
                l.user_id,
                l.expiry_at,
                l.returned_at,
                b.media_type,
                u.status AS user_status
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            JOIN users u ON l.user_id = u.id
            WHERE l.returned_at IS NULL
              AND l.expiry_at < NOW()
              AND (u.status IS NULL OR u.status <> 'deleted')
              AND ($1::bigint IS NULL OR l.user_id = $1)
            ORDER BY l.expiry_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(accrual_loan_from_row).collect())
    }
}

fn accrual_loan_from_row(row: &sqlx::postgres::PgRow) -> FineAccrualLoan {
    let status: Option<String> = row.get("user_status");
    FineAccrualLoan {
        loan_id: row.get("loan_id"),
        user_id: row.get("user_id"),
        expiry_at: row.get("expiry_at"),
        returned_at: row.get("returned_at"),
        media_type: row.get("media_type"),
        user_status: status
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(UserStatus::Active),
    }
}

fn is_open_fine_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("23505")
                && db.constraint() == Some("idx_fines_one_open_per_loan")
        }
        _ => false,
    }
}
