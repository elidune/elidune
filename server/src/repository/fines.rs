//! Fine domain methods on Repository

use async_trait::async_trait;
use snowflaked::Generator;
use sqlx::Row;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        dto::{
            circulation::{LostFineAction, LostFineDeskStatus, LostItemOnReturn},
            fines::AccrueOutcome,
        },
        fine::{Cents, Fine, FineAccrualLoan, FineChargeType, FineRule, FineStatus, GlobalUnpaidThresholdMode, StoredGlobalUnpaidThreshold},
        user::UserStatus,
    },
};

#[async_trait]
pub trait FinesRepository: Send + Sync {
    async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>>;
    async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine>;
    async fn fines_create(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<Fine>;
    async fn fines_pay(&self, id: i64, payment: Cents, notes: Option<&str>) -> AppResult<Fine>;
    async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine>;
    async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>>;
    async fn fines_upsert_rule(&self, media_type: Option<&str>, daily_rate: Cents, max_amount: Option<Cents>, grace_days: i32) -> AppResult<FineRule>;
    async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Cents>;
    async fn fines_get_unpaid_threshold(&self, public_type_id: Option<i64>) -> AppResult<Option<Cents>>;
    fn accrue_fines_enabled(&self) -> bool;
    async fn fines_get_global_unpaid_threshold(&self) -> AppResult<StoredGlobalUnpaidThreshold>;
    async fn fines_set_global_unpaid_threshold(&self, mode: GlobalUnpaidThresholdMode, amount: Option<Cents>) -> AppResult<StoredGlobalUnpaidThreshold>;
    async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>>;
    async fn fines_upsert_open(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<(Fine, AccrueOutcome)>;
    async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan>;
    async fn fines_list_accrual_loans(&self, user_id: Option<i64>) -> AppResult<Vec<FineAccrualLoan>>;
}

#[async_trait::async_trait]
impl FinesRepository for Repository {
    async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
        Repository::fines_list_for_user(self, user_id).await
    }
    async fn fines_get_by_id(&self, id: i64) -> AppResult<Fine> {
        Repository::fines_get_by_id(self, id).await
    }
    async fn fines_create(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<Fine> {
        Repository::fines_create(self, loan_id, user_id, amount, notes).await
    }
    async fn fines_pay(&self, id: i64, payment: Cents, notes: Option<&str>) -> AppResult<Fine> {
        Repository::fines_pay(self, id, payment, notes).await
    }
    async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        Repository::fines_waive(self, id, notes).await
    }
    async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>> {
        Repository::fines_list_rules(self).await
    }
    async fn fines_upsert_rule(&self, media_type: Option<&str>, daily_rate: Cents, max_amount: Option<Cents>, grace_days: i32) -> AppResult<FineRule> {
        Repository::fines_upsert_rule(self, media_type, daily_rate, max_amount, grace_days).await
    }
    async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Cents> {
        Repository::fines_total_unpaid(self, user_id).await
    }
    async fn fines_get_unpaid_threshold(&self, public_type_id: Option<i64>) -> AppResult<Option<Cents>> {
        Repository::fines_get_unpaid_threshold(self, public_type_id).await
    }
    fn accrue_fines_enabled(&self) -> bool {
        Repository::accrue_fines_enabled(self)
    }
    async fn fines_get_global_unpaid_threshold(&self) -> AppResult<StoredGlobalUnpaidThreshold> {
        Repository::fines_get_global_unpaid_threshold(self).await
    }
    async fn fines_set_global_unpaid_threshold(&self, mode: GlobalUnpaidThresholdMode, amount: Option<Cents>) -> AppResult<StoredGlobalUnpaidThreshold> {
        Repository::fines_set_global_unpaid_threshold(self, mode, amount).await
    }
    async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>> {
        Repository::fines_get_open_for_loan(self, loan_id).await
    }
    async fn fines_upsert_open(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<(Fine, AccrueOutcome)> {
        Repository::fines_upsert_open(self, loan_id, user_id, amount, notes).await
    }
    async fn fines_get_accrual_loan(&self, loan_id: i64) -> AppResult<FineAccrualLoan> {
        Repository::fines_get_accrual_loan(self, loan_id).await
    }
    async fn fines_list_accrual_loans(&self, user_id: Option<i64>) -> AppResult<Vec<FineAccrualLoan>> {
        Repository::fines_list_accrual_loans(self, user_id).await
    }
}

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(2)));

