//! Loans repository for database operations

use chrono::{DateTime, Duration, Utc};
use sqlx::{Pool, Postgres, Row};

use crate::{
    error::{AppError, AppResult},
    models::{
        item::ItemShort,
        loan::{CreateLoan, Loan, LoanDetails, LoanSettings},
        user::{UserShort, UserShortRow},
    },
};

#[derive(Clone)]
pub struct LoansRepository {
    pool: Pool<Postgres>,
}

impl LoansRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Get loan by ID
    pub async fn get_by_id(&self, id: i32) -> AppResult<Loan> {
        sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Loan with id {} not found", id)))
    }

    /// Get loans for a user
    pub async fn get_user_loans(&self, user_id: i32) -> AppResult<Vec<LoanDetails>> {
        let loans = sqlx::query(
            r#"
            SELECT l.*, s.identification as specimen_identification,
                   i.id as item_id, i.media_type, i.identification as item_identification,
                   i.title1, i.publication_date, i.nb_specimens,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s2
                       WHERE s2.id_item = i.id
                         AND s2.lifecycle_status != 2
                         AND NOT EXISTS (
                             SELECT 1 FROM loans l2
                             WHERE l2.specimen_id = s2.id
                               AND l2.returned_date IS NULL
                         )
                   ), 0::smallint)::smallint as nb_available
            FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            JOIN items i ON s.id_item = i.id
            WHERE l.user_id = $1 AND l.returned_date IS NULL
            ORDER BY l.issue_date
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();

        let mut result = Vec::new();
        for row in loans {
            let start_date: DateTime<Utc> = row.get("date");
            let issue_date: Option<DateTime<Utc>> = row.get("issue_date");
            let renew_date: Option<DateTime<Utc>> = row.get("renew_date");
            
            result.push(LoanDetails {
                id: row.get("id"),
                start_date,
                issue_date: issue_date.unwrap_or(now),
                renewal_date: renew_date,
                nb_renews: row.get::<Option<i16>, _>("nb_renews").unwrap_or(0),
                item: ItemShort {
                    id: row.get("item_id"),
                    media_type: row.get("media_type"),
                    identification: row.get("item_identification"),
                    title: row.get("title1"),
                    date: row.get("publication_date"),
                    status: Some(0),
                    is_local: Some(1),
                    is_archive: Some(0),
                    is_valid: Some(1),
                    nb_specimens: row.get("nb_specimens"),
                    nb_available: row.get("nb_available"),
                    authors: Vec::new(),
                    source_name: None,
                },
                user: None,
                specimen_identification: row.get("specimen_identification"),
                is_overdue: issue_date.map(|d| d < now).unwrap_or(false),
            });
        }

        Ok(result)
    }

    /// Create a new loan
    pub async fn create(&self, loan: &CreateLoan) -> AppResult<(i32, DateTime<Utc>)> {
        let now = Utc::now();

        // Get specimen ID
        let specimen_id = if let Some(id) = loan.specimen_id {
            id
        } else if let Some(ref identification) = loan.specimen_identification {
            sqlx::query_scalar::<_, i32>(
                "SELECT id FROM specimens WHERE identification = $1"
            )
            .bind(identification)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Specimen not found".to_string()))?
        } else {
            return Err(AppError::BadRequest("specimen_id or specimen_identification required".to_string()));
        };

        // Check if specimen is already borrowed
        let already_borrowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE specimen_id = $1 AND returned_date IS NULL)"
        )
        .bind(specimen_id)
        .fetch_one(&self.pool)
        .await?;

        if already_borrowed && !loan.force {
            return Err(AppError::BusinessRule("Specimen is already borrowed".to_string()));
        }

        // Get specimen info and loan settings
        let specimen_row = sqlx::query(
            r#"
            SELECT s.id_item, s.status, i.media_type
            FROM specimens s
            JOIN items i ON s.id_item = i.id
            WHERE s.id = $1
            "#
        )
        .bind(specimen_id)
        .fetch_one(&self.pool)
        .await?;

        let status: Option<i16> = specimen_row.get("status");
        let media_type: Option<String> = specimen_row.get("media_type");
        let item_id: i32 = specimen_row.get("id_item");

        // Check if borrowable
        if status != Some(98) && !loan.force {
            return Err(AppError::BusinessRule("Specimen is not borrowable".to_string()));
        }

        // Get loan duration from settings
        let duration_days: i16 = sqlx::query_scalar(
            "SELECT duration FROM loans_settings WHERE media_type = $1"
        )
        .bind(&media_type)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(21); // Default 21 days

        let issue_date = now + Duration::days(duration_days as i64);

        // Check max loans
        let current_loans: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE user_id = $1 AND returned_date IS NULL"
        )
        .bind(loan.user_id)
        .fetch_one(&self.pool)
        .await?;

        let max_loans: i16 = sqlx::query_scalar(
            "SELECT nb_max FROM loans_settings WHERE media_type = $1"
        )
        .bind(&media_type)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(5); // Default 5 loans

        if current_loans >= max_loans as i64 && !loan.force {
            return Err(AppError::BusinessRule(format!(
                "Maximum loans reached ({}/{})",
                current_loans, max_loans
            )));
        }

        // Create the loan
        let loan_id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO loans (user_id, specimen_id, item_id, date, issue_date, nb_renews)
            VALUES ($1, $2, $3, $4, $5, 0)
            RETURNING id
            "#
        )
        .bind(loan.user_id)
        .bind(specimen_id)
        .bind(item_id)
        .bind(now)
        .bind(issue_date)
        .fetch_one(&self.pool)
        .await?;

        Ok((loan_id, issue_date))
    }

    /// Return a loan (moves it to loans_archives)
    pub async fn return_loan(&self, loan_id: i32) -> AppResult<LoanDetails> {
        let now = Utc::now();

        // Get loan details before returning
        let loan = self.get_by_id(loan_id).await?;

        if loan.returned_date.is_some() {
            return Err(AppError::BusinessRule("Loan already returned".to_string()));
        }

        // Get user info for archiving
        let user_row = sqlx::query(
            "SELECT addr_city, account_type, public_type FROM users WHERE id = $1"
        )
        .bind(loan.user_id)
        .fetch_optional(&self.pool)
        .await?;

        let account_type: Option<String> = user_row.as_ref().and_then(|r| r.get("account_type"));

        // Archive the loan
        sqlx::query(
            r#"
            INSERT INTO loans_archives (
                user_id, item_id, specimen_id, date, nb_renews, issue_date, 
                returned_date, notes, borrower_public_type,
                addr_city, account_type
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#
        )
        .bind(loan.user_id)
        .bind(loan.item_id)
        .bind(loan.specimen_id)
        .bind(loan.date)
        .bind(loan.nb_renews)
        .bind(loan.issue_date)
        .bind(now)
        .bind(&loan.notes)
        .bind(user_row.as_ref().and_then(|r| r.get::<Option<i32>, _>("public_type")))
        .bind(user_row.as_ref().and_then(|r| r.get::<Option<String>, _>("addr_city")))
        .bind(account_type)
        .execute(&self.pool)
        .await?;

        // Delete from active loans
        sqlx::query("DELETE FROM loans WHERE id = $1")
            .bind(loan_id)
            .execute(&self.pool)
            .await?;

        // Get item details 
        let item_row = sqlx::query(
            r#"
            SELECT i.*, s.identification as specimen_identification,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s2
                       WHERE s2.id_item = i.id
                         AND s2.lifecycle_status != 2
                         AND NOT EXISTS (
                             SELECT 1 FROM loans l2
                             WHERE l2.specimen_id = s2.id
                               AND l2.returned_date IS NULL
                         )
                   ), 0::smallint)::smallint as nb_available
            FROM items i
            JOIN specimens s ON s.id_item = i.id
            WHERE s.id = $1
            "#
        )
        .bind(loan.specimen_id)
        .fetch_one(&self.pool)
        .await?;

        let user_row = sqlx::query_as::<_, UserShortRow>(
            r#"
            SELECT u.id, u.firstname, u.lastname, u.account_type, u.public_type,
                   0::bigint as nb_loans, 0::bigint as nb_late_loans
            FROM users u
            WHERE u.id = $1
            "#
        )
        .bind(loan.user_id)
        .fetch_optional(&self.pool)
        .await?;

        let user: Option<UserShort> = user_row.map(|r| r.into());

        Ok(LoanDetails {
            id: loan.id,
            start_date: loan.date,
            issue_date: loan.issue_date.unwrap_or(now),
            renewal_date: loan.renew_date,
            nb_renews: loan.nb_renews.unwrap_or(0),
            item: ItemShort {
                id: item_row.get("id"),
                media_type: item_row.get("media_type"),
                identification: item_row.get("identification"),
                title: item_row.get("title1"),
                date: item_row.get("publication_date"),
                status: Some(0),
                is_local: Some(1),
                is_archive: Some(0),
                is_valid: Some(1),
                nb_specimens: item_row.get("nb_specimens"),
                nb_available: item_row.get("nb_available"),
                authors: Vec::new(),
                source_name: None,
            },
            user,
            specimen_identification: item_row.get("specimen_identification"),
            is_overdue: false,
        })
    }

    /// Renew a loan
    pub async fn renew_loan(&self, loan_id: i32) -> AppResult<(DateTime<Utc>, i16)> {
        let now = Utc::now();

        let loan = self.get_by_id(loan_id).await?;

        if loan.returned_date.is_some() {
            return Err(AppError::BusinessRule("Cannot renew a returned loan".to_string()));
        }

        // Get max renewals
        let specimen_row = sqlx::query(
            "SELECT i.media_type FROM specimens s JOIN items i ON s.id_item = i.id WHERE s.id = $1"
        )
        .bind(loan.specimen_id)
        .fetch_one(&self.pool)
        .await?;

        let media_type: Option<String> = specimen_row.get("media_type");

        let max_renews: i16 = sqlx::query_scalar(
            "SELECT nb_renews FROM loans_settings WHERE media_type = $1"
        )
        .bind(&media_type)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(2);

        let current_renews = loan.nb_renews.unwrap_or(0);

        if current_renews >= max_renews {
            return Err(AppError::BusinessRule(format!(
                "Maximum renewals reached ({}/{})",
                current_renews, max_renews
            )));
        }

        // Get loan duration
        let duration_days: i16 = sqlx::query_scalar(
            "SELECT duration FROM loans_settings WHERE media_type = $1"
        )
        .bind(&media_type)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(21);

        let new_issue_date = now + Duration::days(duration_days as i64);
        let new_renews = current_renews + 1;

        sqlx::query(
            "UPDATE loans SET issue_date = $1, renew_date = $2, nb_renews = $3 WHERE id = $4"
        )
        .bind(new_issue_date)
        .bind(now)
        .bind(new_renews)
        .bind(loan_id)
        .execute(&self.pool)
        .await?;

        Ok((new_issue_date, new_renews))
    }

    /// Get loan settings
    pub async fn get_settings(&self) -> AppResult<Vec<LoanSettings>> {
        let settings = sqlx::query_as::<_, LoanSettings>(
            "SELECT * FROM loans_settings ORDER BY media_type"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(settings)
    }

    /// Count active loans
    pub async fn count_active(&self) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loans WHERE returned_date IS NULL")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Count overdue loans
    pub async fn count_overdue(&self) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE returned_date IS NULL AND issue_date < NOW()"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }
}


