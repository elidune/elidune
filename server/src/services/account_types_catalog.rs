//! Catalog service for `account_types` (roles and rights).

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::account_type::{AccountTypeDefinition, UpdateAccountTypeDefinition},
    repository::AccountTypesCatalogRepository,
};

#[derive(Clone)]
pub struct AccountTypesCatalogService {
    repository: Arc<dyn AccountTypesCatalogRepository>,
}

impl AccountTypesCatalogService {
    pub fn new(repository: Arc<dyn AccountTypesCatalogRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(&self) -> AppResult<Vec<AccountTypeDefinition>> {
        self.repository.account_types_list().await
    }

    pub async fn get_by_code(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        self.repository.account_types_get_by_code(code.trim()).await
    }

    /// Normalize and validate `data` in place (rights letters, name length).
    pub async fn update(&self, code: &str, data: &mut UpdateAccountTypeDefinition) -> AppResult<AccountTypeDefinition> {
        let code = code.trim();
        if code.is_empty() {
            return Err(AppError::Validation("code must not be empty".to_string()));
        }

        if let Some(ref mut name) = data.name {
            let t = name.trim();
            if t.is_empty() {
                return Err(AppError::Validation("name must not be empty when provided".to_string()));
            }
            if t.len() > 100 {
                return Err(AppError::Validation("name must be at most 100 characters".to_string()));
            }
            *name = t.to_string();
        }

        normalize_right_field(&mut data.items_rights)?;
        normalize_right_field(&mut data.users_rights)?;
        normalize_right_field(&mut data.loans_rights)?;
        normalize_right_field(&mut data.items_archive_rights)?;
        normalize_holds_right_field(&mut data.holds_rights)?;
        normalize_right_field(&mut data.settings_rights)?;
        normalize_right_field(&mut data.events_rights)?;

        self.repository.account_types_update(code, data).await
    }
}

fn normalize_right_field(field: &mut Option<String>) -> AppResult<()> {
    if let Some(raw) = field.as_ref() {
        let t = raw.trim();
        if t.len() != 1 {
            return Err(AppError::Validation(
                "each rights field must be exactly one character: n, r, or w".to_string(),
            ));
        }
        let c = t.chars().next().expect("length checked").to_ascii_lowercase();
        if !matches!(c, 'n' | 'r' | 'w') {
            return Err(AppError::Validation(
                "rights must be n (none), r (read), or w (write)".to_string(),
            ));
        }
        *field = Some(c.to_string());
    }
    Ok(())
}

fn normalize_holds_right_field(field: &mut Option<String>) -> AppResult<()> {
    if let Some(raw) = field.as_ref() {
        let t = raw.trim();
        if t.len() != 1 {
            return Err(AppError::Validation(
                "holds_rights must be exactly one character: n, o, r, or w".to_string(),
            ));
        }
        let c = t.chars().next().expect("length checked").to_ascii_lowercase();
        if !matches!(c, 'n' | 'o' | 'r' | 'w') {
            return Err(AppError::Validation(
                "holds_rights must be n (none), o (own holds), r (read), or w (write)".to_string(),
            ));
        }
        *field = Some(c.to_string());
    }
    Ok(())
}