fn next_id() -> i64 {
    SNOWFLAKE.lock().unwrap_or_else(|e| e.into_inner()).generate::<i64>()
}

impl Repository {
    /// List fines for a user
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_for_user(&self, user_id: i64) -> AppResult<Vec<Fine>> {
        let rows = sqlx::query_as::<_, Fine>("SELECT * FROM fines WHERE user_id = $1 ORDER BY created_at DESC")
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
    pub async fn fines_create(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<Fine> {
        let id = next_id();
        let row = sqlx::query_as::<_, Fine>(
            r#"
            INSERT INTO fines (id, loan_id, user_id, amount, notes, charge_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(loan_id)
        .bind(user_id)
        .bind(amount)
        .bind(notes)
        .bind(FineChargeType::Overdue)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Create a charge of an explicit type (replacement / damage / overdue).
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_create_charge(&self, loan_id: i64, user_id: i64, amount: Cents, charge_type: FineChargeType, notes: Option<&str>) -> AppResult<Fine> {
        let id = next_id();
        let row = sqlx::query_as::<_, Fine>(
            r#"
            INSERT INTO fines (id, loan_id, user_id, amount, notes, charge_type)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(loan_id)
        .bind(user_id)
        .bind(amount)
        .bind(notes)
        .bind(charge_type)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Create a charge inside an open transaction (circulation exceptions).
    #[allow(clippy::too_many_arguments)]
    pub async fn fines_create_charge_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        loan_id: i64,
        user_id: i64,
        item_id: i64,
        amount: Cents,
        charge_type: FineChargeType,
        notes: Option<&str>,
    ) -> AppResult<Fine> {
        let id = next_id();
        let row = sqlx::query_as::<_, Fine>(
            r#"
            INSERT INTO fines (id, loan_id, user_id, item_id, amount, notes, charge_type)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(loan_id)
        .bind(user_id)
        .bind(item_id)
        .bind(amount)
        .bind(notes)
        .bind(charge_type)
        .fetch_one(&mut **tx)
        .await?;
        Ok(row)
    }

    /// Apply a payment to a fine
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_pay(&self, id: i64, payment: Cents, notes: Option<&str>) -> AppResult<Fine> {
        let mut conn = self.pool.acquire().await?;
        apply_fine_payment(&mut conn, id, payment, notes).await
    }

    /// Apply a payment inside an open transaction.
    pub async fn fines_pay_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, id: i64, payment: Cents, notes: Option<&str>) -> AppResult<Fine> {
        apply_fine_payment(tx, id, payment, notes).await
    }

    /// Waive a fine (write off)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_waive(&self, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        waive_fine(&self.pool, id, notes).await
    }

    /// Waive a fine inside an open transaction.
    pub async fn fines_waive_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, id: i64, notes: Option<&str>) -> AppResult<Fine> {
        waive_fine(&mut **tx, id, notes).await
    }

    /// Get fine rules (per media type + default)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_rules(&self) -> AppResult<Vec<FineRule>> {
        let rows = sqlx::query_as::<_, FineRule>("SELECT * FROM fine_rules ORDER BY media_type NULLS FIRST").fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Upsert a fine rule for a media type (or default)
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_upsert_rule(&self, media_type: Option<&str>, daily_rate: Cents, max_amount: Option<Cents>, grace_days: i32) -> AppResult<FineRule> {
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
    pub async fn fines_total_unpaid(&self, user_id: i64) -> AppResult<Cents> {
        let mut conn = self.pool.acquire().await?;
        self.fines_total_unpaid_conn(&mut conn, user_id).await
    }

    pub(crate) async fn fines_total_unpaid_conn(&self, conn: &mut sqlx::PgConnection, user_id: i64) -> AppResult<Cents> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount - paid_amount), 0)::bigint FROM fines
             WHERE user_id = $1
               AND status IN ('pending','partial')
               AND amount > paid_amount",
        )
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await?;
        Ok(Cents(total))
    }

    /// Effective unpaid-fine amount. `None` means no amount-based block.
    ///
    /// Public type `amount` uses that type's value. `none` does not block.
    /// `inherit` (or no public type) uses the global mode: `amount` uses the
    /// global value, `none` does not block.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_unpaid_threshold(&self, public_type_id: Option<i64>) -> AppResult<Option<Cents>> {
        let mut conn = self.pool.acquire().await?;
        self.fines_get_unpaid_threshold_conn(&mut conn, public_type_id).await
    }

