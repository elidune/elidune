//! `account_types` table — catalog of account roles and rights.

use async_trait::async_trait;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::account_type::{AccountTypeDefinition, UpdateAccountTypeDefinition},
};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AccountTypesCatalogRepository: Send + Sync {
    async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>>;
    async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition>;
    async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition>;
}

#[async_trait]
impl AccountTypesCatalogRepository for Repository {
    async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>> {
        Repository::account_types_list(self).await
    }
    async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        Repository::account_types_get_by_code(self, code).await
    }
    async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition> {
        Repository::account_types_update(self, code, data).await
    }
}

impl Repository {
    pub async fn account_types_list(&self) -> AppResult<Vec<AccountTypeDefinition>> {
        sqlx::query_as::<_, AccountTypeDefinition>(
            r#"
            SELECT code, name, items_rights, users_rights, loans_rights,
                   items_archive_rights, borrows_rights, settings_rights, events_rights
            FROM account_types
            ORDER BY code
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn account_types_get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        sqlx::query_as::<_, AccountTypeDefinition>(
            r#"
            SELECT code, name, items_rights, users_rights, loans_rights,
                   items_archive_rights, borrows_rights, settings_rights, events_rights
            FROM account_types
            WHERE code = $1
            "#,
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Account type '{code}' not found")))
    }

    /// Apply a partial update; at least one column must be set by the caller.
    pub async fn account_types_update(
        &self,
        code: &str,
        data: &UpdateAccountTypeDefinition,
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
        add_opt!(data.borrows_rights, "borrows_rights");
        add_opt!(data.settings_rights, "settings_rights");
        add_opt!(data.events_rights, "events_rights");

        if sets.is_empty() {
            return Err(AppError::Validation(
                "No fields to update".to_string(),
            ));
        }

        let q = format!(
            "UPDATE account_types SET {} WHERE code = ${} RETURNING code, name, items_rights, users_rights, loans_rights, \
             items_archive_rights, borrows_rights, settings_rights, events_rights",
            sets.join(", "),
            idx
        );

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
        bind_opt!(data.borrows_rights);
        bind_opt!(data.settings_rights);
        bind_opt!(data.events_rights);

        b = b.bind(code);

        b.fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Account type '{code}' not found")))
    }
}
