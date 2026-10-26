//! Sites service

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::site::{CreateSite, MergeSites, Site, UpdateSite},
    repository::SitesRepository,
};

#[derive(Clone)]
pub struct SitesService {
    repository: Arc<dyn SitesRepository>,
}

impl SitesService {
    pub fn new(repository: Arc<dyn SitesRepository>) -> Self {
        Self { repository }
    }

    /// List sources
    pub async fn list(&self, include_archived: bool) -> AppResult<Vec<Site>> {
        self.repository.sites_list(include_archived).await
    }

    /// Get source by ID
    pub async fn get_by_id(&self, id: i64) -> AppResult<Site> {
        self.repository.sites_get_by_id(id).await
    }

    /// Create a source
    pub async fn create(&self, data: &CreateSite) -> AppResult<Site> {
        let name = data.name.trim();
        if name.is_empty() {
            return Err(AppError::Validation(
                "Site name cannot be empty".to_string(),
            ));
        }
        self.repository.sites_create(name, data.default).await
    }

    /// Rename a source
    pub async fn rename(&self, id: i64, name: &str) -> AppResult<Site> {
        if name.trim().is_empty() {
            return Err(AppError::Validation(
                "Site name cannot be empty".to_string(),
            ));
        }
        self.repository.sites_rename(id, name.trim()).await
    }

    /// Update a source (name and/or default status)
    pub async fn update(&self, id: i64, data: &UpdateSite) -> AppResult<Site> {
        // Validate name if provided
        if let Some(ref name) = data.name {
            if name.trim().is_empty() {
                return Err(AppError::Validation(
                    "Site name cannot be empty".to_string(),
                ));
            }
        }

        self.repository
            .sites_update(id, data.name.as_deref(), data.default)
            .await
    }

    /// Archive a source (fails if non-archived items are linked)
    pub async fn archive(&self, id: i64) -> AppResult<Site> {
        // Verify source exists
        let source = self.repository.sites_get_by_id(id).await?;

        // Check if already archived
        if source.is_archive == Some(1) {
            return Err(AppError::BusinessRule(
                "Site is already archived".to_string(),
            ));
        }

        // Check for non-archived items
        let active_count = self.repository.sites_count_active_items(id).await?;
        if active_count > 0 {
            return Err(AppError::BusinessRule(format!(
                "Cannot archive site: {} non-archived item(s) still linked",
                active_count
            )));
        }

        self.repository.sites_archive(id).await
    }

    /// Merge multiple sources into a new one.
    ///
    /// All three writes (create, reassign, archive) run inside a single transaction so
    /// a failure cannot leave items pointing at a non-existent or wrong source.
    pub async fn merge(&self, data: &MergeSites) -> AppResult<Site> {
        if data.name.trim().is_empty() {
            return Err(AppError::Validation(
                "Merged site name cannot be empty".to_string(),
            ));
        }
        if data.site_ids.len() < 2 {
            return Err(AppError::Validation(
                "At least 2 site IDs are required for merge".to_string(),
            ));
        }

        // Verify all source IDs exist before opening the transaction.
        for &id in &data.site_ids {
            self.repository.sites_get_by_id(id).await?;
        }

        let name = data.name.trim();
        let old_ids = &data.site_ids;

        self.repository.merge_sites_tx(name, old_ids).await
    }
}