    pub(crate) async fn fines_get_unpaid_threshold_conn(&self, conn: &mut sqlx::PgConnection, public_type_id: Option<i64>) -> AppResult<Option<Cents>> {
        let threshold: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT CASE
                WHEN pt.unpaid_threshold_mode = 'amount' THEN pt.unpaid_fine_threshold
                WHEN pt.unpaid_threshold_mode = 'none' THEN NULL
                WHEN cs.unpaid_threshold_mode = 'amount' THEN cs.unpaid_fine_threshold
                ELSE NULL
            END
            FROM (SELECT 1) AS dual
            LEFT JOIN public_types pt ON pt.id = $1
            LEFT JOIN circulation_settings cs ON cs.id = 1
            "#,
        )
        .bind(public_type_id)
        .fetch_one(&mut *conn)
        .await?;
        Ok(threshold.map(Cents))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_global_unpaid_threshold(&self) -> AppResult<StoredGlobalUnpaidThreshold> {
        let stored = sqlx::query_as::<_, StoredGlobalUnpaidThreshold>(
            r#"
            SELECT unpaid_threshold_mode, unpaid_fine_threshold
            FROM circulation_settings
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(stored.unwrap_or(StoredGlobalUnpaidThreshold {
            mode: GlobalUnpaidThresholdMode::None,
            amount: Cents::ZERO,
        }))
    }

    /// `amount` is written only when `Some`. Mode `none` keeps the stored numeric value.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_set_global_unpaid_threshold(&self, mode: GlobalUnpaidThresholdMode, amount: Option<Cents>) -> AppResult<StoredGlobalUnpaidThreshold> {
        let stored = sqlx::query_as::<_, StoredGlobalUnpaidThreshold>(
            r#"
            INSERT INTO circulation_settings (id, unpaid_threshold_mode, unpaid_fine_threshold)
            VALUES (1, $1, COALESCE($2, 0))
            ON CONFLICT (id) DO UPDATE SET
                unpaid_threshold_mode = EXCLUDED.unpaid_threshold_mode,
                unpaid_fine_threshold = COALESCE($2, circulation_settings.unpaid_fine_threshold)
            RETURNING unpaid_threshold_mode, unpaid_fine_threshold
            "#,
        )
        .bind(mode)
        .bind(amount)
        .fetch_one(&self.pool)
        .await?;
        Ok(stored)
    }

