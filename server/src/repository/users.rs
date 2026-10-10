//! Users domain methods on Repository

use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::user::{
        AccountTypeSlug, GuardianIdPatch, Rights, UpdateProfile, User, UserErasureResult,
        UserPayload, UserQuery, UserRights, UserShort, UserStatus,
    },
};

/// Minimal user info used for bulk email targeting
#[derive(Debug, sqlx::FromRow)]
pub struct UserEmailTarget {
    pub id: i64,
    pub email: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub language: Option<String>,
}

/// Child named on a guardian's announcement. Empty for a direct send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementChild {
    pub id: i64,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
}

/// One announcement email. `children` lists every targeted child this recipient
/// stands in for, and is empty when the patron is emailed directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementRecipient {
    pub id: i64,
    pub email: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub language: Option<String>,
    pub children: Vec<AnnouncementChild>,
    /// True when this recipient still needs the one-time migration explanation.
    pub include_migration_notice: bool,
}

/// Flat row from [`ANNOUNCEMENT_RECIPIENTS_SQL`] before grouping.
/// `child_id` is null for a direct send and set when the row routes one child.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub(crate) struct AnnouncementRecipientRow {
    id: i64,
    email: Option<String>,
    firstname: Option<String>,
    lastname: Option<String>,
    language: Option<String>,
    child_id: Option<i64>,
    child_firstname: Option<String>,
    child_lastname: Option<String>,
    include_migration_notice: bool,
}

/// SQL for [`Repository::users_list_announcement_recipients`].
///
/// `$1::boolean` is all-audiences (no type filter). `$2::text[]` is the audience names.
/// Direct rows are non-`child` patrons. Each active `child` in the audience adds a row
/// for their major guardian (`user_guardians`). Consent, email, and deletion are checked
/// on the recipient, never on the child. A child routed to a guardian uses the
/// guardian's `events_consent_at`. [`group_announcement_recipients`] collapses
/// these rows to one recipient.
pub(crate) const ANNOUNCEMENT_RECIPIENTS_SQL: &str = r#"
SELECT
    id,
    email,
    firstname,
    lastname,
    language,
    child_id,
    child_firstname,
    child_lastname,
    include_migration_notice
FROM (
    SELECT
        u.id,
        u.email,
        u.firstname,
        u.lastname,
        u.language,
        NULL::bigint AS child_id,
        NULL::text AS child_firstname,
        NULL::text AS child_lastname,
        (u.events_consent_source = 'migration' AND u.events_consent_notice_at IS NULL) AS include_migration_notice
    FROM users u
    LEFT JOIN public_types pt ON pt.id = u.public_type
    WHERE u.events_consent_at IS NOT NULL
      AND u.email IS NOT NULL
      AND u.email <> ''
      AND (u.status IS NULL OR u.status <> 'deleted')
      AND pt.name IS DISTINCT FROM 'child'
      AND ($1::boolean OR pt.name = ANY($2::text[]))

    UNION ALL

    SELECT
        g.id,
        g.email,
        g.firstname,
        g.lastname,
        g.language,
        c.id AS child_id,
        c.firstname AS child_firstname,
        c.lastname AS child_lastname,
        (g.events_consent_source = 'migration' AND g.events_consent_notice_at IS NULL) AS include_migration_notice
    FROM users c
    JOIN public_types cpt ON cpt.id = c.public_type AND cpt.name = 'child'
    JOIN user_guardians ug ON ug.child_id = c.id
    JOIN users g ON g.id = ug.guardian_id
    LEFT JOIN public_types gpt ON gpt.id = g.public_type
    WHERE (c.status IS NULL OR c.status <> 'deleted')
      AND ($1::boolean OR cpt.name = ANY($2::text[]))
      AND g.events_consent_at IS NOT NULL
      AND g.email IS NOT NULL
      AND g.email <> ''
      AND (g.status IS NULL OR g.status <> 'deleted')
      AND gpt.name IS DISTINCT FROM 'child'
      AND gpt.name IS DISTINCT FROM 'school'
) AS announcement_recipients
ORDER BY id, child_id NULLS FIRST
"#;

/// Collapse SQL rows into one recipient per patron.
///
/// A null `child_id` is a direct send. Several children of the same guardian,
/// or a guardian who is also targeted directly, become one recipient whose
/// `children` lists each child once, ordered by id. Recipient order is first-seen.
pub(crate) fn group_announcement_recipients(
    rows: Vec<AnnouncementRecipientRow>,
) -> Vec<AnnouncementRecipient> {
    let mut order: Vec<i64> = Vec::new();
    let mut by_id: std::collections::HashMap<i64, AnnouncementRecipient> =
        std::collections::HashMap::new();

    for row in rows {
        let id = row.id;
        let notice = row.include_migration_notice;
        match by_id.entry(id) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                order.push(id);
                slot.insert(AnnouncementRecipient {
                    id,
                    email: row.email,
                    firstname: row.firstname,
                    lastname: row.lastname,
                    language: row.language,
                    children: Vec::new(),
                    include_migration_notice: notice,
                });
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                if notice {
                    slot.get_mut().include_migration_notice = true;
                }
            }
        }
        if let Some(child_id) = row.child_id {
            if let Some(recipient) = by_id.get_mut(&id) {
                if !recipient.children.iter().any(|child| child.id == child_id) {
                    recipient.children.push(AnnouncementChild {
                        id: child_id,
                        firstname: row.child_firstname,
                        lastname: row.child_lastname,
                    });
                }
            }
        }
    }

    let mut recipients: Vec<AnnouncementRecipient> = order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect();
    for recipient in &mut recipients {
        recipient.children.sort_by_key(|child| child.id);
    }
    recipients
}

/// Patron fields for hold-ready notification email.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HoldReadyUserContact {
    pub email: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub language: Option<String>,
}

