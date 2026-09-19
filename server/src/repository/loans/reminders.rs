//! Loans repository — overdue reminder queries.

use chrono::{DateTime, Utc};
use sqlx::Row;

use super::super::Repository;
use crate::error::AppResult;
use crate::models::circulation::CirculationStatus;

/// Per-tier delays (days after due date) used to select the next reminder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReminderTierDelays {
    pub first_days: u32,
    pub second_days: u32,
    pub formal_notice_days: u32,
    /// Exclusive upper bound on `reminder_count` (highest enabled tier).
    pub max_count: i32,
}

impl Default for ReminderTierDelays {
    fn default() -> Self {
        Self {
            first_days: 7,
            second_days: 14,
            formal_notice_days: 21,
            max_count: 3,
        }
    }
}

impl ReminderTierDelays {
    pub fn from_config(cfg: &crate::config::RemindersConfig) -> Self {
        Self {
            first_days: cfg.first_reminder_delay_days,
            second_days: cfg.second_reminder_delay_days,
            formal_notice_days: cfg.formal_notice_delay_days,
            max_count: cfg.max_sendable_reminder_count(),
        }
    }
}

/// A flat row from overdue loan queries, used by the reminders service and API
#[derive(Debug, Clone)]
pub struct OverdueLoanRow {
    pub loan_id: i64,
    pub user_id: i64,
    pub loan_date: DateTime<Utc>,
    pub expiry_at: Option<DateTime<Utc>>,
    pub last_reminder_sent_at: Option<DateTime<Utc>>,
    pub reminder_count: i32,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub user_email: Option<String>,
    pub user_language: Option<String>,
    pub biblio_id: i64,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub item_barcode: Option<String>,
    pub circulation_status: Option<i16>,
}

impl OverdueLoanRow {
    fn from_row(row: sqlx::postgres::PgRow) -> Self {
        Self {
            loan_id: row.get("loan_id"),
            user_id: row.get("user_id"),
            loan_date: row.get("loan_date"),
            expiry_at: row.get("expiry_at"),
            last_reminder_sent_at: row.get("last_reminder_sent_at"),
            reminder_count: row.get::<Option<i32>, _>("reminder_count").unwrap_or(0),
            firstname: row.get("firstname"),
            lastname: row.get("lastname"),
            user_email: row.get("user_email"),
            user_language: row.get::<Option<String>, _>("user_language"),
            biblio_id: row.get("biblio_id"),
            title: row.get("title"),
            authors: row.get("authors"),
            item_barcode: row.get("item_barcode"),
            circulation_status: row.try_get("circulation_status").ok().flatten(),
        }
    }
}

const OVERDUE_SELECT: &str = r#"
                l.id as loan_id,
                l.user_id,
                l.date as loan_date,
                l.expiry_at,
                l.last_reminder_sent_at,
                l.reminder_count,
                u.firstname,
                u.lastname,
                u.email as user_email,
                u.language as user_language,
                b.id as biblio_id,
                b.title,
                (
                    SELECT string_agg(a.lastname || ' ' || COALESCE(a.firstname, ''), ', ' ORDER BY ba.position)
                    FROM biblio_authors ba
                    JOIN authors a ON a.id = ba.author_id
                    WHERE ba.biblio_id = b.id
                ) as authors,
                it.barcode as item_barcode,
                it.circulation_status
"#;

/// Same as [`OVERDUE_SELECT`], but name/email/language come from the legal guardian
/// when the borrower is a `child` with a `user_guardians` row. `school` and other
/// types keep the borrower's own contact. Grouping stays on `l.user_id`.
const REMINDER_SELECT: &str = r#"
                l.id as loan_id,
                l.user_id,
                l.date as loan_date,
                l.expiry_at,
                l.last_reminder_sent_at,
                l.reminder_count,
                CASE WHEN g.id IS NOT NULL THEN g.firstname ELSE u.firstname END as firstname,
                CASE WHEN g.id IS NOT NULL THEN g.lastname ELSE u.lastname END as lastname,
                CASE WHEN g.id IS NOT NULL THEN g.email ELSE u.email END as user_email,
                CASE WHEN g.id IS NOT NULL THEN g.language ELSE u.language END as user_language,
                b.id as biblio_id,
                b.title,
                (
                    SELECT string_agg(a.lastname || ' ' || COALESCE(a.firstname, ''), ', ' ORDER BY ba.position)
                    FROM biblio_authors ba
                    JOIN authors a ON a.id = ba.author_id
                    WHERE ba.biblio_id = b.id
                ) as authors,
                it.barcode as item_barcode,
                it.circulation_status