    /// Open (pending/partial) fine for a loan, if any.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_get_open_for_loan(&self, loan_id: i64) -> AppResult<Option<Fine>> {
        let row = sqlx::query_as::<_, Fine>("SELECT * FROM fines WHERE loan_id = $1 AND status IN ('pending','partial') LIMIT 1")
            .bind(loan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Insert or refresh the single open fine for a loan.
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_upsert_open(&self, loan_id: i64, user_id: i64, amount: Cents, notes: Option<&str>) -> AppResult<(Fine, AccrueOutcome)> {
        if let Some(existing) = self.fines_get_open_for_loan(loan_id).await? {
            if existing.amount == amount && existing.notes.as_deref() == notes {
                return Ok((existing, AccrueOutcome::Unchanged));
            }
            let updated = self.fines_update_open_amount(existing.id, amount, notes).await?;
            return Ok((updated, AccrueOutcome::Updated));
        }

        match self.fines_create(loan_id, user_id, amount, notes).await {
            Ok(created) => Ok((created, AccrueOutcome::Created)),
            Err(AppError::Database(e)) if is_open_fine_unique_violation(&e) => {
                let existing = self
                    .fines_get_open_for_loan(loan_id)
                    .await?
                    .ok_or_else(|| AppError::Conflict(format!("Open fine for loan {loan_id} raced and then disappeared")))?;
                if existing.amount == amount && existing.notes.as_deref() == notes {
                    return Ok((existing, AccrueOutcome::Unchanged));
                }
                let updated = self.fines_update_open_amount(existing.id, amount, notes).await?;
                Ok((updated, AccrueOutcome::Updated))
            }
            Err(e) => Err(e),
        }
    }

    async fn fines_update_open_amount(&self, id: i64, amount: Cents, notes: Option<&str>) -> AppResult<Fine> {
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
        let site = loan_calendar_site_sql();
        let row = sqlx::query(&format!(
            r#"
            SELECT
                l.id AS loan_id,
                l.user_id,
                l.expiry_at,
                l.returned_at,
                b.media_type,
                u.status AS user_status,
                {site} AS site_id
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            JOIN users u ON l.user_id = u.id
            WHERE l.id = $1
            "#,
        ))
        .bind(loan_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Loan {loan_id} not found")))?;

        Ok(accrual_loan_from_row(&row))
    }

    /// Unreturned overdue loans eligible for accrual (skips deleted patrons).
    #[tracing::instrument(skip(self), err)]
    pub async fn fines_list_accrual_loans(&self, user_id: Option<i64>) -> AppResult<Vec<FineAccrualLoan>> {
        let site = loan_calendar_site_sql();
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                l.id AS loan_id,
                l.user_id,
                l.expiry_at,
                l.returned_at,
                b.media_type,
                u.status AS user_status,
                {site} AS site_id
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
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(accrual_loan_from_row).collect())
    }

    /// Replacement charge to show when a lost copy is checked in.
    pub async fn fines_lost_return_notice(&self, loan_id: Option<i64>, item_id: i64) -> AppResult<LostItemOnReturn> {
        let row = replacement_row(&self.pool, loan_id, item_id, false).await?;
        lost_notice(row.map(|row| (row.id, row.amount, row.paid_amount, row.status)))
    }

    /// Same lookup inside the return transaction.
    pub async fn fines_lost_return_notice_tx(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, loan_id: i64, item_id: i64) -> AppResult<LostItemOnReturn> {
        let row = replacement_row(&mut **tx, Some(loan_id), item_id, true).await?;
        lost_notice(row.map(|row| (row.id, row.amount, row.paid_amount, row.status)))
    }

    /// Open replacement charge linked to the copy. Does not update the row.
    pub async fn fines_linked_replacement_id_tx(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, item_id: i64) -> AppResult<Option<i64>> {
        Ok(replacement_row(&mut **tx, None, item_id, false).await?.map(|row| row.id))
    }

    /// Cancel an unpaid replacement charge, refund a collected one, or both.
    /// Does nothing when no actionable replacement charge is linked to the copy.
    /// Not used for disposition `damaged` — that path leaves the charge as it is.
    pub async fn fines_settle_replacement_tx(&self, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, item_id: i64, notes: Option<&str>) -> AppResult<Option<LostFineSettlement>> {
        let Some(row) = replacement_row(&mut **tx, None, item_id, true).await? else {
            return Ok(None);
        };
        let mut settled = settle_replacement_charge(row.amount, row.paid_amount);
        settled.fine_id = row.id;
        settled.user_id = row.user_id;
        let status = settled.status().as_str();
        sqlx::query(
            r#"
            UPDATE fines SET
                status = $2,
                refunded_amount = $3,
                notes = CASE
                    WHEN $4::text IS NULL OR btrim($4) = '' THEN notes
                    WHEN notes IS NULL OR btrim(notes) = '' THEN $4
                    ELSE notes || E'\n' || $4
                END
            WHERE id = $1
              AND status IN ('pending', 'partial', 'paid')
            "#,
        )
        .bind(row.id)
        .bind(status)
        .bind(settled.refunded_amount)
        .bind(notes)
        .execute(&mut **tx)
        .await?;
        Ok(Some(settled))
    }

    /// Staff-defined replacement bill: a fine rule whose media type is `replacement`.
    /// Prefer a positive cap, otherwise a positive daily rate. No row is seeded.
    pub async fn fines_default_replacement_amount(&self) -> AppResult<Option<Cents>> {
        let row: Option<(i64, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT daily_rate, max_amount
            FROM fine_rules
            WHERE lower(media_type) = 'replacement'
            ORDER BY id
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|(daily, max)| {
            if max.is_some_and(|value| value > 0) {
                max.map(Cents)
            } else if daily > 0 {
                Some(Cents(daily))
            } else {
                None
            }
        }))
    }
}

