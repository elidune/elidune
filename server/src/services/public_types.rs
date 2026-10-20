//! Public types service

use std::collections::HashSet;
use std::sync::Arc;

use crate::{
    error::{error_code, AppError, AppResult},
    models::public_type::{merge_nullable, CreatePublicType, PublicType, PublicTypeLoanSettingInput, PublicTypeLoanSettings, ReplacePublicTypeLoanSettingsRequest, UpdatePublicType},
    public_type_age::{age_band_range_inverted, conflicting_age_band, PublicTypeAgeBand},
    repository::PublicTypesRepository,
};

#[derive(Clone)]
pub struct PublicTypesService {
    repository: Arc<dyn PublicTypesRepository>,
}

impl PublicTypesService {
    pub fn new(repository: Arc<dyn PublicTypesRepository>) -> Self {
        Self { repository }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list(&self) -> AppResult<Vec<PublicType>> {
        self.repository.public_types_list().await
    }

    pub async fn get_by_id(&self, id: i64) -> AppResult<PublicType> {
        self.repository.public_types_get_by_id(id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_loan_settings(&self, public_type_id: i64) -> AppResult<Vec<PublicTypeLoanSettings>> {
        self.repository.public_types_get_loan_settings(public_type_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn create(&self, data: &CreatePublicType) -> AppResult<PublicType> {
        super::holds::validate_max_active_holds(data.max_active_holds)?;
        self.ensure_age_band_can_be_saved(None, data.name.as_str(), data.age_min, data.age_max).await?;
        self.repository.public_types_create(data).await
    }

    pub async fn update(&self, id: i64, data: &UpdatePublicType) -> AppResult<PublicType> {
        super::holds::validate_max_active_holds(data.max_active_holds.flatten())?;
        let existing = self.repository.public_types_get_by_id(id).await?;
        let age_min = merge_nullable(data.age_min, existing.age_min);
        let age_max = merge_nullable(data.age_max, existing.age_max);
        let name = data.name.as_deref().unwrap_or(existing.name.as_str());
        self.ensure_age_band_can_be_saved(Some(id), name, age_min, age_max).await?;
        self.repository.public_types_update(id, data).await
    }

    /// Reject an inverted range or an overlap with another age band.
    ///
    /// Bands that are not part of this save are not revalidated, so a legacy overlap
    /// does not block creating or updating an unrelated type. Startup and reads stay valid.
    async fn ensure_age_band_can_be_saved(&self, id: Option<i64>, name: &str, age_min: Option<i16>, age_max: Option<i16>) -> AppResult<()> {
        if age_band_range_inverted(age_min, age_max) {
            return Err(AppError::coded(422, error_code::PUBLIC_TYPE_AGE_RANGE_INVALID, "ageMin must be less than or equal to ageMax"));
        }
        let existing = self.repository.public_types_list().await?;
        let others: Vec<PublicTypeAgeBand> = existing.iter().map(band_from_public_type).collect();
        let candidate = PublicTypeAgeBand {
            id: id.unwrap_or(i64::MAX),
            name: name.to_string(),
            label: name.to_string(),
            age_min,
            age_max,
        };
        if let Some(conflict) = conflicting_age_band(&candidate, &others) {
            return Err(AppError::coded(
                422,
                error_code::PUBLIC_TYPE_AGE_OVERLAP,
                format!(
                    "Age bands overlap: {} ({}) and {} ({}). Adjust the ranges so they do not overlap.",
                    candidate.name,
                    candidate.range_label(),
                    conflict.name,
                    conflict.range_label()
                ),
            ));
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn delete(&self, id: i64) -> AppResult<()> {
        self.repository.public_types_delete(id).await
    }

    /// Full replace of loan settings for a public type; returns rows in the same order as GET.
    #[tracing::instrument(skip(self, body), err)]
    pub async fn update_loan_settings(&self, public_type_id: i64, body: &ReplacePublicTypeLoanSettingsRequest) -> AppResult<Vec<PublicTypeLoanSettings>> {
        validate_replace_public_type_loan_settings(&body.settings)?;
        self.repository.public_types_replace_loan_settings(public_type_id, &body.settings).await
    }
}

fn band_from_public_type(public_type: &PublicType) -> PublicTypeAgeBand {
    PublicTypeAgeBand {
        id: public_type.id,
        name: public_type.name.clone(),
        label: public_type.label.clone(),
        age_min: public_type.age_min,
        age_max: public_type.age_max,
    }
}

fn validate_replace_public_type_loan_settings(rows: &[PublicTypeLoanSettingInput]) -> AppResult<()> {
    if rows.is_empty() {
        return Err(AppError::Validation("loan settings: at least one row is required".into()));
    }

    let mut default_rows = 0usize;
    let mut seen_media = HashSet::<String>::new();

    for r in rows {
        match r.media_type.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => {
                default_rows += 1;
            }
            Some(mt) => {
                if !seen_media.insert(mt.to_string()) {
                    return Err(AppError::Validation(format!("loan settings: duplicate mediaType `{}`", mt)));
                }
            }
        }
    }

    if default_rows != 1 {
        return Err(AppError::Validation(format!(
            "loan settings: exactly one audience-default row is required (mediaType null or omitted); found {}",
            default_rows
        )));
    }

    Ok(())
}
