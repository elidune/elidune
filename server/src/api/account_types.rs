//! Library `account_types` API (roles and fine-grained rights).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;
use crate::{
    error::AppResult,
    models::account_type::{AccountTypeDefinition, CreateAccountType, UpdateAccountTypeDefinition},
    services::audit,
};

use super::{AuthenticatedUser, ClientIp};

/// List all account type definitions (`account_types` table).
#[utoipa::path(
    get,
    path = "/account-types",
    tag = "account_types",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Account types with rights", body = Vec<AccountTypeDefinition>),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Settings write required", body = ErrorResponse),
    )
)]
pub async fn list_account_types(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<Vec<AccountTypeDefinition>>> {
    claims.require_write_settings()?;
    let rows = state.services.account_types_catalog.list().await?;
    Ok(Json(rows))
}

/// Get one account type by code (e.g. `librarian`, `admin`, or a custom code).
#[utoipa::path(
    get,
    path = "/account-types/{code}",
    tag = "account_types",
    security(("bearer_auth" = [])),
    params(("code" = String, Path, description = "Account type code")),
    responses(
        (status = 200, description = "Account type", body = AccountTypeDefinition),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Settings write required", body = ErrorResponse),
        (status = 404, description = "Unknown code", body = ErrorResponse),
    )
)]
pub async fn get_account_type(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(code): Path<String>,
) -> AppResult<Json<AccountTypeDefinition>> {
    claims.require_write_settings()?;
    let row = state
        .services
        .account_types_catalog
        .get_by_code(&code)
        .await?;
    Ok(Json(row))
}

/// Create a custom account type. System types are created by migrations only.
#[utoipa::path(
    post,
    path = "/account-types",
    tag = "account_types",
    security(("bearer_auth" = [])),
    request_body = CreateAccountType,
    responses(
        (status = 201, description = "Created account type", body = AccountTypeDefinition),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Settings write required", body = ErrorResponse),
        (status = 409, description = "Code already exists", body = ErrorResponse),
    )
)]
pub async fn create_account_type(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(mut body): Json<CreateAccountType>,
) -> AppResult<(StatusCode, Json<AccountTypeDefinition>)> {
    claims.require_write_settings()?;
    let created = state
        .services
        .account_types_catalog
        .create(&mut body)
        .await?;

    state.services.audit.log(
        audit::event::ACCOUNT_TYPE_CREATED,
        Some(claims.user_id),
        Some("account_type"),
        None,
        ip,
        Some(serde_json::json!({
            "code": created.code,
            "before": serde_json::Value::Null,
            "after": &created,
        })),
        audit::AuditLogMeta::success(),
    );

    Ok((StatusCode::CREATED, Json(created)))
}

/// Update display name and/or rights for an account type. `code` is immutable.
///
/// Removing a right invalidates open sessions of accounts of that type.
/// A change that would leave no active account with settings write is refused.
#[utoipa::path(
    put,
    path = "/account-types/{code}",
    tag = "account_types",
    security(("bearer_auth" = [])),
    params(("code" = String, Path, description = "Account type code")),
    request_body = UpdateAccountTypeDefinition,
    responses(
        (status = 200, description = "Updated account type", body = AccountTypeDefinition),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Settings write required", body = ErrorResponse),
        (status = 404, description = "Unknown code", body = ErrorResponse),
        (status = 409, description = "Would leave no active admin (`last_active_admin`)", body = ErrorResponse),
    )
)]
pub async fn update_account_type(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(code): Path<String>,
    Json(mut body): Json<UpdateAccountTypeDefinition>,
) -> AppResult<Json<AccountTypeDefinition>> {
    claims.require_write_settings()?;
    let before = state
        .services
        .account_types_catalog
        .get_by_code(&code)
        .await?;
    let updated = state
        .services
        .account_types_catalog
        .update(&code, &mut body)
        .await?;

    state.services.audit.log(
        audit::event::ACCOUNT_TYPE_UPDATED,
        Some(claims.user_id),
        Some("account_type"),
        None,
        ip,
        Some(serde_json::json!({
            "code": updated.code,
            "before": before,
            "after": &updated,
        })),
        audit::AuditLogMeta::success(),
    );

    Ok(Json(updated))
}

/// Delete a custom account type. System types and types still assigned to an account are refused.
#[utoipa::path(
    delete,
    path = "/account-types/{code}",
    tag = "account_types",
    security(("bearer_auth" = [])),
    params(("code" = String, Path, description = "Account type code")),
    responses(
        (status = 204, description = "Account type deleted"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Settings write required", body = ErrorResponse),
        (status = 404, description = "Unknown code", body = ErrorResponse),
        (status = 409, description = "System type (`account_type_system`), still assigned (`account_type_in_use`), or last admin (`last_active_admin`)", body = ErrorResponse),
    )
)]
pub async fn delete_account_type(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(code): Path<String>,
) -> AppResult<StatusCode> {
    claims.require_write_settings()?;
    let deleted = state.services.account_types_catalog.delete(&code).await?;

    state.services.audit.log(
        audit::event::ACCOUNT_TYPE_DELETED,
        Some(claims.user_id),
        Some("account_type"),
        None,
        ip,
        Some(serde_json::json!({
            "code": deleted.code,
            "before": deleted,
            "after": serde_json::Value::Null,
        })),
        audit::AuditLogMeta::success(),
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Routes under `/api/v1`.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::get;
    axum::Router::new()
        .route(
            "/account-types",
            get(list_account_types).post(create_account_type),
        )
        .route(
            "/account-types/:code",
            get(get_account_type)
                .put(update_account_type)
                .delete(delete_account_type),
        )
}