struct ReplacementRow {
    id: i64,
    user_id: i64,
    amount: Cents,
    paid_amount: Cents,
    status: FineStatus,
}

async fn replacement_row<'e, E>(executor: E, loan_id: Option<i64>, item_id: i64, lock: bool) -> AppResult<Option<ReplacementRow>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let sql = if lock {
        r#"
        SELECT id, user_id, amount, paid_amount, status
        FROM fines
        WHERE charge_type = 'replacement'
          AND status IN ('pending', 'partial', 'paid')
          AND (item_id = $1 OR ($2::bigint IS NOT NULL AND loan_id = $2))
        ORDER BY created_at DESC
        LIMIT 1
        FOR UPDATE
        "#
    } else {
        r#"
        SELECT id, user_id, amount, paid_amount, status
        FROM fines
        WHERE charge_type = 'replacement'
          AND status IN ('pending', 'partial', 'paid')
          AND (item_id = $1 OR ($2::bigint IS NOT NULL AND loan_id = $2))
        ORDER BY created_at DESC
        LIMIT 1
        "#
    };
    let row = sqlx::query(sql).bind(item_id).bind(loan_id).fetch_optional(executor).await?;
    Ok(row.map(|row| ReplacementRow {
        id: row.get("id"),
        user_id: row.get("user_id"),
        amount: row.get("amount"),
        paid_amount: row.get("paid_amount"),
        status: FineStatus::from(row.get::<String, _>("status")),
    }))
}

fn loan_calendar_site_sql() -> String {
    format!("COALESCE(l.site_id, {})", crate::services::calendar::LIBRARY_DEFAULT_SITE_ID_SQL)
}

fn accrual_loan_from_row(row: &sqlx::postgres::PgRow) -> FineAccrualLoan {
    let status: Option<String> = row.get("user_status");
    FineAccrualLoan {
        loan_id: row.get("loan_id"),
        user_id: row.get("user_id"),
        expiry_at: row.get("expiry_at"),
        returned_at: row.get("returned_at"),
        media_type: row.get("media_type"),
        user_status: status.as_deref().and_then(|s| s.parse().ok()).unwrap_or(UserStatus::Active),
        site_id: row.get("site_id"),
    }
}

/// How a replacement charge is closed when the lost copy is accepted back.
#[derive(Debug, Clone)]
pub struct LostFineSettlement {
    pub fine_id: i64,
    pub user_id: i64,
    pub action: LostFineAction,
    pub refunded_amount: Cents,
    pub cancelled_amount: Cents,
}

/// Refund only what was collected. Cancel the uncollected remainder.
/// A zero paid amount never produces a refund.
#[must_use]
pub fn settle_replacement_charge(amount: Cents, paid_amount: Cents) -> LostFineSettlement {
    let refunded = if paid_amount.0 > 0 { paid_amount } else { Cents::ZERO };
    let cancelled = amount.saturating_sub(paid_amount);
    let action = match (refunded.0 > 0, cancelled.0 > 0) {
        (false, _) => LostFineAction::Cancelled,
        (true, false) => LostFineAction::Refunded,
        (true, true) => LostFineAction::Partial,
    };
    LostFineSettlement {
        fine_id: 0,
        user_id: 0,
        action,
        refunded_amount: refunded,
        cancelled_amount: cancelled,
    }
}

impl LostFineSettlement {
    fn status(&self) -> FineStatus {
        if self.refunded_amount.0 > 0 {
            FineStatus::Refunded
        } else {
            FineStatus::Cancelled
        }
    }
}