"#;

impl Repository {
    /// Get overdue loans eligible for the next reminder tier.
    ///
    /// Excludes returned loans, lost / claimed-returned items, loans that have
    /// already received the formal notice (`reminder_count >= 3`), and loans
    /// reserved by a pending outbox row. Delays are days after `expiry_at`.
    /// A loan is only eligible for `reminder_count + 1` (no skip, no double-send).
    ///
    /// Recipient contact (email, name, language, `receive_reminders`) is the
    /// legal guardian when the borrower is a `child` with a `user_guardians`
    /// row; otherwise the borrower (`school` included). Grouping remains per
    /// borrower (`l.user_id`).
    pub async fn loans_get_overdue_for_reminders(
        &self,
        delays: ReminderTierDelays,
    ) -> AppResult<Vec<OverdueLoanRow>> {
        let lost = CirculationStatus::LOST;
        let claimed = CirculationStatus::CLAIMED_RETURNED;
        let sql = format!(
            r#"
            SELECT
                {REMINDER_SELECT}
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            JOIN users u ON l.user_id = u.id
            LEFT JOIN public_types pt ON pt.id = u.public_type
            LEFT JOIN user_guardians ug ON ug.child_id = u.id AND pt.name = 'child'
            LEFT JOIN users g ON g.id = ug.guardian_id
            WHERE l.returned_at IS NULL
              AND l.expiry_at < NOW()
              AND COALESCE(l.reminder_count, 0) < $6
              AND COALESCE(it.circulation_status, 0) NOT IN ($4, $5)
              AND (
                  (COALESCE(l.reminder_count, 0) = 0 AND l.expiry_at <= NOW() - ($1 || ' days')::INTERVAL)
                  OR (COALESCE(l.reminder_count, 0) = 1 AND l.expiry_at <= NOW() - ($2 || ' days')::INTERVAL)
                  OR (COALESCE(l.reminder_count, 0) = 2 AND l.expiry_at <= NOW() - ($3 || ' days')::INTERVAL)
              )
              AND CASE
                    WHEN g.id IS NOT NULL THEN (
                        g.email IS NOT NULL AND g.email <> '' AND g.receive_reminders = TRUE
                    )
                    ELSE (
                        u.email IS NOT NULL AND u.email <> '' AND u.receive_reminders = TRUE
                    )
                  END
              AND NOT EXISTS (
                  SELECT 1
                  FROM email_outbox_reminder_loans rl
                  JOIN email_outbox o ON o.id = rl.outbox_id
                  WHERE rl.loan_id = l.id
                    AND o.status = 'pending'
              )
            ORDER BY u.id, l.expiry_at
            "#
        );
        let rows = sqlx::query(&sql)
            .bind(delays.first_days as i64)
            .bind(delays.second_days as i64)
            .bind(delays.formal_notice_days as i64)
            .bind(lost)
            .bind(claimed)
            .bind(delays.max_count)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(OverdueLoanRow::from_row).collect())
    }

    /// Get all overdue loans for the admin dashboard (paginated).
    pub async fn loans_get_overdue(
        &self,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<OverdueLoanRow>, i64)> {
        let offset = (page - 1) * per_page;

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE returned_at IS NULL AND expiry_at < NOW()",
        )
        .fetch_one(&self.pool)
        .await?;

        let sql = format!(
            r#"
            SELECT
                {OVERDUE_SELECT}
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            JOIN users u ON l.user_id = u.id
            WHERE l.returned_at IS NULL
              AND l.expiry_at < NOW()
            ORDER BY l.expiry_at ASC
            LIMIT $1 OFFSET $2
            "#
        );
        let rows = sqlx::query(&sql)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let loans = rows.into_iter().map(OverdueLoanRow::from_row).collect();

        Ok((loans, total))
    }

    /// Mark loans as reminded: update last_reminder_sent_at and increment reminder_count.
    pub async fn loans_update_reminder_sent(&self, loan_ids: &[i64]) -> AppResult<()> {
        if loan_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            r#"
            UPDATE loans
            SET last_reminder_sent_at = NOW(),
                reminder_count = COALESCE(reminder_count, 0) + 1
            WHERE id = ANY($1)
            "#,
        )
        .bind(loan_ids)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
