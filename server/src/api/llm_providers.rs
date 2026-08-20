//! Admin LLM provider HTTP API.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::{
    error::AppResult,
    models::chat::{CreateLlmProviderRequest, LlmProviderAdmin, UpdateLlmProviderRequest},
    AppState,
};

use super::AdminUser;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/llm/providers", get(list_providers).post(create_provider))
        .route("/admin/llm/providers/:id", axum::routing::put(update_provider).delete(delete_provider))
        .route("/admin/llm/providers/:id/test", post(test_provider))
}

#[utoipa::path(
    get,
    path = "/admin/llm/providers",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "LLM providers", body = Vec<LlmProviderAdmin>),
        (status = 403, description = "Admin required")
    )
)]
pub async fn list_providers(State(state): State<AppState>, AdminUser(_claims): AdminUser) -> AppResult<Json<Vec<LlmProviderAdmin>>> {
    let list = state.services.llm_providers.list_admin().await?;
    Ok(Json(list))
}

#[utoipa::path(
    post,
    path = "/admin/llm/providers",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = CreateLlmProviderRequest,
    responses(
        (status = 200, description = "Created provider", body = LlmProviderAdmin),
        (status = 403, description = "Admin required")
    )
)]
pub async fn create_provider(State(state): State<AppState>, AdminUser(claims): AdminUser, Json(req): Json<CreateLlmProviderRequest>) -> AppResult<Json<LlmProviderAdmin>> {
    let created = state.services.llm_providers.create(req).await?;
    state.services.audit.log(
        crate::services::audit::event::SETTINGS_UPDATED,
        Some(claims.user_id),
        Some("llm_provider"),
        Some(created.id),
        None,
        Some(serde_json::json!({ "action": "create" })),
        crate::services::audit::AuditLogMeta::success(),
    );
    Ok(Json(created))
}

#[serde_as]
#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ProviderIdPath {
    #[serde_as(as = "DisplayFromStr")]
    #[param(value_type = String)]
    pub id: i64,
}

#[utoipa::path(
    put,
    path = "/admin/llm/providers/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    request_body = UpdateLlmProviderRequest,
    responses(
        (status = 200, description = "Updated provider", body = LlmProviderAdmin),
        (status = 404, description = "Not found")
    )
)]
pub async fn update_provider(State(state): State<AppState>, AdminUser(claims): AdminUser, Path(id): Path<i64>, Json(req): Json<UpdateLlmProviderRequest>) -> AppResult<Json<LlmProviderAdmin>> {
    let updated = state.services.llm_providers.update(id, req).await?;
    state.services.audit.log(
        crate::services::audit::event::SETTINGS_UPDATED,
        Some(claims.user_id),
        Some("llm_provider"),
        Some(updated.id),
        None,
        Some(serde_json::json!({ "action": "update" })),
        crate::services::audit::AuditLogMeta::success(),
    );
    Ok(Json(updated))
}

#[utoipa::path(
    delete,
    path = "/admin/llm/providers/{id}",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_provider(State(state): State<AppState>, AdminUser(claims): AdminUser, Path(id): Path<i64>) -> AppResult<impl axum::response::IntoResponse> {
    state.services.llm_providers.delete(id).await?;
    state.services.audit.log(
        crate::services::audit::event::SETTINGS_UPDATED,
        Some(claims.user_id),
        Some("llm_provider"),
        Some(id),
        None,
        Some(serde_json::json!({ "action": "delete" })),
        crate::services::audit::AuditLogMeta::success(),
    );
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestProviderResponse {
    pub ok: bool,
}

#[utoipa::path(
    post,
    path = "/admin/llm/providers/{id}/test",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "Connection test result", body = TestProviderResponse))
)]
pub async fn test_provider(State(state): State<AppState>, AdminUser(_claims): AdminUser, Path(id): Path<i64>) -> AppResult<Json<TestProviderResponse>> {
    state.services.llm_providers.test(id, &state.config.chat).await?;
    Ok(Json(TestProviderResponse { ok: true }))
}