fn lost_notice(fine: Option<(i64, Cents, Cents, FineStatus)>) -> AppResult<LostItemOnReturn> {
    let Some((id, amount, paid, status)) = fine else {
        return Ok(LostItemOnReturn {
            was_lost: true,
            fine_id: None,
            fine_status: None,
            fine_amount: None,
            paid_amount: None,
        });
    };
    let desk = match status {
        FineStatus::Pending => Some(LostFineDeskStatus::Unpaid),
        FineStatus::Partial => Some(LostFineDeskStatus::Partial),
        FineStatus::Paid => Some(LostFineDeskStatus::Paid),
        FineStatus::Waived | FineStatus::Cancelled | FineStatus::Refunded => None,
    };
    let Some(desk) = desk else {
        return Ok(LostItemOnReturn {
            was_lost: true,
            fine_id: None,
            fine_status: None,
            fine_amount: None,
            paid_amount: None,
        });
    };
    Ok(LostItemOnReturn {
        was_lost: true,
        fine_id: Some(id),
        fine_status: Some(desk),
        fine_amount: Some(amount.get()),
        paid_amount: Some(paid.get()),
    })
}

async fn apply_fine_payment(conn: &mut sqlx::PgConnection, id: i64, payment: Cents, notes: Option<&str>) -> AppResult<Fine> {
    // A check-constraint failure aborts an open transaction. The savepoint lets
    // the follow-up read of the remaining balance still run.
    let in_tx: bool = sqlx::query_scalar("SELECT pg_current_xact_id_if_assigned() IS NOT NULL").fetch_one(&mut *conn).await?;
    if in_tx {
        sqlx::query("SAVEPOINT fine_payment").execute(&mut *conn).await?;
    }

    let updated = sqlx::query_as::<_, Fine>(
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
          AND paid_amount + $2 <= amount
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(payment)
    .bind(notes)
    .fetch_optional(&mut *conn)
    .await;

    match updated {
        Ok(Some(fine)) => {
            release_fine_payment_savepoint(conn, in_tx).await?;
            Ok(fine)
        }
        Ok(None) => {
            release_fine_payment_savepoint(conn, in_tx).await?;
            unpaid_or_missing(conn, id).await
        }
        Err(err) if is_fines_paid_amount_cap(&err) => {
            if in_tx {
                sqlx::query("ROLLBACK TO SAVEPOINT fine_payment").execute(&mut *conn).await?;
            }
            release_fine_payment_savepoint(conn, in_tx).await?;
            unpaid_or_missing(conn, id).await
        }
        Err(err) => Err(err.into()),
    }
}

/// `NotFound` when the fine is gone, otherwise 422 with `amount - paid_amount`.
async fn unpaid_or_missing(conn: &mut sqlx::PgConnection, id: i64) -> AppResult<Fine> {
    let row = sqlx::query("SELECT amount, paid_amount FROM fines WHERE id = $1").bind(id).fetch_optional(&mut *conn).await?;
    let Some(row) = row else {
        return Err(AppError::NotFound(format!("Fine {id} not found")));
    };
    let amount: Cents = row.get("amount");
    let paid_amount: Cents = row.get("paid_amount");
    Err(AppError::payment_exceeds_balance(amount.saturating_sub(paid_amount).get()))
}

async fn release_fine_payment_savepoint(conn: &mut sqlx::PgConnection, in_tx: bool) -> AppResult<()> {
    if in_tx {
        sqlx::query("RELEASE SAVEPOINT fine_payment").execute(&mut *conn).await?;
    }
    Ok(())
}

fn is_fines_paid_amount_cap(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23514") && db.constraint() == Some("fines_paid_amount_lte_amount"),
        _ => false,
    }
}

async fn waive_fine<'e, E>(executor: E, id: i64, notes: Option<&str>) -> AppResult<Fine>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query_as::<_, Fine>(
        "UPDATE fines SET status = 'waived', paid_at = NOW(), notes = COALESCE($2, notes)
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(notes)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Fine {id} not found")))
}

fn is_open_fine_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505") && matches!(db.constraint(), Some("idx_fines_one_open_per_loan") | Some("idx_fines_one_open_per_loan_charge")),
        _ => false,
    }
}
