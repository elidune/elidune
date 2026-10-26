//! Catalog service for `account_types` (roles and rights).

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::{
        account_type::{AccountTypeDefinition, CreateAccountType, UpdateAccountTypeDefinition},
        user::Rights,
    },
    repository::{account_types::ActiveAdminForecast, AccountTypesCatalogRepository},
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

    pub async fn create(&self, data: &mut CreateAccountType) -> AppResult<AccountTypeDefinition> {
        let code = normalize_code(&data.code)?;
        let name = normalize_name(&data.name)?;
        let row = AccountTypeDefinition {
            code,
            name: Some(name),
            items_rights: Some(right_or_none(&mut data.items_rights, false)?),
            users_rights: Some(right_or_none(&mut data.users_rights, false)?),
            loans_rights: Some(right_or_none(&mut data.loans_rights, false)?),
            items_archive_rights: Some(right_or_none(&mut data.items_archive_rights, false)?),
            holds_rights: Some(right_or_none(&mut data.holds_rights, true)?),
            settings_rights: Some(right_or_none(&mut data.settings_rights, false)?),
            events_rights: Some(right_or_none(&mut data.events_rights, false)?),
            acquisitions_rights: Some(right_or_none(&mut data.acquisitions_rights, false)?),
            weeding_rights: Some(right_or_none(&mut data.weeding_rights, false)?),
            weeding_cancel_rights: Some(right_or_none(&mut data.weeding_cancel_rights, false)?),
            circulation_override_rights: Some(right_or_none(
                &mut data.circulation_override_rights,
                false,
            )?),
            is_system: false,
        };
        self.repository.account_types_create(&row).await
    }

    /// Normalize and validate `data` in place (rights letters, name length).
    ///
    /// Removing a right invalidates open sessions of accounts of this type.
    /// Adding a right leaves those sessions in place.
    pub async fn update(
        &self,
        code: &str,
        data: &mut UpdateAccountTypeDefinition,
    ) -> AppResult<AccountTypeDefinition> {
        let code = code.trim();
        if code.is_empty() {
            return Err(AppError::Validation("code must not be empty".to_string()));
        }

        if let Some(ref mut name) = data.name {
            *name = normalize_name(name)?;
        }

        normalize_right_field(&mut data.items_rights)?;
        normalize_right_field(&mut data.users_rights)?;
        normalize_right_field(&mut data.loans_rights)?;
        normalize_right_field(&mut data.items_archive_rights)?;
        normalize_holds_right_field(&mut data.holds_rights)?;
        normalize_right_field(&mut data.settings_rights)?;
        normalize_right_field(&mut data.events_rights)?;
        normalize_right_field(&mut data.acquisitions_rights)?;
        normalize_right_field(&mut data.weeding_rights)?;
        normalize_right_field(&mut data.weeding_cancel_rights)?;
        normalize_right_field(&mut data.circulation_override_rights)?;

        let before = self.repository.account_types_get_by_code(code).await?;
        if let Some(ref settings) = data.settings_rights {
            self.repository
                .guard_active_admins(&ActiveAdminForecast {
                    type_code: Some(before.code.clone()),
                    type_settings_rights: Some(settings.clone()),
                    ..ActiveAdminForecast::default()
                })
                .await?;
        }

        let revoke_sessions = removes_any_right(&before, data);
        self.repository
            .account_types_update(code, data, revoke_sessions)
            .await
    }

    /// Delete a custom type. System types and types still assigned to a user are refused.
    /// Deleting the only source of admin rights is refused even when accounts still use it.
    pub async fn delete(&self, code: &str) -> AppResult<AccountTypeDefinition> {
        let existing = self.get_by_code(code).await?;
        if existing.is_system {
            return Err(AppError::account_type_system());
        }
        self.repository
            .guard_active_admins(&ActiveAdminForecast {
                type_code: Some(existing.code.clone()),
                type_settings_rights: Some("n".to_string()),
                ..ActiveAdminForecast::default()
            })
            .await?;
        let assigned = self
            .repository
            .account_types_count_users(&existing.code)
            .await?;
        if assigned > 0 {
            return Err(AppError::account_type_in_use());
        }
        self.repository.account_types_delete(&existing.code).await?;
        Ok(existing)
    }
}

