//! Public types service

use std::collections::HashSet;
use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::public_type::{
        CreatePublicType, PublicType, PublicTypeLoanSettingInput, PublicTypeLoanSettings,
        ReplacePublicTypeLoanSettingsRequest, UpdatePublicType,
    },
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
        self.repository.public_types_create(data).await
    }

    pub async fn update(&self, id: i64, data: &UpdatePublicType) -> AppResult<PublicType> {
        self.repository.public_types_update(id, data).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn delete(&self, id: i64) -> AppResult<()> {
        self.repository.public_types_delete(id).await
    }

    /// Full replace of loan settings for a public type; returns rows in the same order as GET.
    #[tracing::instrument(skip(self, body), err)]
    pub async fn update_loan_settings(
        &self,
        public_type_id: i64,
        body: &ReplacePublicTypeLoanSettingsRequest,
    ) -> AppResult<Vec<PublicTypeLoanSettings>> {
        validate_replace_public_type_loan_settings(&body.settings)?;
        self.repository
            .public_types_replace_loan_settings(public_type_id, &body.settings)
            .await
    }
}

fn validate_replace_public_type_loan_settings(rows: &[PublicTypeLoanSettingInput]) -> AppResult<()> {
    if rows.is_empty() {
        return Err(AppError::Validation(
            "loan settings: at least one row is required".into(),
        ));
    }

    let mut default_rows = 0usize;
    let mut seen_media = HashSet::<String>::new();

    for r in rows {
        match r
            .media_type
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            None => {
                default_rows += 1;
            }
            Some(mt) => {
                if !seen_media.insert(mt.to_string()) {
                    return Err(AppError::Validation(format!(
                        "loan settings: duplicate mediaType `{}`",
                        mt
                    )));
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