// Note: not `mockall::automock` — several methods use `Option<&str>` which mockall cannot derive for.
#[async_trait]
pub trait UsersRepository: Send + Sync {
    async fn users_get_by_id(&self, id: i64) -> AppResult<User>;
    async fn users_get_by_login(&self, login: &str) -> AppResult<Option<User>>;
    async fn users_get_by_email(&self, email: &str) -> AppResult<Option<User>>;
    async fn users_get_token_version(&self, id: i64) -> AppResult<i64>;
    async fn users_update_password(&self, id: i64, password_hash: &str) -> AppResult<()>;
    async fn users_email_exists(&self, email: &str, exclude_id: Option<i64>) -> AppResult<bool>;
    async fn users_login_exists(&self, login: &str, exclude_id: Option<i64>) -> AppResult<bool>;
    async fn users_get_rights(&self, account_type: &AccountTypeSlug) -> AppResult<UserRights>;
    async fn users_search(&self, query: &UserQuery) -> AppResult<(Vec<UserShort>, i64)>;
    async fn users_create(&self, user: &UserPayload, password: Option<String>) -> AppResult<User>;
    async fn users_update(
        &self,
        id: i64,
        user: &UserPayload,
        password: Option<String>,
    ) -> AppResult<User>;
    async fn users_delete(&self, id: i64, force: bool) -> AppResult<UserErasureResult>;
    async fn users_list_due_for_auto_erasure(&self, years_after_expiry: u32)
        -> AppResult<Vec<i64>>;
    async fn users_block(&self, id: i64) -> AppResult<User>;
    async fn users_unblock(&self, id: i64) -> AppResult<User>;
    async fn users_update_profile(
        &self,
        id: i64,
        profile: &UpdateProfile,
        password: Option<String>,
    ) -> AppResult<User>;
    async fn users_update_account_type(
        &self,
        id: i64,
        account_type: &AccountTypeSlug,
    ) -> AppResult<User>;
    async fn users_update_2fa_settings(
        &self,
        id: i64,
        enabled: bool,
        method: Option<&str>,
        totp_secret: Option<&str>,
        recovery_codes: Option<&str>,
    ) -> AppResult<()>;
    async fn users_mark_recovery_code_used(&self, id: i64, used_codes: &str) -> AppResult<()>;
    async fn users_get_emails_by_public_type(
        &self,
        public_type: Option<i64>,
    ) -> AppResult<Vec<UserEmailTarget>>;
    /// Patrons who should receive an event announcement, one entry per recipient.
    /// `all_audiences` skips the type filter. Otherwise `public_types.name` must be in `audience_names`.
    /// A `child` is not emailed; their major guardian is, when the child is still active.
    /// Consent is `events_consent_at IS NOT NULL` on the adult recipient, or on the
    /// guardian when a child is routed. The child's own consent is not consulted.
    async fn users_list_announcement_recipients(
        &self,
        all_audiences: bool,
        audience_names: &[String],
    ) -> AppResult<Vec<AnnouncementRecipient>>;
    async fn users_count(&self) -> AppResult<i64>;
    async fn users_set_must_change_password(&self, id: i64, value: bool) -> AppResult<()>;
    async fn users_hold_ready_contact(
        &self,
        user_id: i64,
    ) -> AppResult<Option<HoldReadyUserContact>>;
}

