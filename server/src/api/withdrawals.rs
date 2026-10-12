//! Staff weeding withdrawals (#139).

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};

use crate::{
    error::AppResult,
    models::withdrawal::{
        BatchDisposeRequest, BatchWithdrawalRequest, BatchWithdrawalResponse, ItemWithdrawal,
        WithdrawalList, WithdrawalQuery, WithdrawalWrite,
    },
    services::audit,
};

use super::{AuthenticatedUser, ClientIp};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, patch, post};
    axum::Router::new()
        .route("/items/withdrawals", post(batch_withdraw))
        .route(
            "/items/:id/withdrawals",
            post(withdraw_item).get(get_active_item_withdrawal),
        )
        .route("/item-withdrawals/export.csv", get(export_withdrawals_csv))
        .route("/item-withdrawals/dispose", post(dispose_withdrawals))
        .route("/item-withdrawals", get(list_withdrawals))
        .route("/item-withdrawals/:id/cancel", post(cancel_withdrawal))
        .route(
            "/item-withdrawals/:id",
            get(get_withdrawal).patch(complete_legacy_withdrawal),
        )
}

#[utoipa::path(
    post,
    path = "/items/{id}/withdrawals",
    tag = "items",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Physical copy (item) ID")),
    request_body = WithdrawalWrite,
    responses(
        (status = 201, description = "Copy withdrawn", body = ItemWithdrawal),
        (status = 400, description = "Reason detail or destination missing", body = crate::error::ErrorResponse),
        (status = 403, description = "Missing weeding right", body = crate::error::ErrorResponse),
        (status = 404, description = "Item not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Active withdrawal already exists", body = crate::error::ErrorResponse),
        (status = 422, description = "Copy is on loan, in transit, or held for pickup", body = crate::error::ErrorResponse)
    )
)]
pub async fn withdraw_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    Json(body): Json<WithdrawalWrite>,
) -> AppResult<(StatusCode, Json<ItemWithdrawal>)> {
    claims.require_weeding()?;
    match state
        .services
        .catalog
        .withdraw_item(item_id, claims.user_id, &body)
        .await
    {
        Ok(withdrawal) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWN,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip.clone(),
                Some(serde_json::json!({
                    "itemId": item_id.to_string(),
                    "withdrawalId": withdrawal.id.to_string(),
                    "reason": body.reason,
                    "destination": body.destination,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(withdrawal)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWN,
                Some(claims.user_id),
                Some("item"),
                Some(item_id),
                ip,
                Some(serde_json::json!({ "itemId": item_id.to_string() })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/items/withdrawals",
    tag = "items",
    security(("bearer_auth" = [])),
    request_body = BatchWithdrawalRequest,
    responses(
        (status = 201, description = "Every copy withdrawn, or none", body = BatchWithdrawalResponse),
        (status = 400, description = "Empty batch, duplicate ids, or invalid reason", body = crate::error::ErrorResponse),
        (status = 422, description = "One copy cannot be withdrawn; the batch is rolled back", body = crate::error::ErrorResponse)
    )
)]
pub async fn batch_withdraw(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(body): Json<BatchWithdrawalRequest>,
) -> AppResult<(StatusCode, Json<BatchWithdrawalResponse>)> {
    claims.require_weeding()?;
    let write = body.write();
    match state
        .services
        .catalog
        .withdraw_items(&body.item_ids, claims.user_id, &write)
        .await
    {
        Ok(response) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWALS_BATCH,
                Some(claims.user_id),
                Some("item"),
                None,
                ip.clone(),
                Some(serde_json::json!({
                    "itemIds": body.item_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    "reason": body.reason,
                    "destination": body.destination,
                    "count": response.withdrawals.len(),
                })),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWALS_BATCH,
                Some(claims.user_id),
                Some("item"),
                None,
                ip,
                Some(serde_json::json!({
                    "itemIds": body.item_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/item-withdrawals/{id}/cancel",
    tag = "items",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Withdrawal ID")),
    responses(
        (status = 200, description = "Withdrawal cancelled and the copy returned to onShelf", body = ItemWithdrawal),
        (status = 403, description = "Missing weeding cancel right", body = crate::error::ErrorResponse),
        (status = 404, description = "Withdrawal not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Withdrawal is already cancelled or already disposed", body = crate::error::ErrorResponse)
    )
)]
pub async fn cancel_withdrawal(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<ItemWithdrawal>> {
    claims.require_cancel_withdrawal()?;
    match state
        .services
        .catalog
        .cancel_withdrawal(id, claims.user_id)
        .await
    {
        Ok(withdrawal) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWAL_CANCELLED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({
                    "withdrawalId": id.to_string(),
                    "itemId": withdrawal.item_id.to_string(),
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(withdrawal))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWAL_CANCELLED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/item-withdrawals/dispose",
    tag = "items",
    security(("bearer_auth" = [])),
    request_body = BatchDisposeRequest,
    responses(
        (status = 200, description = "Every withdrawal marked disposed, or none", body = BatchWithdrawalResponse),
        (status = 400, description = "Empty batch, duplicate ids, or invalid disposedAt", body = crate::error::ErrorResponse),
        (status = 403, description = "Missing weeding right", body = crate::error::ErrorResponse),
        (status = 409, description = "Every rejected id is cancelled or already disposed", body = crate::models::withdrawal::WithdrawalDisposeRejectedBody),
        (status = 422, description = "At least one id is unknown. Nothing was disposed", body = crate::models::withdrawal::WithdrawalDisposeRejectedBody)
    )
)]
pub async fn dispose_withdrawals(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(body): Json<BatchDisposeRequest>,
) -> AppResult<Json<BatchWithdrawalResponse>> {
    claims.require_weeding()?;
    match state
        .services
        .catalog
        .dispose_withdrawals(
            &body.withdrawal_ids,
            claims.user_id,
            body.disposed_at.as_deref(),
        )
        .await
    {
        Ok(response) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWALS_DISPOSED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                None,
                ip.clone(),
                Some(serde_json::json!({
                    "withdrawalIds": body
                        .withdrawal_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>(),
                    "count": response.withdrawals.len(),
                    "disposedAt": body.disposed_at,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(response))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWALS_DISPOSED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                None,
                ip,
                Some(serde_json::json!({
                    "withdrawalIds": body
                        .withdrawal_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>(),
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    patch,
    path = "/item-withdrawals/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Legacy withdrawal ID")),
    request_body = WithdrawalWrite,
    responses(
        (status = 200, description = "Legacy withdrawal completed", body = ItemWithdrawal),
        (status = 422, description = "Row is not a legacy withdrawal", body = crate::error::ErrorResponse)
    )
)]
pub async fn complete_legacy_withdrawal(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(body): Json<WithdrawalWrite>,
) -> AppResult<Json<ItemWithdrawal>> {
    claims.require_weeding()?;
    match state
        .services
        .catalog
        .complete_legacy_withdrawal(id, &body)
        .await
    {
        Ok(withdrawal) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWAL_COMPLETED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({
                    "withdrawalId": id.to_string(),
                    "reason": body.reason,
                    "destination": body.destination,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(withdrawal))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::ITEM_WITHDRAWAL_COMPLETED,
                Some(claims.user_id),
                Some("item_withdrawal"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    get,
    path = "/items/{id}/withdrawals",
    tag = "items",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Physical copy (item) ID")),
    responses(
        (status = 200, description = "Active withdrawal for this copy", body = ItemWithdrawal),
        (status = 403, description = "Missing weeding right", body = crate::error::ErrorResponse),
        (status = 404, description = "No active withdrawal", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_active_item_withdrawal(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(item_id): Path<i64>,
) -> AppResult<Json<ItemWithdrawal>> {
    claims.require_read_weeding()?;
    let withdrawal = state
        .services
        .catalog
        .get_active_item_withdrawal(item_id)
        .await?;
    Ok(Json(withdrawal))
}

#[utoipa::path(
    get,
    path = "/item-withdrawals/{id}",
    tag = "items",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Withdrawal ID")),
    responses(
        (status = 200, description = "Withdrawal record", body = ItemWithdrawal),
        (status = 403, description = "Missing weeding right", body = crate::error::ErrorResponse),
        (status = 404, description = "Withdrawal not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_withdrawal(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<ItemWithdrawal>> {
    claims.require_read_weeding()?;
    let withdrawal = state.services.catalog.get_withdrawal(id).await?;
    Ok(Json(withdrawal))
}

#[utoipa::path(
    get,
    path = "/item-withdrawals",
    tag = "items",
    security(("bearer_auth" = [])),
    params(WithdrawalQuery),
    responses(
        (status = 200, description = "Withdrawn copies, with totals by reason and destination", body = WithdrawalList)
    )
)]
pub async fn list_withdrawals(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<WithdrawalQuery>,
) -> AppResult<Json<WithdrawalList>> {
    claims.require_read_weeding()?;
    let list = state.services.catalog.list_withdrawals(&query).await?;
    Ok(Json(list))
}

#[utoipa::path(
    get,
    path = "/item-withdrawals/export.csv",
    tag = "items",
    security(("bearer_auth" = [])),
    params(WithdrawalQuery),
    responses(
        (status = 200, description = "One UTF-8 CSV file with a BOM, semicolon separators, and French headers: Code-barres, Titre, Auteur, Cote, Site, Date d'acquisition, Prix d'achat, Motif, Précision, Destination, Date de retrait, Retiré par, Éliminé le, Éliminé par. Dates are DD/MM/YYYY in the library timezone. Prix d'achat reads items.price as decimal euros (12.50 prints 12,50, 0 prints 0,00). Blank, unparseable, and negative prices are empty. Formula-like cells are prefixed with a quote. A null legacy destination is localized.", content_type = "text/csv"),
        (status = 400, description = "from and to are required", body = crate::error::ErrorResponse)
    )
)]
pub async fn export_withdrawals_csv(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<WithdrawalQuery>,
) -> AppResult<axum::response::Response> {
    claims.require_read_weeding()?;
    let csv = state
        .services
        .catalog
        .export_withdrawals_csv(&query, claims.user_id)
        .await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"withdrawals.csv\"",
            ),
        ],
        csv,
    )
        .into_response())
}
