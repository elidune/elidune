//! LLM provider catalog (admin CRUD + public listing).

use std::sync::Arc;

use crate::{
    config::ChatConfig,
    error::{AppError, AppResult},
    models::chat::{CreateLlmProviderRequest, LlmProviderAdmin, LlmProviderKind, LlmProviderPublic, LlmProviderRow, UpdateLlmProviderRequest},
    repository::LlmProvidersRepository,
    services::chat::{build_backend, models_from_json},
};

#[derive(Clone)]
pub struct LlmProvidersService {
    repo: Arc<dyn LlmProvidersRepository>,
}

impl LlmProvidersService {
    pub fn new(repo: Arc<dyn LlmProvidersRepository>) -> Self {
        Self { repo }
    }

    pub fn repository(&self) -> Arc<dyn LlmProvidersRepository> {
        self.repo.clone()
    }

    pub async fn list_public(&self) -> AppResult<Vec<LlmProviderPublic>> {
        let rows = self.repo.llm_providers_list_enabled().await?;
        Ok(rows.into_iter().map(row_to_public).collect())
    }

    pub async fn list_admin(&self) -> AppResult<Vec<LlmProviderAdmin>> {
        let rows = self.repo.llm_providers_list_all().await?;
        Ok(rows.into_iter().map(row_to_admin).collect())
    }

    pub async fn get_row(&self, id: i64) -> AppResult<LlmProviderRow> {
        self.repo.llm_providers_get(id).await
    }

    pub async fn create(&self, req: CreateLlmProviderRequest) -> AppResult<LlmProviderAdmin> {
        validate_provider_request(&req.slug, &req.base_url, &req.models)?;
        let row = self.repo.llm_providers_create(&req).await?;
        Ok(row_to_admin(row))
    }

    pub async fn update(&self, id: i64, req: UpdateLlmProviderRequest) -> AppResult<LlmProviderAdmin> {
        if let Some(ref slug) = req.slug {
            if slug.trim().is_empty() {
                return Err(AppError::Validation("slug must not be empty".into()));
            }
        }
        if let Some(ref models) = req.models {
            if models.is_empty() {
                return Err(AppError::Validation("models must not be empty".into()));
            }
        }
        let clear_key = req.api_key.as_ref().is_some_and(|s| s.is_empty());
        let row = self.repo.llm_providers_update(id, &req, clear_key).await?;
        Ok(row_to_admin(row))
    }

    pub async fn delete(&self, id: i64) -> AppResult<()> {
        self.repo.llm_providers_delete(id).await
    }

    pub async fn test(&self, id: i64, chat_cfg: &ChatConfig) -> AppResult<()> {
        let row = self.repo.llm_providers_get(id).await?;
        let model = row
            .default_model
            .clone()
            .or_else(|| models_from_json(&row.models).into_iter().next())
            .ok_or_else(|| AppError::Validation("Provider has no model configured".into()))?;
        let backend = build_backend(&row, chat_cfg.request_timeout_secs)?;
        backend.test_connection(&model).await
    }

    pub async fn seed_from_config(&self, chat_cfg: &ChatConfig) -> AppResult<()> {
        for (i, p) in chat_cfg.providers.iter().enumerate() {
            self.repo
                .llm_providers_seed_if_missing(
                    &p.slug,
                    &p.label,
                    &p.kind,
                    &p.base_url,
                    p.api_key.as_deref(),
                    p.api_key_env.as_deref(),
                    &p.models,
                    p.default_model.as_deref(),
                    p.enabled,
                    if p.sort_order != 0 { p.sort_order } else { i as i32 },
                )
                .await?;
        }
        Ok(())
    }

    pub fn resolve_model(row: &LlmProviderRow, requested: Option<&str>) -> AppResult<String> {
        if let Some(m) = requested {
            let models = models_from_json(&row.models);
            if models.is_empty() || models.iter().any(|x| x == m) {
                return Ok(m.to_string());
            }
            return Err(AppError::Validation(format!("Model '{m}' is not allowed for this provider")));
        }
        row.default_model
            .clone()
            .or_else(|| models_from_json(&row.models).into_iter().next())
            .ok_or_else(|| AppError::Validation("No model configured for provider".into()))
    }
}

fn validate_provider_request(slug: &str, base_url: &str, models: &[String]) -> AppResult<()> {
    if slug.trim().is_empty() {
        return Err(AppError::Validation("slug is required".into()));
    }
    if base_url.trim().is_empty() {
        return Err(AppError::Validation("baseUrl is required".into()));
    }
    if models.is_empty() {
        return Err(AppError::Validation("at least one model is required".into()));
    }
    Ok(())
}

fn row_to_public(row: LlmProviderRow) -> LlmProviderPublic {
    LlmProviderPublic {
        id: row.id,
        slug: row.slug,
        label: row.label,
        kind: LlmProviderKind::from_db(&row.kind).unwrap_or(LlmProviderKind::OpenaiCompat),
        models: models_from_json(&row.models),
        default_model: row.default_model,
    }
}

fn row_to_admin(row: LlmProviderRow) -> LlmProviderAdmin {
    let api_key_set = super::chat::resolve_api_key(&row).is_some();
    LlmProviderAdmin {
        id: row.id,
        slug: row.slug,
        label: row.label,
        kind: LlmProviderKind::from_db(&row.kind).unwrap_or(LlmProviderKind::OpenaiCompat),
        base_url: row.base_url,
        api_key_set,
        api_key_env: row.api_key_env,
        models: models_from_json(&row.models),
        default_model: row.default_model,
        enabled: row.enabled,
        sort_order: row.sort_order,
    }
}

pub fn row_to_public_ref(row: &LlmProviderRow) -> LlmProviderPublic {
    row_to_public(row.clone())
}
