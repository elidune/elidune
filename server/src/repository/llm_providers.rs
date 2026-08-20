//! `llm_providers` table — configured LLM backends.

use async_trait::async_trait;
use serde_json::json;
use snowflaked::Generator;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::chat::{CreateLlmProviderRequest, LlmProviderRow, UpdateLlmProviderRequest},
};

#[async_trait]
pub trait LlmProvidersRepository: Send + Sync {
    async fn llm_providers_list_all(&self) -> AppResult<Vec<LlmProviderRow>>;
    async fn llm_providers_list_enabled(&self) -> AppResult<Vec<LlmProviderRow>>;
    async fn llm_providers_get(&self, id: i64) -> AppResult<LlmProviderRow>;
    async fn llm_providers_get_by_slug(&self, slug: &str) -> AppResult<Option<LlmProviderRow>>;
    async fn llm_providers_create(&self, req: &CreateLlmProviderRequest) -> AppResult<LlmProviderRow>;
    async fn llm_providers_update(&self, id: i64, req: &UpdateLlmProviderRequest, clear_api_key: bool) -> AppResult<LlmProviderRow>;
    async fn llm_providers_delete(&self, id: i64) -> AppResult<()>;
    async fn llm_providers_seed_if_missing(
        &self,
        slug: &str,
        label: &str,
        kind: &str,
        base_url: &str,
        api_key: Option<&str>,
        api_key_env: Option<&str>,
        models: &[String],
        default_model: Option<&str>,
        enabled: bool,
        sort_order: i32,
    ) -> AppResult<()>;
}

#[async_trait]
impl LlmProvidersRepository for Repository {
    async fn llm_providers_list_all(&self) -> AppResult<Vec<LlmProviderRow>> {
        Repository::llm_providers_list_all(self).await
    }
    async fn llm_providers_list_enabled(&self) -> AppResult<Vec<LlmProviderRow>> {
        Repository::llm_providers_list_enabled(self).await
    }
    async fn llm_providers_get(&self, id: i64) -> AppResult<LlmProviderRow> {
        Repository::llm_providers_get(self, id).await
    }
    async fn llm_providers_get_by_slug(&self, slug: &str) -> AppResult<Option<LlmProviderRow>> {
        Repository::llm_providers_get_by_slug(self, slug).await
    }
    async fn llm_providers_create(&self, req: &CreateLlmProviderRequest) -> AppResult<LlmProviderRow> {
        Repository::llm_providers_create(self, req).await
    }
    async fn llm_providers_update(&self, id: i64, req: &UpdateLlmProviderRequest, clear_api_key: bool) -> AppResult<LlmProviderRow> {
        Repository::llm_providers_update(self, id, req, clear_api_key).await
    }
    async fn llm_providers_delete(&self, id: i64) -> AppResult<()> {
        Repository::llm_providers_delete(self, id).await
    }
    async fn llm_providers_seed_if_missing(
        &self,
        slug: &str,
        label: &str,
        kind: &str,
        base_url: &str,
        api_key: Option<&str>,
        api_key_env: Option<&str>,
        models: &[String],
        default_model: Option<&str>,
        enabled: bool,
        sort_order: i32,
    ) -> AppResult<()> {
        Repository::llm_providers_seed_if_missing(self, slug, label, kind, base_url, api_key, api_key_env, models, default_model, enabled, sort_order).await
    }
}