fn normalize_code(raw: &str) -> AppResult<String> {
    let code = raw.trim().to_ascii_lowercase();
    if !(1..=50).contains(&code.len()) {
        return Err(AppError::Validation(
            "code must be 1 to 50 characters".to_string(),
        ));
    }
    let mut chars = code.chars();
    let Some(first) = chars.next() else {
        return Err(AppError::Validation(
            "code must be 1 to 50 characters".to_string(),
        ));
    };
    if !first.is_ascii_lowercase()
        || chars.any(|c| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_')
    {
        return Err(AppError::Validation(
            "code must start with a letter and contain only letters, digits, and underscores"
                .to_string(),
        ));
    }
    Ok(code)
}

fn normalize_name(raw: &str) -> AppResult<String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError::Validation(
            "name must not be empty when provided".to_string(),
        ));
    }
    if name.len() > 100 {
        return Err(AppError::Validation(
            "name must be at most 100 characters".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn right_or_none(field: &mut Option<String>, holds: bool) -> AppResult<String> {
    if field.is_none() {
        *field = Some("n".to_string());
    }
    if holds {
        normalize_holds_right_field(field)?;
    } else {
        normalize_right_field(field)?;
    }
    field.clone().ok_or_else(|| {
        AppError::Validation("each rights field must be exactly one character".to_string())
    })
}

fn normalize_right_field(field: &mut Option<String>) -> AppResult<()> {
    if let Some(raw) = field.as_ref() {
        let t = raw.trim();
        if t.len() != 1 {
            return Err(AppError::Validation(
                "each rights field must be exactly one character: n, o, r, or w".to_string(),
            ));
        }
        let c = t
            .chars()
            .next()
            .ok_or_else(|| AppError::Validation("each rights field must be one character".into()))?
            .to_ascii_lowercase();
        if !matches!(c, 'n' | 'o' | 'r' | 'w') {
            return Err(AppError::Validation(
                "rights must be n (none), o (own), r (read), or w (write)".to_string(),
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
        let c = t
            .chars()
            .next()
            .ok_or_else(|| AppError::Validation("holds_rights must be one character".into()))?
            .to_ascii_lowercase();
        if !matches!(c, 'n' | 'o' | 'r' | 'w') {
            return Err(AppError::Validation(
                "holds_rights must be n (none), o (own holds), r (read), or w (write)".to_string(),
            ));
        }
        *field = Some(c.to_string());
    }
    Ok(())
}

fn patch_removes_right(current: &Option<String>, patch: &Option<String>) -> bool {
    let Some(next) = patch else {
        return false;
    };
    Rights::from(Some(next.clone())).rank() < Rights::from(current.clone()).rank()
}

fn removes_any_right(before: &AccountTypeDefinition, patch: &UpdateAccountTypeDefinition) -> bool {
    patch_removes_right(&before.items_rights, &patch.items_rights)
        || patch_removes_right(&before.users_rights, &patch.users_rights)
        || patch_removes_right(&before.loans_rights, &patch.loans_rights)
        || patch_removes_right(&before.items_archive_rights, &patch.items_archive_rights)
        || patch_removes_right(&before.holds_rights, &patch.holds_rights)
        || patch_removes_right(&before.settings_rights, &patch.settings_rights)
        || patch_removes_right(&before.events_rights, &patch.events_rights)
        || patch_removes_right(&before.acquisitions_rights, &patch.acquisitions_rights)
        || patch_removes_right(&before.weeding_rights, &patch.weeding_rights)
        || patch_removes_right(&before.weeding_cancel_rights, &patch.weeding_cancel_rights)
        || patch_removes_right(
            &before.circulation_override_rights,
            &patch.circulation_override_rights,
        )
}

#[cfg(test)]
mod tests {
    use super::{normalize_right_field, patch_removes_right};

    #[test]
    fn users_and_loans_rights_accept_own() {
        let mut own = Some("O".into());
        normalize_right_field(&mut own).expect("o is valid");
        assert_eq!(own.as_deref(), Some("o"));
        let mut bad = Some("x".into());
        assert!(normalize_right_field(&mut bad).is_err());
    }

    #[test]
    fn lowering_a_right_is_a_removal_and_raising_one_is_not() {
        assert!(patch_removes_right(&Some("w".into()), &Some("r".into())));
        assert!(!patch_removes_right(&Some("n".into()), &Some("r".into())));
        assert!(!patch_removes_right(&Some("r".into()), &None));
    }
}