// ---------------------------------------------------------------------------
// Trait implementation — forwards to inherent methods above.
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl UsersRepository for Repository {
    async fn users_get_by_id(&self, id: i64) -> crate::error::AppResult<User> {
        Repository::users_get_by_id(self, id).await
    }
    async fn users_get_by_login(&self, login: &str) -> crate::error::AppResult<Option<User>> {
        Repository::users_get_by_login(self, login).await
    }
    async fn users_get_by_email(&self, email: &str) -> crate::error::AppResult<Option<User>> {
        Repository::users_get_by_email(self, email).await
    }
    async fn users_get_token_version(&self, id: i64) -> crate::error::AppResult<i64> {
        Repository::users_get_token_version(self, id).await
    }
    async fn users_update_password(
        &self,
        id: i64,
        password_hash: &str,
    ) -> crate::error::AppResult<()> {
        Repository::users_update_password(self, id, password_hash).await
    }
    async fn users_email_exists(
        &self,
        email: &str,
        exclude_id: Option<i64>,
    ) -> crate::error::AppResult<bool> {
        Repository::users_email_exists(self, email, exclude_id).await
    }
    async fn users_login_exists(
        &self,
        login: &str,
        exclude_id: Option<i64>,
    ) -> crate::error::AppResult<bool> {
        Repository::users_login_exists(self, login, exclude_id).await
    }
    async fn users_get_rights(
        &self,
        account_type: &crate::models::user::AccountTypeSlug,
    ) -> crate::error::AppResult<crate::models::user::UserRights> {
        Repository::users_get_rights(self, account_type).await
    }
    async fn users_search(
        &self,
        query: &crate::models::user::UserQuery,
    ) -> crate::error::AppResult<(Vec<crate::models::user::UserShort>, i64)> {
        Repository::users_search(self, query).await
    }
    async fn users_create(
        &self,
        user: &crate::models::user::UserPayload,
        password: Option<String>,
    ) -> crate::error::AppResult<User> {
        Repository::users_create(self, user, password).await
    }
    async fn users_update(
        &self,
        id: i64,
        user: &crate::models::user::UserPayload,
        password: Option<String>,
    ) -> crate::error::AppResult<User> {
        Repository::users_update(self, id, user, password).await
    }
    async fn users_delete(
        &self,
        id: i64,
        force: bool,
    ) -> crate::error::AppResult<crate::models::user::UserErasureResult> {
        Repository::users_delete(self, id, force).await
    }
    async fn users_list_due_for_auto_erasure(
        &self,
        years_after_expiry: u32,
    ) -> crate::error::AppResult<Vec<i64>> {
        Repository::users_list_due_for_auto_erasure(self, years_after_expiry).await
    }
    async fn users_block(&self, id: i64) -> crate::error::AppResult<User> {
        Repository::users_block(self, id).await
    }
    async fn users_unblock(&self, id: i64) -> crate::error::AppResult<User> {
        Repository::users_unblock(self, id).await
    }
    async fn users_update_profile(
        &self,
        id: i64,
        profile: &crate::models::user::UpdateProfile,
        password: Option<String>,
    ) -> crate::error::AppResult<User> {
        Repository::users_update_profile(self, id, profile, password).await
    }
    async fn users_update_account_type(
        &self,
        id: i64,
        account_type: &crate::models::user::AccountTypeSlug,
    ) -> crate::error::AppResult<User> {
        Repository::users_update_account_type(self, id, account_type).await
    }
    async fn users_update_2fa_settings(
        &self,
        id: i64,
        enabled: bool,
        method: Option<&str>,
        totp_secret: Option<&str>,
        recovery_codes: Option<&str>,
    ) -> crate::error::AppResult<()> {
        Repository::users_update_2fa_settings(
            self,
            id,
            enabled,
            method,
            totp_secret,
            recovery_codes,
        )
        .await
    }
    async fn users_mark_recovery_code_used(
        &self,
        id: i64,
        used_codes: &str,
    ) -> crate::error::AppResult<()> {
        Repository::users_mark_recovery_code_used(self, id, used_codes).await
    }
    async fn users_get_emails_by_public_type(
        &self,
        public_type: Option<i64>,
    ) -> crate::error::AppResult<Vec<UserEmailTarget>> {
        Repository::users_get_emails_by_public_type(self, public_type).await
    }
    async fn users_list_announcement_recipients(
        &self,
        all_audiences: bool,
        audience_names: &[String],
    ) -> crate::error::AppResult<Vec<AnnouncementRecipient>> {
        Repository::users_list_announcement_recipients(self, all_audiences, audience_names).await
    }
    async fn users_count(&self) -> crate::error::AppResult<i64> {
        Repository::users_count(self).await
    }
    async fn users_set_must_change_password(
        &self,
        id: i64,
        value: bool,
    ) -> crate::error::AppResult<()> {
        Repository::users_set_must_change_password(self, id, value).await
    }
    async fn users_hold_ready_contact(
        &self,
        user_id: i64,
    ) -> crate::error::AppResult<Option<HoldReadyUserContact>> {
        Repository::users_hold_ready_contact(self, user_id).await
    }
}

impl Repository {
    /// Get user by ID
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_by_id(&self, id: i64) -> AppResult<User> {
        use crate::models::user::UserRow;
        let user_row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User with id {} not found", id)))?;

        let mut user: User = user_row.into();
        user.guardian_id = self.users_guardian_id(id).await?;
        Ok(user)
    }

