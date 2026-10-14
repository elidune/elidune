//! `account_types` table — catalog of account roles and rights.

use async_trait::async_trait;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::account_type::{AccountTypeDefinition, UpdateAccountTypeDefinition},
};

const ACCOUNT_TYPE_COLUMNS: &str = "code, name, items_rights, users_rights, loans_rights, \
     items_archive_rights, holds_rights, settings_rights, events_rights, \
     acquisitions_rights, weeding_rights, weeding_cancel_rights, \
     circulation_override_rights, is_system";

/// Hypothetical change used to count active accounts that would still have admin rights.
///
/// Admin rights are `settings_rights = w`, the same bar as `UserClaims::require_write_settings`.
#[derive(Debug, Clone, Default)]
pub struct ActiveAdminForecast {
    /// Drop this user from the active set (deactivation or erasure).
    pub exclude_user_id: Option<i64>,
    /// Move this user onto `move_to_type`.
    pub move_user_id: Option<i64>,
    pub move_to_type: Option<String>,
    /// Replace `settings_rights` for every user of this type.
    pub type_code: Option<String>,
    pub type_settings_rights: Option<String>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AccountTypesCatalogRepository: Send + Sync {
    async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>>;
    async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition>;
    async fn account_types_create(
        &self,
        row: &AccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition>;
    async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
        revoke_sessions: bool,
    ) -> AppResult<AccountTypeDefinition>;
    async fn account_types_delete(&self, code: &str) -> AppResult<()>;
    async fn account_types_count_users(&self, code: &str) -> AppResult<i64>;
    async fn guard_active_admins(&self, forecast: &ActiveAdminForecast) -> AppResult<()>;
}

#[async_trait]
impl AccountTypesCatalogRepository for Repository {
    async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>> {
        Repository::account_types_list(self).await
    }
    async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        Repository::account_types_get_by_code(self, code).await
    }
    async fn account_types_create(
        &self,
        row: &AccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition> {
        Repository::account_types_create(self, row).await
    }
    async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
        revoke_sessions: bool,
    ) -> AppResult<AccountTypeDefinition> {
        Repository::account_types_update(self, code, data, revoke_sessions).await
    }
    async fn account_types_delete(&self, code: &str) -> AppResult<()> {
        Repository::account_types_delete(self, code).await
    }
    async fn account_types_count_users(&self, code: &str) -> AppResult<i64> {
        Repository::account_types_count_users(self, code).await
    }
    async fn guard_active_admins(&self, forecast: &ActiveAdminForecast) -> AppResult<()> {
        Repository::guard_active_admins(self, forecast).await
    }
}

fn map_account_type_sql(err: sqlx::Error) -> AppError {
    let sql_state = match &err {
        sqlx::Error::Database(db) => db.code().map(|code| code.into_owned()),
        _ => None,
    };
    match sql_state.as_deref() {
        Some("23503") => AppError::account_type_in_use(),
        Some("23505") => AppError::Conflict("Account type code already exists".to_string()),
        _ => AppError::from(err),
    }
}

impl Repository {
    pub async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>> {
        let sql = format!("SELECT {ACCOUNT_TYPE_COLUMNS} FROM account_types ORDER BY code");
        sqlx::query_as::<_, AccountTypeDefinition>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        let sql = format!("SELECT {ACCOUNT_TYPE_COLUMNS} FROM account_types WHERE code = $1");
        sqlx::query_as::<_, AccountTypeDefinition>(&sql)
            .bind(code)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Account type '{code}' not found")))
    }

