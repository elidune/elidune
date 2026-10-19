//! Lenders and incoming deposit batches.

use crate::{
    error::AppResult,
    models::deposit::{
        CreateDepositBatch, CreateLender, DepositBatch, DepositBatchDetail, DepositBatchItem,
        DepositBatchItemsQuery, DepositBatchListQuery, DepositCloseReport, DepositScanOut,
        DepositScanOutRequest, Lender, UpdateDepositBatch, UpdateLender,
    },
    services::audit,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, patch, post};
    axum::Router::new()
        .route("/lenders", get(list_lenders).post(create_lender))
        .route("/lenders/:id", patch(update_lender).delete(archive_lender))
        .route(
            "/deposit-batches",
            get(list_deposit_batches).post(create_deposit_batch),
        )
        .route(
            "/deposit-batches/:id",
            get(get_deposit_batch).patch(update_deposit_batch),
        )
        .route("/deposit-batches/:id/items", get(list_deposit_batch_items))
        .route(
            "/deposit-batches/:id/start-return",
            post(start_deposit_return),
        )
        .route("/deposit-batches/:id/scan-out", post(scan_out_deposit_copy))
        .route("/deposit-batches/:id/close", post(close_deposit_batch))
}

/// Query parameters for listing lenders.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct LendersQuery {
    /// Include archived lenders. Default is active lenders only.
    pub include_archived: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/lenders",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(LendersQuery),
    responses(
        (status = 200, description = "Lenders. Active only unless includeArchived is true.", body = Vec<Lender>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_lenders(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<LendersQuery>,
) -> AppResult<Json<Vec<Lender>>> {
    claims.require_read_items()?;
    Ok(Json(
        state
            .services
            .deposits
            .list_lenders(query.include_archived.unwrap_or(false))
            .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/lenders",
    tag = "deposits",
    security(("bearer_auth" = [])),
    request_body = CreateLender,
    responses(
        (status = 201, description = "Lender created", body = Lender),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 409, description = "A lender with this name already exists", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_lender(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreateLender>,
) -> AppResult<(StatusCode, Json<Lender>)> {
    claims.require_write_items()?;
    let lender = state.services.deposits.create_lender(&data).await?;
    state.services.audit.log(
        audit::event::LENDER_CREATED,
        Some(claims.user_id),
        Some("lender"),
        Some(lender.id),
        ip,
        Some(&lender),
        audit::AuditLogMeta::success(),
    );
    Ok((StatusCode::CREATED, Json(lender)))
}

#[utoipa::path(
    patch,
    path = "/lenders/{id}",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Lender ID")),
    request_body = UpdateLender,
    responses(
        (status = 200, description = "Lender updated. Omit `replacementRateCents` to leave it unchanged. JSON `null` clears it. Zero and negative values are `validation_error`.", body = Lender),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Lender not found", body = crate::error::ErrorResponse),
        (status = 409, description = "A lender with this name already exists", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_lender(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<UpdateLender>,
) -> AppResult<Json<Lender>> {
    claims.require_write_items()?;
    let lender = state.services.deposits.update_lender(id, &data).await?;
    state.services.audit.log(
        audit::event::LENDER_UPDATED,
        Some(claims.user_id),
        Some("lender"),
        Some(id),
        ip,
        Some(&lender),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(lender))
}

#[utoipa::path(
    delete,
    path = "/lenders/{id}",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Lender ID")),
    responses(
        (status = 200, description = "Lender archived", body = Lender),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Lender not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Lender still has a batch that is not closed (`lender_has_active_batches`)", body = crate::error::ErrorResponse)
    )
)]
pub async fn archive_lender(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<Lender>> {
    claims.require_write_items()?;
    let lender = state.services.deposits.archive_lender(id).await?;
    state.services.audit.log(
        audit::event::LENDER_ARCHIVED,
        Some(claims.user_id),
        Some("lender"),
        Some(id),
        ip,
        Some(&lender),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(lender))
}

#[utoipa::path(
    get,
    path = "/deposit-batches",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(DepositBatchListQuery),
    responses(
        (status = 200, description = "Deposit batches", body = Vec<DepositBatch>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Lender not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_deposit_batches(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<DepositBatchListQuery>,
) -> AppResult<Json<Vec<DepositBatch>>> {
    claims.require_read_items()?;
    Ok(Json(
        state
            .services
            .deposits
            .list_batches(query.lender_id, query.status)
            .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/deposit-batches/{id}",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Deposit batch ID")),
    responses(
        (status = 200, description = "Deposit batch and copy counts", body = DepositBatchDetail),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Deposit batch not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn get_deposit_batch(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<DepositBatchDetail>> {
    claims.require_read_items()?;
    Ok(Json(state.services.deposits.get_batch(id).await?))
}

#[utoipa::path(
    post,
    path = "/deposit-batches",
    tag = "deposits",
    security(("bearer_auth" = [])),
    request_body = CreateDepositBatch,
    responses(
        (status = 201, description = "Deposit batch created", body = DepositBatch),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Lender or site not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Reference already used for this lender", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_deposit_batch(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreateDepositBatch>,
) -> AppResult<(StatusCode, Json<DepositBatch>)> {
    claims.require_write_items()?;
    let batch = state.services.deposits.create_batch(&data).await?;
    state.services.audit.log(
        audit::event::DEPOSIT_BATCH_CREATED,
        Some(claims.user_id),
        Some("deposit_batch"),
        Some(batch.id),
        ip,
        Some(&batch),
        audit::AuditLogMeta::success(),
    );
    Ok((StatusCode::CREATED, Json(batch)))
}

#[utoipa::path(
    patch,
    path = "/deposit-batches/{id}",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Deposit batch ID")),
    request_body = UpdateDepositBatch,
    responses(
        (status = 200, description = "Deposit batch updated. siteId does not move copies.", body = DepositBatch),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Deposit batch or site not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Batch is not active (`deposit_batch_not_active`)", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_deposit_batch(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<UpdateDepositBatch>,
) -> AppResult<Json<DepositBatch>> {
    claims.require_write_items()?;
    let batch = state.services.deposits.update_batch(id, &data).await?;
    state.services.audit.log(
        audit::event::DEPOSIT_BATCH_UPDATED,
        Some(claims.user_id),
        Some("deposit_batch"),
        Some(id),
        ip,
        Some(&batch),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(batch))
}

#[utoipa::path(
    post,
    path = "/deposit-batches/{id}/start-return",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Deposit batch ID")),
    responses(
        (status = 200, description = "Batch is `returning`. Calling this again is a no-op. Holds pinned to its copies return to the title queue.", body = DepositBatch),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Deposit batch not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Batch is closed (`deposit_batch_not_active`)", body = crate::error::ErrorResponse)
    )
)]
pub async fn start_deposit_return(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<DepositBatch>> {
    claims.require_write_items()?;
    let batch = state.services.deposits.start_return(id).await?;
    state.services.audit.log(
        audit::event::DEPOSIT_BATCH_RETURN_STARTED,
        Some(claims.user_id),
        Some("deposit_batch"),
        Some(id),
        ip,
        Some(&batch),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(batch))
}

#[utoipa::path(
    post,
    path = "/deposit-batches/{id}/scan-out",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Deposit batch ID")),
    request_body = DepositScanOutRequest,
    responses(
        (status = 200, description = "Copy marked returned to the lender. A second scan keeps the first timestamp.", body = DepositScanOut),
        (status = 400, description = "Validation error", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Batch not found, or the barcode is not a copy of this batch", body = crate::error::ErrorResponse),
        (status = 422, description = "Batch is not `returning` (`deposit_batch_not_returning`, body `{ code, error, message }`) or this copy still has an open loan (`deposit_item_on_loan`). The loan body is `{ code, error, message, itemId, barcode }`. `itemId` is a string. `barcode` is null when the copy has none. Check the copy in at the desk (`returnToLender`) before scanning it out.", body = crate::error::DepositItemOnLoanBody)
    )
)]
pub async fn scan_out_deposit_copy(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(body): ValidatedJson<DepositScanOutRequest>,
) -> AppResult<Json<DepositScanOut>> {
    claims.require_write_items()?;
    let scanned = state.services.deposits.scan_out(id, &body.barcode).await?;
    state.services.audit.log(
        audit::event::DEPOSIT_BATCH_SCANNED_OUT,
        Some(claims.user_id),
        Some("item"),
        Some(scanned.item_id),
        ip,
        Some(&scanned),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(scanned))
}

#[utoipa::path(
    post,
    path = "/deposit-batches/{id}/close",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Deposit batch ID")),
    responses(
        (status = 200, description = "Batch closed. The report is `{ returned, missing }`. Copies with `depositReturnedAt` are `returned` and archived. Copies without it (still on the shelf, or declared lost) are `missing` and archived. Nothing is deleted. Calling this again on a closed batch returns the report again.", body = DepositCloseReport),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Deposit batch not found", body = crate::error::ErrorResponse),
        (status = 422, description = "Batch is still `active` (`deposit_batch_not_returning`, body `{ code, error, message }`) or a copy is still on loan (`deposit_batch_has_active_loans`). The loans body is `{ code, error, message, copies: [{ itemId, barcode }] }`. `itemId` is a string. `barcode` is null when the copy has none. The batch stays `returning`.", body = crate::error::DepositBatchHasActiveLoansBody)
    )
)]
pub async fn close_deposit_batch(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<DepositCloseReport>> {
    claims.require_write_items()?;
    let report = state.services.deposits.close_batch(id).await?;
    state.services.audit.log(
        audit::event::DEPOSIT_BATCH_CLOSED,
        Some(claims.user_id),
        Some("deposit_batch"),
        Some(id),
        ip,
        Some(&report),
        audit::AuditLogMeta::success(),
    );
    Ok(Json(report))
}

#[utoipa::path(
    get,
    path = "/deposit-batches/{id}/items",
    tag = "deposits",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Deposit batch ID"),
        DepositBatchItemsQuery
    ),
    responses(
        (status = 200, description = "Copies in the batch", body = Vec<DepositBatchItem>),
        (status = 400, description = "Unknown state", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = crate::error::ErrorResponse),
        (status = 404, description = "Deposit batch not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_deposit_batch_items(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
    Query(query): Query<DepositBatchItemsQuery>,
) -> AppResult<Json<Vec<DepositBatchItem>>> {
    claims.require_read_items()?;
    Ok(Json(
        state
            .services
            .deposits
            .list_batch_items(id, query.state.as_deref())
            .await?,
    ))
}