impl Repository {
    pub async fn llm_providers_list_all(&self) -> AppResult<Vec<LlmProviderRow>> {
        sqlx::query_as::<_, LlmProviderRow>(
            r#"
            SELECT id, slug, label, kind, base_url, api_key, api_key_env,
                   models, default_model, enabled, sort_order, created_at, updated_at
            FROM llm_providers
            ORDER BY sort_order, label
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn llm_providers_list_enabled(&self) -> AppResult<Vec<LlmProviderRow>> {
        sqlx::query_as::<_, LlmProviderRow>(
            r#"
            SELECT id, slug, label, kind, base_url, api_key, api_key_env,
                   models, default_model, enabled, sort_order, created_at, updated_at
            FROM llm_providers
            WHERE enabled = true
            ORDER BY sort_order, label
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn llm_providers_get(&self, id: i64) -> AppResult<LlmProviderRow> {
        sqlx::query_as::<_, LlmProviderRow>(
            r#"
            SELECT id, slug, label, kind, base_url, api_key, api_key_env,
                   models, default_model, enabled, sort_order, created_at, updated_at
            FROM llm_providers
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("LLM provider {id} not found")))
    }

    pub async fn llm_providers_get_by_slug(&self, slug: &str) -> AppResult<Option<LlmProviderRow>> {
        sqlx::query_as::<_, LlmProviderRow>(
            r#"
            SELECT id, slug, label, kind, base_url, api_key, api_key_env,
                   models, default_model, enabled, sort_order, created_at, updated_at
            FROM llm_providers
            WHERE slug = $1
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn llm_providers_create(&self, req: &CreateLlmProviderRequest) -> AppResult<LlmProviderRow> {
        let id: i64 = Generator::new(3).generate();
        let models_json = json!(req.models);
        sqlx::query_as::<_, LlmProviderRow>(
            r#"
            INSERT INTO llm_providers
                (id, slug, label, kind, base_url, api_key, api_key_env, models, default_model, enabled, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, slug, label, kind, base_url, api_key, api_key_env,
                      models, default_model, enabled, sort_order, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(req.slug.trim())
        .bind(req.label.trim())
        .bind(req.kind.as_str())
        .bind(req.base_url.trim())
        .bind(req.api_key.as_deref().filter(|s| !s.is_empty()))
        .bind(req.api_key_env.as_deref().filter(|s| !s.is_empty()))
        .bind(models_json)
        .bind(req.default_model.as_deref())
        .bind(req.enabled)
        .bind(req.sort_order)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e {
                if db.constraint() == Some("llm_providers_slug_key") {
                    return AppError::Conflict(format!("LLM provider slug '{}' already exists", req.slug));
                }
            }
            AppError::from(e)
        })
    }

    pub async fn llm_providers_update(&self, id: i64, req: &UpdateLlmProviderRequest, clear_api_key: bool) -> AppResult<LlmProviderRow> {
        let mut sets = Vec::new();
        let mut idx = 1usize;

        macro_rules! add {
            ($field:expr, $col:expr) => {
                if $field.is_some() {
                    sets.push(format!("{} = ${}", $col, idx));
                    idx += 1;
                }
            };
        }

        add!(req.slug, "slug");
        add!(req.label, "label");
        add!(req.kind, "kind");
        add!(req.base_url, "base_url");
        add!(req.api_key_env, "api_key_env");
        add!(req.models, "models");
        add!(req.default_model, "default_model");
        add!(req.enabled, "enabled");
        add!(req.sort_order, "sort_order");

        if clear_api_key {
            sets.push("api_key = NULL".to_string());
        } else if req.api_key.is_some() {
            sets.push(format!("api_key = ${idx}"));
            idx += 1;
        }

        if sets.is_empty() {
            return self.llm_providers_get(id).await;
        }

        sets.push("updated_at = NOW()".to_string());

        let q = format!(
            "UPDATE llm_providers SET {} WHERE id = ${} RETURNING id, slug, label, kind, base_url, api_key, api_key_env, \
             models, default_model, enabled, sort_order, created_at, updated_at",
            sets.join(", "),
            idx
        );

        let mut b = sqlx::query_as::<_, LlmProviderRow>(&q);

        if let Some(ref v) = req.slug {
            b = b.bind(v.trim());
        }
        if let Some(ref v) = req.label {
            b = b.bind(v.trim());
        }
        if let Some(v) = req.kind {
            b = b.bind(v.as_str());
        }
        if let Some(ref v) = req.base_url {
            b = b.bind(v.trim());
        }
        if let Some(ref v) = req.api_key_env {
            b = b.bind(v.trim());
        }
        if let Some(ref v) = req.models {
            b = b.bind(json!(v));
        }
        if let Some(ref v) = req.default_model {
            b = b.bind(v.as_str());
        }
        if let Some(v) = req.enabled {
            b = b.bind(v);
        }
        if let Some(v) = req.sort_order {
            b = b.bind(v);
        }
        if !clear_api_key {
            if let Some(ref v) = req.api_key {
                b = b.bind(if v.is_empty() { None::<&str> } else { Some(v.as_str()) });
            }
        }
        b = b.bind(id);

        b.fetch_optional(&self.pool).await?.ok_or_else(|| AppError::NotFound(format!("LLM provider {id} not found")))
    }

    pub async fn llm_providers_delete(&self, id: i64) -> AppResult<()> {
        let r = sqlx::query("DELETE FROM llm_providers WHERE id = $1").bind(id).execute(&self.pool).await?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("LLM provider {id} not found")));
        }
        Ok(())
    }

    pub async fn llm_providers_seed_if_missing(
        &self,
        slug: &str,
        label: &str,
        kind: &str,
        base_url: &str,
        api_key: Option<&str>,
        api_key_env: Option<&str>,
        models: &[String],
        default_model: Option<&str>,
        enabled: bool,
        sort_order: i32,
    ) -> AppResult<()> {
        if self.llm_providers_get_by_slug(slug).await?.is_some() {
            return Ok(());
        }
        let id: i64 = Generator::new(3).generate();
        sqlx::query(
            r#"
            INSERT INTO llm_providers
                (id, slug, label, kind, base_url, api_key, api_key_env, models, default_model, enabled, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (slug) DO NOTHING
            "#,
        )
        .bind(id)
        .bind(slug)
        .bind(label)
        .bind(kind)
        .bind(base_url)
        .bind(api_key.filter(|s| !s.is_empty()))
        .bind(api_key_env.filter(|s| !s.is_empty()))
        .bind(json!(models))
        .bind(default_model)
        .bind(enabled)
        .bind(sort_order)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