    pub async fn account_types_create(
        &self,
        row: &AccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition> {
        let sql = format!(
            "INSERT INTO account_types ({ACCOUNT_TYPE_COLUMNS}) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, FALSE) \
             RETURNING {ACCOUNT_TYPE_COLUMNS}"
        );
        sqlx::query_as::<_, AccountTypeDefinition>(&sql)
            .bind(&row.code)
            .bind(&row.name)
            .bind(&row.items_rights)
            .bind(&row.users_rights)
            .bind(&row.loans_rights)
            .bind(&row.items_archive_rights)
            .bind(&row.holds_rights)
            .bind(&row.settings_rights)
            .bind(&row.events_rights)
            .bind(&row.acquisitions_rights)
            .bind(&row.weeding_rights)
            .bind(&row.weeding_cancel_rights)
            .bind(&row.circulation_override_rights)
            .fetch_one(&self.pool)
            .await
            .map_err(map_account_type_sql)
    }

    /// Apply a partial update. When `revoke_sessions` is set, open sessions of
    /// accounts of this type are invalidated in the same transaction.
    pub async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
        revoke_sessions: bool,
    ) -> AppResult<AccountTypeDefinition> {
        let mut sets = Vec::new();
        let mut idx: usize = 1;

        macro_rules! add_opt {
            ($field:expr, $col:expr) => {
                if $field.is_some() {
                    sets.push(format!("{} = ${}", $col, idx));
                    idx += 1;
                }
            };
        }

        add_opt!(data.name, "name");
        add_opt!(data.items_rights, "items_rights");
        add_opt!(data.users_rights, "users_rights");
        add_opt!(data.loans_rights, "loans_rights");
        add_opt!(data.items_archive_rights, "items_archive_rights");
        add_opt!(data.holds_rights, "holds_rights");
        add_opt!(data.settings_rights, "settings_rights");
        add_opt!(data.events_rights, "events_rights");
        add_opt!(data.acquisitions_rights, "acquisitions_rights");
        add_opt!(data.weeding_rights, "weeding_rights");
        add_opt!(data.weeding_cancel_rights, "weeding_cancel_rights");
        add_opt!(
            data.circulation_override_rights,
            "circulation_override_rights"
        );

        if sets.is_empty() {
            return Err(AppError::Validation("No fields to update".to_string()));
        }

        let q = format!(
            "UPDATE account_types SET {} WHERE code = ${} RETURNING {ACCOUNT_TYPE_COLUMNS}",
            sets.join(", "),
            idx
        );

        let mut tx = self.pool.begin().await?;
        let mut b = sqlx::query_as::<_, AccountTypeDefinition>(&q);

        macro_rules! bind_opt {
            ($field:expr) => {
                if let Some(ref v) = $field {
                    b = b.bind(v);
                }
            };
        }

        bind_opt!(data.name);
        bind_opt!(data.items_rights);
        bind_opt!(data.users_rights);
        bind_opt!(data.loans_rights);
        bind_opt!(data.items_archive_rights);
        bind_opt!(data.holds_rights);
        bind_opt!(data.settings_rights);
        bind_opt!(data.events_rights);
        bind_opt!(data.acquisitions_rights);
        bind_opt!(data.weeding_rights);
        bind_opt!(data.weeding_cancel_rights);
        bind_opt!(data.circulation_override_rights);

        b = b.bind(code);

        let updated = b
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_account_type_sql)?
            .ok_or_else(|| AppError::NotFound(format!("Account type '{code}' not found")))?;

        if revoke_sessions {
            sqlx::query(
                "UPDATE users SET token_version = token_version + 1, update_at = NOW() WHERE account_type = $1",
            )
            .bind(code)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(updated)
    }

    pub async fn account_types_delete(&self, code: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM account_types WHERE code = $1")
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(map_account_type_sql)?;
        Ok(())
    }

    pub async fn account_types_count_users(&self, code: &str) -> AppResult<i64> {
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM users WHERE account_type = $1")
            .bind(code)
            .fetch_one(&self.pool)
            .await
            .map_err(Into::into)
    }

    /// Reject the forecast when it would remove the last active admin.
    ///
    /// An active admin is an account with status `active`, no `archived_at`,
    /// whose type has settings write. A change that starts from zero admins
    /// is left alone so the library can grant settings write again.
    pub async fn guard_active_admins(&self, forecast: &ActiveAdminForecast) -> AppResult<()> {
        let before = self
            .count_active_admins(&ActiveAdminForecast::default())
            .await?;
        let after = self.count_active_admins(forecast).await?;
        if before > 0 && after == 0 {
            return Err(AppError::last_active_admin());
        }
        Ok(())
    }

    async fn count_active_admins(&self, forecast: &ActiveAdminForecast) -> AppResult<i64> {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::bigint
            FROM users u
            LEFT JOIN account_types t ON t.code = u.account_type
            WHERE u.archived_at IS NULL
              AND lower(COALESCE(u.status, 'active')) = 'active'
              AND ($1::bigint IS NULL OR u.id <> $1)
              AND CASE
                    WHEN $2::bigint IS NOT NULL AND u.id = $2 THEN
                      COALESCE((
                        SELECT lower(nt.settings_rights) = 'w'
                        FROM account_types nt
                        WHERE nt.code = $3
                      ), FALSE)
                    WHEN $4::text IS NOT NULL AND u.account_type = $4 THEN
                      lower(COALESCE($5::text, '')) = 'w'
                    ELSE lower(COALESCE(t.settings_rights, '')) = 'w'
                  END
            "#,
        )
        .bind(forecast.exclude_user_id)
        .bind(forecast.move_user_id)
        .bind(forecast.move_to_type.as_deref())
        .bind(forecast.type_code.as_deref())
        .bind(forecast.type_settings_rights.as_deref())
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }
}