    /// Legal guardian of `child_id`, if a `user_guardians` row exists.
    pub async fn users_guardian_id(&self, child_id: i64) -> AppResult<Option<i64>> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT guardian_id FROM user_guardians WHERE child_id = $1",
        )
        .bind(child_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Insert or replace the unique guardian for `child_id`.
    pub async fn users_set_guardian(&self, child_id: i64, guardian_id: i64) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO user_guardians (child_id, guardian_id)
            VALUES ($1, $2)
            ON CONFLICT (child_id) DO UPDATE SET guardian_id = EXCLUDED.guardian_id
            "#,
        )
        .bind(child_id)
        .bind(guardian_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove the legal guardian link for `child_id`, if any.
    pub async fn users_clear_guardian(&self, child_id: i64) -> AppResult<()> {
        sqlx::query("DELETE FROM user_guardians WHERE child_id = $1")
            .bind(child_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get user by login (primary authentication method)
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_by_login(&self, login: &str) -> AppResult<Option<User>> {
        use crate::models::user::UserRow;
        let user_row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users WHERE LOWER(login) = LOWER($1) AND (status IS NULL OR status <> 'deleted')
            "#,
        )
        .bind(login)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row.map(|r| r.into()))
    }

    /// Get user by email (primary authentication method)
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_by_email(&self, email: &str) -> AppResult<Option<User>> {
        use crate::models::user::UserRow;
        let user_row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT * FROM users WHERE LOWER(email) = LOWER($1) AND (status IS NULL OR status <> 'deleted')
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user_row.map(|r| r.into()))
    }

    /// Current JWT revocation counter for a user.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_token_version(&self, id: i64) -> AppResult<i64> {
        let version: Option<i64> =
            sqlx::query_scalar("SELECT token_version FROM users WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        version.ok_or_else(|| AppError::NotFound(format!("User with id {} not found", id)))
    }

    /// Update user password directly (used for password reset flow).
    /// Also clears the must_change_password flag and bumps `token_version`.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_update_password(&self, id: i64, password_hash: &str) -> AppResult<()> {
        let result = sqlx::query("UPDATE users SET password = $1, must_change_password = FALSE, token_version = token_version + 1, update_at = NOW() WHERE id = $2")
            .bind(password_hash)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User with id {} not found", id)));
        }

        Ok(())
    }

    /// Count total users (used to detect first-run empty database).
    #[tracing::instrument(skip(self), err)]
    pub async fn users_count(&self) -> AppResult<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Set or clear the must_change_password flag for a user.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_set_must_change_password(&self, id: i64, value: bool) -> AppResult<()> {
        let result = sqlx::query(
            "UPDATE users SET must_change_password = $1, update_at = NOW() WHERE id = $2",
        )
        .bind(value)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("User with id {} not found", id)));
        }

        Ok(())
    }

    /// Check if email already exists
    #[tracing::instrument(skip(self), err)]
    pub async fn users_email_exists(
        &self,
        email: &str,
        exclude_id: Option<i64>,
    ) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_id {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(email) = LOWER($1) AND id != $2)",
            )
            .bind(email)
            .bind(id)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(email) = LOWER($1))")
                .bind(email)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists)
    }

    /// Check if login already exists
    #[tracing::instrument(skip(self), err)]
    pub async fn users_login_exists(
        &self,
        login: &str,
        exclude_id: Option<i64>,
    ) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(login) = LOWER($1) AND id != $2 AND (status IS NULL OR status <> 'deleted'))")
                .bind(login)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE LOWER(login) = LOWER($1) AND (status IS NULL OR status <> 'deleted'))")
                .bind(login)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists)
    }

    /// Get user rights from account type
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_rights(&self, account_type: &AccountTypeSlug) -> AppResult<UserRights> {
        let row = sqlx::query(
            r#"
            SELECT items_rights, users_rights, loans_rights,
                   holds_rights, settings_rights, events_rights, acquisitions_rights
            FROM account_types
            WHERE code = $1
            "#,
        )
        .bind(account_type.as_str())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Account type '{}' not found",
                account_type.as_str()
            ))
        })?;

        Ok(UserRights {
            items_rights: Rights::from(row.get::<Option<String>, _>("items_rights")),
            users_rights: Rights::from(row.get::<Option<String>, _>("users_rights")),
            loans_rights: Rights::from(row.get::<Option<String>, _>("loans_rights")),
            holds_rights: Rights::from(row.get::<Option<String>, _>("holds_rights")),
            settings_rights: Rights::from(row.get::<Option<String>, _>("settings_rights")),
            events_rights: Rights::from(row.get::<Option<String>, _>("events_rights")),
            acquisitions_rights: Rights::from(row.get::<Option<String>, _>("acquisitions_rights")),
        })
    }

    /// Search users with pagination
    #[tracing::instrument(skip(self), err)]
    pub async fn users_search(&self, query: &UserQuery) -> AppResult<(Vec<UserShort>, i64)> {
        let page = query.page.unwrap_or(1);
        let per_page = query.per_page.unwrap_or(20);
        let offset = (page - 1) * per_page;

        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(ref name) = query.name {
            params.push(format!("%{}%", name.to_lowercase()));
            conditions.push(format!(
                "(LOWER(firstname) LIKE ${} OR LOWER(lastname) LIKE ${})",
                params.len(),
                params.len()
            ));
        }

        if let Some(ref barcode) = query.barcode {
            params.push(barcode.clone());
            conditions.push(format!("barcode = ${}", params.len()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Count total
        let count_query = format!(
            r#"
            SELECT COUNT(*) as count FROM users {}
            "#,
            where_clause
        );

        let mut count_builder = sqlx::query_scalar::<_, i64>(&count_query);
        for param in &params {
            count_builder = count_builder.bind(param);
        }
        let total = count_builder.fetch_one(&self.pool).await?;

        // Fetch users (exclude deleted users by default)
        let status_filter = if conditions.is_empty() {
            "WHERE (u.status IS NULL OR u.status <> 'deleted')".to_string()
        } else {
            " AND (u.status IS NULL OR u.status <> 'deleted')".to_string()
        };

        use crate::models::user::UserShortRow;
        let select_query = format!(
            r#"
            SELECT u.id, u.firstname, u.lastname, u.account_type, u.public_type,
                   u.status, u.created_at, u.expiry_at,
                   (SELECT COUNT(*) FROM loans l WHERE l.user_id = u.id AND l.returned_at IS NULL) as nb_loans,
                   (SELECT COUNT(*) FROM loans l WHERE l.user_id = u.id AND l.returned_at IS NULL AND l.expiry_at < NOW()) as nb_late_loans
            FROM users u
            {}{}
            ORDER BY u.lastname, u.firstname
            LIMIT {} OFFSET {}
            "#,
            where_clause, status_filter, per_page, offset
        );

        let mut select_builder = sqlx::query_as::<_, UserShortRow>(&select_query);
        for param in &params {
            select_builder = select_builder.bind(param);
        }
        let user_rows = select_builder.fetch_all(&self.pool).await?;
        let users: Vec<UserShort> = user_rows.into_iter().map(|r| r.into()).collect();

        Ok((users, total))
    }

    /// Create a new user
    #[tracing::instrument(skip(self), err)]
    pub async fn users_create(
        &self,
        user: &UserPayload,
        password: Option<String>,
    ) -> AppResult<User> {
        let now = Utc::now();

        let account_type = user
            .account_type
            .as_ref()
            .map(|at| at.as_str())
            .unwrap_or("guest");
        let fee = user.fee.as_ref().map(|f| f.as_str());

        // Parse staff dates
        let staff_start_date = user
            .staff_start_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let staff_end_date = user
            .staff_end_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let hours_pw = user.hours_per_week.map(|v| v as f32);

        let mut tx = self.pool.begin().await?;
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO users (
                login, password, firstname, lastname, email,
                addr_street, addr_zip_code, addr_city, phone,
                birthdate, account_type,
                fee, public_type, notes, group_id, barcode,
                sex, staff_type, hours_per_week, staff_start_date, staff_end_date,
                status, created_at, update_at, expiry_at, must_change_password
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26
            ) RETURNING id
            "#,
        )
        .bind(
            user.login
                .as_deref()
                .ok_or_else(|| AppError::Validation("Login is required".to_string()))?,
        )
        .bind(&password)
        .bind(&user.firstname)
        .bind(&user.lastname)
        .bind(&user.email)
        .bind(&user.addr_street)
        .bind(user.addr_zip_code)
        .bind(&user.addr_city)
        .bind(&user.phone)
        .bind(user.birthdate)
        .bind(account_type)
        .bind(fee)
        .bind(user.public_type)
        .bind(&user.notes)
        .bind(user.group_id)
        .bind(&user.barcode)
        .bind(user.sex)
        .bind(user.staff_type)
        .bind(hours_pw)
        .bind(staff_start_date)
        .bind(staff_end_date)
        .bind(UserStatus::Active)
        .bind(now)
        .bind(now)
        .bind(user.expiry_at)
        .bind(true)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(guardian_id) = user.guardian_id.as_id() {
            sqlx::query(
                r#"
                INSERT INTO user_guardians (child_id, guardian_id)
                VALUES ($1, $2)
                "#,
            )
            .bind(id)
            .bind(guardian_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        self.users_get_by_id(id).await
    }

    /// Update an existing user
    #[tracing::instrument(skip(self), err)]
    pub async fn users_update(
        &self,
        id: i64,
        user: &UserPayload,
        password: Option<String>,
    ) -> AppResult<User> {
        // Build dynamic update query ($1..$N consecutive; `update_at` uses NOW() in SQL, not a bind)
        let mut sets = vec![];
        let mut param_idx = 1;

        macro_rules! add_field {
            ($field:expr, $name:expr) => {
                if $field.is_some() {
                    sets.push(format!("{} = ${}", $name, param_idx));
                    param_idx += 1;
                }
            };
        }

        macro_rules! add_field_enum {
            ($field:expr, $name:expr) => {
                if $field.is_some() {
                    sets.push(format!("{} = ${}", $name, param_idx));
                    param_idx += 1;
                }
            };
        }

        add_field!(user.login, "login");
        add_field!(user.firstname, "firstname");
        add_field!(user.lastname, "lastname");
        add_field!(user.email, "email");
        add_field!(user.addr_street, "addr_street");
        add_field!(user.addr_zip_code, "addr_zip_code");
        add_field!(user.addr_city, "addr_city");
        add_field!(user.phone, "phone");
        add_field!(user.birthdate, "birthdate");
        add_field_enum!(user.account_type, "account_type");
        add_field_enum!(user.fee, "fee");
        add_field!(user.public_type, "public_type");
        add_field!(user.notes, "notes");
        add_field!(user.group_id, "group_id");
        add_field!(user.barcode, "barcode");
        add_field!(user.status, "status");
        add_field!(user.sex, "sex");
        // expiry_at may be NULL for unlimited membership
        sets.push(format!("expiry_at = ${}", param_idx));
        param_idx += 1;
        add_field!(user.staff_type, "staff_type");
        add_field!(user.hours_per_week, "hours_per_week");
        add_field!(user.staff_start_date, "staff_start_date");
        add_field!(user.staff_end_date, "staff_end_date");

        let bumps_token_version = password.is_some() || user.account_type.is_some();
        if password.is_some() {
            sets.push(format!("password = ${}", param_idx));
        }
        if bumps_token_version {
            sets.push("token_version = token_version + 1".to_string());
        }

        let query = format!(
            "UPDATE users SET {}, update_at = NOW() WHERE id = {}",
            sets.join(", "),
            id
        );

        // Parse staff dates before binding
        let staff_start_date = user
            .staff_start_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let staff_end_date = user
            .staff_end_date
            .as_ref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let hours_pw = user.hours_per_week.map(|v| v as f32);

        let mut builder = sqlx::query(&query);

        macro_rules! bind_field {
            ($field:expr) => {
                if let Some(ref val) = $field {
                    builder = builder.bind(val);
                }
            };
        }

        macro_rules! bind_field_enum {
            ($field:expr) => {
                if let Some(ref val) = $field {
                    builder = builder.bind(val.as_str());
                }
            };
        }

        bind_field!(user.login);
        bind_field!(user.firstname);
        bind_field!(user.lastname);
        bind_field!(user.email);
        bind_field!(user.addr_street);
        bind_field!(user.addr_zip_code);
        bind_field!(user.addr_city);
        bind_field!(user.phone);
        bind_field!(user.birthdate);
        bind_field_enum!(user.account_type);
        bind_field_enum!(user.fee);
        bind_field!(user.public_type);
        bind_field!(user.notes);
        bind_field!(user.group_id);
        bind_field!(user.barcode);
        bind_field!(user.status);
        bind_field!(user.sex);
        builder = builder.bind(user.expiry_at);
        bind_field!(user.staff_type);

        if user.hours_per_week.is_some() {
            builder = builder.bind(hours_pw);
        }
        if user.staff_start_date.is_some() {
            builder = builder.bind(staff_start_date);
        }
        if user.staff_end_date.is_some() {
            builder = builder.bind(staff_end_date);
        }

        if let Some(ref hash) = password {
            builder = builder.bind(hash);
        }

        builder.execute(&self.pool).await?;

        match user.guardian_id {
            GuardianIdPatch::Set(guardian_id) => {
                self.users_set_guardian(id, guardian_id).await?;
            }
            GuardianIdPatch::Clear => {
                self.users_clear_guardian(id).await?;
            }
            GuardianIdPatch::Unspecified => {}
        }

        self.users_get_by_id(id).await
    }

    /// Anonymize a patron for stats: snapshot archive dimensions, null history `user_id`,
    /// full PII scrub on the `users` row. Does **not** rewrite `users.id`.
    ///
    /// # `users` column checklist
    /// **Scrubbed:** login, password, firstname, lastname, email, addr_street, addr_zip_code,
    /// addr_city, phone, fee, group_id, barcode, notes, birthdate, language, sex, staff_type,
    /// hours_per_week, staff_start_date, staff_end_date, two_factor_method, totp_secret,
    /// recovery_codes, recovery_codes_used.
    /// **Reset:** receive_reminders=false, events_consent_at=null, events_consent_source=null,
    /// events_consent_notice_at=null, two_factor_enabled=false, must_change_password=false,
    /// token_version+=1, status=deleted, archived_at/update_at=now.
    /// **Kept (non-identifying / operational):** id, account_type, public_type, created_at, expiry_at.
    /// `events_consent_changed_at` is refreshed so the withdrawal stays auditable.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_delete(&self, id: i64, force: bool) -> AppResult<UserErasureResult> {
        let user = self.users_get_by_id(id).await?;

        let active_loans = self.loans_get_active_ids_for_user(id).await?;
        let mut loans_force_returned = 0_u64;

        if !active_loans.is_empty() {
            if !force {
                return Err(AppError::BusinessRule(
                    "User has active loans. Use force=true to delete anyway.".to_string(),
                ));
            }
            for loan_id in active_loans {
                self.loans_return(loan_id).await?;
                loans_force_returned += 1;
            }
        }

        // Soft-delete does not remove the `users` row, so ON DELETE CASCADE on `holds` does not run.
        let mut tx = self.pool.begin().await?;

        if let Some(ref email) = user.email {
            sqlx::query("DELETE FROM email_outbox WHERE status = 'pending' AND to_addr = $1")
                .bind(email)
                .execute(&mut *tx)
                .await?;
        }

        let hold_items: Vec<Option<i64>> =
            sqlx::query_scalar("DELETE FROM holds WHERE user_id = $1 RETURNING item_id")
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;
        let holds_cancelled = hold_items.len() as u64;
        let mut seen_items = std::collections::HashSet::new();
        for item_id in hold_items.into_iter().flatten() {
            if seen_items.insert(item_id) {
                self.holds_notify_next_tx(&mut tx, item_id, self.hold_ready_expiry_days())
                    .await?;
            }
        }

        let archives_unlinked = sqlx::query(
            r#"
            UPDATE loans_archives la
            SET
                borrower_public_type = COALESCE(la.borrower_public_type, u.public_type),
                addr_city = COALESCE(la.addr_city, u.addr_city),
                account_type = COALESCE(la.account_type, u.account_type),
                loan_year = COALESCE(la.loan_year, EXTRACT(YEAR FROM la.date)::smallint),
                age_band = COALESCE(
                    la.age_band,
                    CASE
                        WHEN u.birthdate IS NULL OR la.date IS NULL THEN NULL
                        WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 18 THEN '0-17'
                        WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 30 THEN '18-29'
                        WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 50 THEN '30-49'
                        WHEN EXTRACT(YEAR FROM AGE(la.date::date, u.birthdate)) < 65 THEN '50-64'
                        ELSE '65+'
                    END
                ),
                user_id = NULL
            FROM users u
            WHERE la.user_id = u.id AND u.id = $1
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        let fines_unlinked = sqlx::query("UPDATE fines SET user_id = NULL WHERE user_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

        sqlx::query(
            r#"
            UPDATE users SET
                login = NULL,
                password = NULL,
                firstname = NULL,
                lastname = NULL,
                email = NULL,
                addr_street = NULL,
                addr_zip_code = NULL,
                addr_city = NULL,
                phone = NULL,
                fee = NULL,
                group_id = NULL,
                barcode = NULL,
                notes = NULL,
                birthdate = NULL,
                language = NULL,
                sex = NULL,
                staff_type = NULL,
                hours_per_week = NULL,
                staff_start_date = NULL,
                staff_end_date = NULL,
                receive_reminders = FALSE,
                events_consent_at = NULL,
                events_consent_source = NULL,
                events_consent_changed_at = NOW(),
                events_consent_notice_at = NULL,
                two_factor_enabled = FALSE,
                two_factor_method = NULL,
                totp_secret = NULL,
                recovery_codes = NULL,
                recovery_codes_used = NULL,
                must_change_password = FALSE,
                token_version = token_version + 1,
                status = $1,
                archived_at = COALESCE(archived_at, NOW()),
                update_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(UserStatus::Deleted)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(UserErasureResult {
            force,
            loans_force_returned,
            holds_cancelled,
            archives_unlinked,
            fines_unlinked,
        })
    }

    /// Memberships expired at least `years_after_expiry` years ago and not yet erased.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_list_due_for_auto_erasure(
        &self,
        years_after_expiry: u32,
    ) -> AppResult<Vec<i64>> {
        if years_after_expiry == 0 {
            return Ok(Vec::new());
        }
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id FROM users
            WHERE (status IS NULL OR status <> 'deleted')
              AND expiry_at IS NOT NULL
              AND expiry_at < NOW() - make_interval(years => $1::int)
            ORDER BY id
            LIMIT 500
            "#,
        )
        .bind(i32::try_from(years_after_expiry).unwrap_or(i32::MAX))
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }

    /// Block a user
    #[tracing::instrument(skip(self), err)]
    pub async fn users_block(&self, id: i64) -> AppResult<User> {
        sqlx::query("UPDATE users SET status = $1, update_at = NOW() WHERE id = $2")
            .bind(UserStatus::Blocked)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.users_get_by_id(id).await
    }

    /// Unblock a user
    #[tracing::instrument(skip(self), err)]
    pub async fn users_unblock(&self, id: i64) -> AppResult<User> {
        sqlx::query("UPDATE users SET status = $1, update_at = NOW() WHERE id = $2")
            .bind(UserStatus::Active)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.users_get_by_id(id).await
    }

    /// Update user's own profile (firstname, lastname, password)
    #[tracing::instrument(skip(self), err)]
    pub async fn users_update_profile(
        &self,
        id: i64,
        profile: &UpdateProfile,
        password: Option<String>,
    ) -> AppResult<User> {
        let mut sets = vec![];
        let mut param_idx = 1;

        // Helper macro to add fields
        macro_rules! add_field {
            ($field:expr, $name:expr) => {
                if $field.is_some() {
                    sets.push(format!("{} = ${}", $name, param_idx));
                    param_idx += 1;
                }
            };
        }

        add_field!(profile.firstname, "firstname");
        add_field!(profile.lastname, "lastname");
        add_field!(profile.email, "email");
        add_field!(profile.login, "login");
        add_field!(profile.addr_street, "addr_street");
        add_field!(profile.addr_zip_code, "addr_zip_code");
        add_field!(profile.addr_city, "addr_city");
        add_field!(profile.phone, "phone");
        add_field!(profile.birthdate, "birthdate");
        add_field!(profile.language, "language");

        if password.is_some() {
            add_field!(password, "password");
            // Changing password clears the forced-change flag
            sets.push(format!("must_change_password = ${}", param_idx));
            sets.push("token_version = token_version + 1".to_string());
        }

        let query = format!(
            "UPDATE users SET {}, update_at = NOW() WHERE id = {}",
            sets.join(", "),
            id
        );

        let mut builder = sqlx::query(&query);

        // Helper macro to bind fields
        macro_rules! bind_field {
            ($builder:expr, $field:expr) => {
                if let Some(ref val) = $field {
                    $builder = $builder.bind(val);
                }
            };
        }

        bind_field!(builder, profile.firstname);
        bind_field!(builder, profile.lastname);
        bind_field!(builder, profile.email);
        bind_field!(builder, profile.login);
        bind_field!(builder, profile.addr_street);
        bind_field!(builder, profile.addr_zip_code);
        bind_field!(builder, profile.addr_city);
        bind_field!(builder, profile.phone);
        bind_field!(builder, profile.birthdate);
        if let Some(ref lang) = profile.language {
            builder = builder.bind(lang.as_db_str());
        }

        if let Some(ref hash) = password {
            builder = builder.bind(hash);
            // Bind false for must_change_password (cleared when user sets a new password)
            builder = builder.bind(false);
        }

        // Consent-only profile edits have no column in this statement.
        if !sets.is_empty() {
            builder.execute(&self.pool).await?;
        }

        self.users_get_by_id(id).await
    }

    /// Record an event-announcement consent change.
    ///
    /// Granting consent keeps the existing timestamp and source when the patron
    /// is already consented. Withdrawing consent clears the timestamp, stores
    /// `source` (the route that recorded the withdrawal), and refreshes
    /// `events_consent_changed_at`.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_set_events_consent(
        &self,
        id: i64,
        grant: bool,
        source: &str,
    ) -> AppResult<()> {
        if grant {
            sqlx::query(
                r#"
                UPDATE users SET
                    events_consent_changed_at = CASE
                        WHEN events_consent_at IS NULL THEN NOW()
                        ELSE events_consent_changed_at
                    END,
                    events_consent_source = CASE
                        WHEN events_consent_at IS NULL THEN $2
                        ELSE events_consent_source
                    END,
                    events_consent_at = COALESCE(events_consent_at, NOW())
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(source)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE users SET
                    events_consent_at = NULL,
                    events_consent_source = $2,
                    events_consent_changed_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(source)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Withdraw event-announcement consent from the public unsubscribe endpoint.
    ///
    /// A patron who is already withdrawn stays that way: the stored source and
    /// change timestamp are left as they are, and the caller still returns success.
    #[tracing::instrument(skip(self), err)]
    pub async fn users_unsubscribe_events(&self, id: i64) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE users SET
                events_consent_at = NULL,
                events_consent_source = 'unsubscribe',
                events_consent_changed_at = NOW()
            WHERE id = $1
              AND (
                    events_consent_at IS NOT NULL
                    OR events_consent_source IS DISTINCT FROM 'unsubscribe'
                  )
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update user's account type (admin only)
    #[tracing::instrument(skip(self), err)]
    pub async fn users_update_account_type(
        &self,
        id: i64,
        account_type: &AccountTypeSlug,
    ) -> AppResult<User> {
        sqlx::query("UPDATE users SET account_type = $1, token_version = token_version + 1, update_at = NOW() WHERE id = $2")
            .bind(account_type.as_str())
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.users_get_by_id(id).await
    }

    /// Update 2FA settings for a user
    #[tracing::instrument(skip(self), err)]
    pub async fn users_update_2fa_settings(
        &self,
        id: i64,
        enabled: bool,
        method: Option<&str>,
        totp_secret: Option<&str>,
        recovery_codes: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE users 
            SET two_factor_enabled = $1, 
                two_factor_method = $2,
                totp_secret = $3,
                recovery_codes = $4,
                token_version = token_version + 1,
                update_at = NOW()
            WHERE id = $5
            "#,
        )
        .bind(enabled)
        .bind(method)
        .bind(totp_secret)
        .bind(recovery_codes)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Mark a recovery code as used
    #[tracing::instrument(skip(self), err)]
    pub async fn users_mark_recovery_code_used(&self, id: i64, used_codes: &str) -> AppResult<()> {
        sqlx::query("UPDATE users SET recovery_codes_used = $1, update_at = NOW() WHERE id = $2")
            .bind(used_codes)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Fetch all active users with a non-empty email, optionally filtered by public_type.
    /// If `public_type` is None, all users with an email are returned (no filter).
    #[tracing::instrument(skip(self), err)]
    pub async fn users_get_emails_by_public_type(
        &self,
        public_type: Option<i64>,
    ) -> AppResult<Vec<UserEmailTarget>> {
        let rows = if let Some(pt) = public_type {
            sqlx::query_as::<_, UserEmailTarget>(
                r#"
                SELECT id, email, firstname, lastname, language
                FROM users
                WHERE email IS NOT NULL AND email <> ''
                  AND public_type = $1
                  AND (status IS NULL OR status <> 'deleted')
                "#,
            )
            .bind(pt)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, UserEmailTarget>(
                r#"
                SELECT id, email, firstname, lastname, language
                FROM users
                WHERE email IS NOT NULL AND email <> ''
                  AND (status IS NULL OR status <> 'deleted')
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows)
    }

    /// Patrons who should receive an event announcement, filtered and routed in SQL.
    ///
    /// `$1` is all-audiences: when true there is no public-type filter.
    /// Otherwise `public_types.name` must be in `$2`. The recipient's `events_consent_at`
    /// must be set. A `child` is never the recipient: each active child in the audience
    /// is attached to their guardian from `user_guardians`, and the guardian's consent
    /// is required. Rows are grouped so a guardian of several children, or a guardian
    /// who is also targeted directly, is returned once.
    #[tracing::instrument(skip(self, audience_names), err)]
    pub async fn users_list_announcement_recipients(
        &self,
        all_audiences: bool,
        audience_names: &[String],
    ) -> AppResult<Vec<AnnouncementRecipient>> {
        let rows = sqlx::query_as::<_, AnnouncementRecipientRow>(ANNOUNCEMENT_RECIPIENTS_SQL)
            .bind(all_audiences)
            .bind(audience_names)
            .fetch_all(&self.pool)
            .await?;
        Ok(group_announcement_recipients(rows))
    }

    pub async fn users_hold_ready_contact(
        &self,
        user_id: i64,
    ) -> AppResult<Option<HoldReadyUserContact>> {
        sqlx::query_as::<_, HoldReadyUserContact>(
            r#"SELECT email, firstname, lastname, language FROM users WHERE id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod announcement_recipient_sql_tests {
    use super::{
        group_announcement_recipients, AnnouncementRecipientRow, ANNOUNCEMENT_RECIPIENTS_SQL,
    };

    fn row(id: i64, child: Option<(i64, &str, &str)>) -> AnnouncementRecipientRow {
        let (child_id, child_firstname, child_lastname) = match child {
            Some((child_id, first, last)) => (
                Some(child_id),
                Some(first.to_string()),
                Some(last.to_string()),
            ),
            None => (None, None, None),
        };
        AnnouncementRecipientRow {
            id,
            email: Some(format!("user{id}@test.local")),
            firstname: Some(format!("Patron{id}")),
            lastname: Some("Test".into()),
            language: Some("french".into()),
            child_id,
            child_firstname,
            child_lastname,
            include_migration_notice: false,
        }
    }

    #[test]
    fn filters_consent_audience_and_guardian_routing_in_sql() {
        let sql = ANNOUNCEMENT_RECIPIENTS_SQL;
        assert!(
            sql.contains("u.events_consent_at IS NOT NULL"),
            "direct consent stays in the query"
        );
        assert!(
            sql.contains("g.events_consent_at IS NOT NULL"),
            "guardian consent is what counts for a child"
        );
        assert!(
            !sql.contains("c.events_consent_at"),
            "the child's own consent is not consulted"
        );
        assert!(
            !sql.contains("receive_reminders"),
            "overdue-reminder opt-in is not the announcement filter"
        );
        assert!(sql.contains("JOIN user_guardians ug ON ug.child_id = c.id"));
        assert!(sql.contains("cpt.name = 'child'"));
        assert!(
            sql.contains("pt.name IS DISTINCT FROM 'child'"),
            "a child is not a direct recipient"
        );
        assert!(sql.contains("(c.status IS NULL OR c.status <> 'deleted')"));
        assert!(sql.contains("(g.status IS NULL OR g.status <> 'deleted')"));
        assert!(sql.contains("g.email IS NOT NULL"));
        assert!(sql.contains("g.email <> ''"));
        assert!(sql.contains("gpt.name IS DISTINCT FROM 'child'"));
        assert!(sql.contains("gpt.name IS DISTINCT FROM 'school'"));
        assert!(sql.contains("$1::boolean OR pt.name = ANY($2::text[])"));
        assert!(sql.contains("$1::boolean OR cpt.name = ANY($2::text[])"));
        assert!(
            !sql.contains("Temporary until guardian routing"),
            "the provisional exclusion is replaced by guardian routing"
        );
    }

    #[test]
    fn direct_send_has_no_children() {
        let grouped = group_announcement_recipients(vec![row(1, None), row(2, None)]);
        assert_eq!(grouped.len(), 2);
        assert!(grouped
            .iter()
            .all(|recipient| recipient.children.is_empty()));
        assert_eq!(grouped[0].id, 1);
        assert_eq!(grouped[1].id, 2);
    }

    #[test]
    fn guardian_of_several_children_is_one_recipient() {
        let grouped = group_announcement_recipients(vec![
            row(10, Some((3, "Noe", "Martin"))),
            row(10, Some((2, "Lea", "Martin"))),
            row(10, Some((2, "Lea", "Martin"))),
        ]);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].id, 10);
        assert_eq!(grouped[0].email.as_deref(), Some("user10@test.local"));
        let names: Vec<_> = grouped[0]
            .children
            .iter()
            .map(|child| {
                (
                    child.id,
                    child.firstname.as_deref(),
                    child.lastname.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (2, Some("Lea"), Some("Martin")),
                (3, Some("Noe"), Some("Martin"))
            ]
        );
    }

    #[test]
    fn guardian_who_is_also_targeted_stays_one_recipient() {
        let grouped = group_announcement_recipients(vec![
            row(7, None),
            row(7, Some((4, "Ada", "Lovelace"))),
            row(8, None),
        ]);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].id, 7);
        assert_eq!(grouped[0].children.len(), 1);
        assert_eq!(grouped[0].children[0].id, 4);
        assert_eq!(grouped[0].children[0].firstname.as_deref(), Some("Ada"));
        assert!(grouped[1].children.is_empty());
    }
}
