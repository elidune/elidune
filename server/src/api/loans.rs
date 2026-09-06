//! Loan management endpoints

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        StatusCode,
    },
    response::Response,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::{IntoParams, ToSchema};

use crate::{
    error::{AppError, AppResult},
    models::{
        loan::{CreateLoan, LoanDetails, LoanMarcExportEncoding, LoanMarcExportFormat},
        user::{Rights, UserShort},
    },
    services::{
        audit::{self},
        idempotency,
        reminders::{OverdueLoansPage, ReminderReport},
    },
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, OptionalIdempotencyKey};

pub use crate::models::dto::circulation::{
    CirculationExceptionResponse, ClaimsReturnedQueueItem, MarkClaimedReturnedRequest,
    MarkDamagedRequest, MarkLostRequest, ResolveClaimsReturnedRequest,
};
pub use crate::models::dto::loans::{LoanSettingsDto as LoanSettings, UpdateLoanSettingsRequest};

/// Build the loans routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/loans", post(create_loan))
        .route(
            "/loans/settings",
            get(get_loan_settings).put(update_loan_settings),
        )
        .route("/loans/overdue", get(get_overdue_loans))
        .route(
            "/loans/send-overdue-reminders",
            post(send_overdue_reminders),
        )
        .route("/loans/claims-returned", get(list_claims_returned))
        .route("/loans/:id/user", get(get_loan_borrower))
        .route("/loans/:id/return", post(return_loan))
        .route("/loans/:id/renew", post(renew_loan))
        .route("/loans/:id/lost", post(mark_loan_lost))
        .route("/loans/:id/damaged", post(mark_loan_damaged))
        .route(
            "/loans/:id/claimed-returned",
            post(mark_loan_claimed_returned),
        )
        .route(
            "/loans/:id/claims-returned/resolve",
            post(resolve_loan_claims_returned),
        )
        .route("/loans/items/:item_id/return", post(return_loan_by_item))
        .route("/loans/items/:item_id/renew", post(renew_loan_by_item))
}

/// Get the patron linked to an active loan (circulation desk).
#[utoipa::path(
    get,
    path = "/loans/{id}/user",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    responses(
        (status = 200, description = "Borrower profile", body = UserShort),
        (status = 403, description = "Insufficient loans write rights"),
        (status = 404, description = "Loan not found")
    )
)]
pub async fn get_loan_borrower(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(loan_id): Path<i64>,
) -> AppResult<Json<UserShort>> {
    claims.require_write_loans()?;
    let user = state.services.loans.get_loan_borrower(loan_id).await?;
    Ok(Json(user))
}

/// Create loan request
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLoanRequest {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    pub item_identification: Option<String>,
    /// When true, bypasses patron/subscription/limits/unpaid-fine checks and hold-queue rules; active holds on the copy are cancelled.
    pub force: Option<bool>,
}

/// Optional staff override for renew.
#[derive(Debug, Default, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct RenewLoanQuery {
    /// When true, bypasses account and unpaid-fine threshold checks (staff only; audited).
    pub force: Option<bool>,
}

#[derive(Serialize)]
struct LoanCreatedAudit {
    user_id: i64,
    item_id: Option<i64>,
    item_identification: Option<String>,
    force: bool,
    expiry_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct RenewLoanAudit {
    new_expiry_at: DateTime<Utc>,
    renew_count: i16,
}

#[derive(Serialize)]
struct RenewLoanByItemAudit {
    item_identification: String,
    new_expiry_at: DateTime<Utc>,
    renew_count: i16,
}

#[derive(Serialize)]
struct ReminderBatchManualAudit {
    triggered_by: &'static str,
    emails_sent: u32,
    loans_reminded: u32,
    errors: usize,
}

/// Loan response with calculated dates
#[serde_as]
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanResponse {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub expiry_at: DateTime<Utc>,
    pub message: String,
}

/// Return response with loan details
#[derive(Serialize, ToSchema)]
pub struct ReturnResponse {
    pub status: String,
    pub loan: LoanDetails,
}

/// Query parameters for overdue loans list
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct OverdueLoansQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Query parameters for sending reminders
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SendRemindersQuery {
    /// If true, no emails are sent; only shows what would be sent
    pub dry_run: Option<bool>,
}

/// Get global loan rules per media type (`loans_settings`).
#[utoipa::path(
    get,
    path = "/loans/settings",
    tag = "loans",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Global loan rules per media type", body = Vec<LoanSettings>),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn get_loan_settings(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<Vec<LoanSettings>>> {
    claims.require_read_settings()?;
    let rows = state.services.loans.get_global_loan_settings().await?;
    Ok(Json(rows))
}

/// Update global loan rules per media type.
#[utoipa::path(
    put,
    path = "/loans/settings",
    tag = "loans",
    security(("bearer_auth" = [])),
    request_body = UpdateLoanSettingsRequest,
    responses(
        (status = 200, description = "Updated global loan rules", body = Vec<LoanSettings>),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn update_loan_settings(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(body): Json<UpdateLoanSettingsRequest>,
) -> AppResult<Json<Vec<LoanSettings>>> {
    claims.require_write_settings()?;
    let rows = state
        .services
        .loans
        .update_global_loan_settings(body)
        .await?;

    state.services.audit.log(
        audit::event::SETTINGS_UPDATED,
        Some(claims.user_id),
        None,
        None,
        ip,
        Some(serde_json::json!({ "scope": "loans", "loanSettings": rows })),
        audit::AuditLogMeta::success(),
    );

    Ok(Json(rows))
}

/// Get loans for a specific user (paginated).
#[utoipa::path(
    get,
    path = "/users/{id}/loans",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "User ID"),
        GetUserLoansQuery
    ),
    responses(
        (status = 200, description = "User's loans", body = PaginatedResponse<LoanDetails>),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user_loans(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(user_id): Path<i64>,
    Query(query): Query<GetUserLoansQuery>,
) -> AppResult<Json<PaginatedResponse<LoanDetails>>> {
    claims.require_self_or_staff(user_id)?;

    if claims.rights.loans_rights.rank() < Rights::Read.rank() && user_id != claims.user_id {
        return Err(AppError::Authorization(
            "Insufficient rights to read loans for another user".into(),
        ));
    }

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 200);

    let (items, total) = if query.archived.unwrap_or(false) {
        state
            .services
            .loans
            .get_user_archived_loans(user_id, page, per_page)
            .await?
    } else {
        state
            .services
            .loans
            .get_user_loans(user_id, page, per_page)
            .await?
    };

    Ok(Json(PaginatedResponse::new(items, total, page, per_page)))
}

/// Query for MARC export download (no pagination; full list in one file).
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ExportUserLoansMarcQuery {
    /// If true, export archived (returned) loans instead of active loans.
    pub archived: Option<bool>,
    /// Output serialization: `json`, `marc21`, `unimarc`, `marcxml` (default: `json`).
    #[serde(default)]
    pub format: LoanMarcExportFormat,
    /// Character encoding for ISO2709 binary (`marc21`, `unimarc`). Ignored for `json` and `marcxml` (UTF-8). Default: `utf8`.
    #[serde(default)]
    pub encoding: LoanMarcExportEncoding,
}

/// Download all loans for a user as one MARC file (`Content-Disposition: attachment`).
#[utoipa::path(
    get,
    path = "/users/{id}/loans/export",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "User ID"),
        ExportUserLoansMarcQuery
    ),
    responses(
        (status = 200, description = "File attachment (JSON array of marc-rs records, or ISO2709, or MARC-XML collection)"),
        (status = 400, description = "Too many loans to export"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "User not found")
    )
)]
pub async fn export_user_loans_marc(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(user_id): Path<i64>,
    Query(query): Query<ExportUserLoansMarcQuery>,
) -> AppResult<Response> {
    claims.require_self_or_staff(user_id)?;
    let archived = query.archived.unwrap_or(false);
    let (bytes, content_type, filename) = state
        .services
        .loans
        .export_user_loans_marc_file(user_id, archived, query.format, query.encoding)
        .await?;
    let disposition = format!(r#"attachment; filename="{}""#, filename);
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_DISPOSITION, disposition)
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(format!("export response: {}", e)))
}

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct GetUserLoansQuery {
    /// If true, return past (returned) loans from the archive table
    pub archived: Option<bool>,
    /// Page number (1-based, default 1)
    pub page: Option<i64>,
    /// Page size (default 20, max 200)
    pub per_page: Option<i64>,
}

/// Create a new loan (borrow an item)
#[utoipa::path(
    post,
    path = "/loans",
    tag = "loans",
    security(("bearer_auth" = [])),
    request_body = CreateLoanRequest,
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "Optional 1–255 char key (`A–Z a–z 0–9 _ - . ~`). Same actor+key+payload replays the stored 2xx for 24h. Same key with a different payload returns 409.")
    ),
    responses(
        (status = 201, description = "Loan created", body = LoanResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "User or specimen not found"),
        (status = 409, description = "Specimen already borrowed, max loans reached, or Idempotency-Key conflict"),
        (status = 422, description = "Business rule: unpaid fines exceed the configured threshold (staff may retry with force=true)")
    )
)]
pub async fn create_loan(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    OptionalIdempotencyKey(idempotency_key): OptionalIdempotencyKey,
    Json(request): Json<CreateLoanRequest>,
) -> AppResult<(StatusCode, Json<LoanResponse>)> {
    claims.require_write_loans()?;
    let force = request.force.unwrap_or(false);
    let fingerprint = serde_json::json!({
        "op": "loans.create",
        "userId": request.user_id.to_string(),
        "itemId": request.item_id.map(|id| id.to_string()),
        "itemIdentification": request.item_identification,
        "force": force,
    });

    let (status, body) = idempotency::execute(
        state.services.repository.as_ref(),
        claims.user_id,
        idempotency_key.as_deref(),
        "loans.create",
        &fingerprint,
        || {
            let state = state.clone();
            let ip = ip.clone();
            let request = CreateLoan {
                user_id: request.user_id,
                item_id: request.item_id,
                item_identification: request.item_identification.clone(),
                force,
            };
            async move { create_loan_inner(state, claims.user_id, ip, request).await }
        },
    )
    .await?;
    Ok((status, Json(body)))
}

async fn create_loan_inner(
    state: crate::AppState,
    actor_id: i64,
    ip: Option<String>,
    loan: CreateLoan,
) -> AppResult<(StatusCode, LoanResponse)> {
    let user_id = loan.user_id;
    let item_id = loan.item_id;
    let item_identification = loan.item_identification.clone();
    let force = loan.force;

    let outcome = match state
        .services
        .loans
        .create_loan(loan, Some(actor_id), ip.clone())
        .await
    {
        Ok(outcome) => outcome,
        Err(e) => {
            state.services.audit.log(
                audit::event::LOAN_CREATED,
                Some(actor_id),
                Some("loan"),
                None,
                ip.clone(),
                Some(LoanCreatedAudit {
                    user_id,
                    item_id,
                    item_identification,
                    force,
                    expiry_at: Utc::now(),
                }),
                audit::AuditLogMeta::from_app_error(&e),
            );
            return Err(e);
        }
    };
    let loan_id = outcome.loan_id;
    let expiry_at = outcome.expiry_at;

    state.services.audit.log(
        audit::event::LOAN_CREATED,
        Some(actor_id),
        Some("loan"),
        Some(loan_id),
        ip,
        Some(LoanCreatedAudit {
            user_id,
            item_id,
            item_identification,
            force,
            expiry_at,
        }),
        audit::AuditLogMeta::success(),
    );

    Ok((
        StatusCode::CREATED,
        LoanResponse {
            id: loan_id,
            expiry_at,
            message: "Item borrowed successfully".to_string(),
        },
    ))
}

/// Return a borrowed item
#[utoipa::path(
    post,
    path = "/loans/{id}/return",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Loan ID")),
    responses(
        (status = 200, description = "Item returned", body = ReturnResponse),
        (status = 404, description = "Loan not found"),
        (status = 409, description = "Already returned")
    )
)]
pub async fn return_loan(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(loan_id): Path<i64>,
) -> AppResult<Json<ReturnResponse>> {
    claims.require_write_loans()?;
    let loan = match state
        .services
        .loans
        .return_loan(loan_id, Some(claims.user_id), ip.clone())
        .await
    {
        Ok(loan) => loan,
        Err(e) => {
            state.services.audit.log(
                audit::event::LOAN_RETURNED,
                Some(claims.user_id),
                Some("loan"),
                Some(loan_id),
                ip.clone(),
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            return Err(e);
        }
    };

    state.services.audit.log(
        audit::event::LOAN_RETURNED,
        Some(claims.user_id),
        Some("loan"),
        Some(loan_id),
        ip,
        Some(&loan),
        audit::AuditLogMeta::success(),
    );

    Ok(Json(ReturnResponse {
        status: "returned".to_string(),
        loan,
    }))
}

/// Renew a loan
#[utoipa::path(
    post,
    path = "/loans/{id}/renew",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "Loan ID"),
        RenewLoanQuery,
        ("Idempotency-Key" = Option<String>, Header, description = "Optional 1–255 char key (`A–Z a–z 0–9 _ - . ~`). Same actor+key+payload replays the stored 2xx for 24h. Same key with a different payload returns 409.")
    ),
    responses(
        (status = 200, description = "Loan renewed", body = LoanResponse),
        (status = 404, description = "Loan not found"),
        (status = 409, description = "Idempotency-Key reused with a different payload"),
        (status = 422, description = "Business rule: max renewals reached, already returned, hold waiting, or unpaid fines exceed the threshold")
    )
)]
pub async fn renew_loan(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    OptionalIdempotencyKey(idempotency_key): OptionalIdempotencyKey,
    Path(loan_id): Path<i64>,
    Query(query): Query<RenewLoanQuery>,
) -> AppResult<Json<LoanResponse>> {
    let loan = state.services.loans.get_loan(loan_id).await?;
    let user_id = loan.user_id;
    let force = query.force.unwrap_or(false);

    if claims.rights.loans_rights.rank() < Rights::Write.rank() && user_id != claims.user_id {
        return Err(AppError::Authorization(
            "Insufficient rights to read loans for another user".into(),
        ));
    }
    if force {
        claims.require_write_loans()?;
    }

    let fingerprint = serde_json::json!({
        "op": "loans.renew",
        "loanId": loan_id.to_string(),
        "force": force,
    });

    let (_status, body) = idempotency::execute(
        state.services.repository.as_ref(),
        claims.user_id,
        idempotency_key.as_deref(),
        "loans.renew",
        &fingerprint,
        || {
            let state = state.clone();
            let ip = ip.clone();
            async move { renew_loan_inner(state, claims.user_id, ip, loan_id, force).await }
        },
    )
    .await?;
    Ok(Json(body))
}

async fn renew_loan_inner(
    state: crate::AppState,
    actor_id: i64,
    ip: Option<String>,
    loan_id: i64,
    force: bool,
) -> AppResult<(StatusCode, LoanResponse)> {
    let (new_expiry_date, renew_count) = state
        .services
        .loans
        .renew_loan(loan_id, force, Some(actor_id), ip.clone())
        .await?;

    state.services.audit.log(
        audit::event::LOAN_RENEWED,
        Some(actor_id),
        Some("loan"),
        Some(loan_id),
        ip,
        Some(RenewLoanAudit {
            new_expiry_at: new_expiry_date,
            renew_count,
        }),
        audit::AuditLogMeta::success(),
    );

    Ok((
        StatusCode::OK,
        LoanResponse {
            id: loan_id,
            expiry_at: new_expiry_date,
            message: format!("Loan renewed ({} renewals)", renew_count),
        },
    ))
}

/// Return a borrowed item by item identification (barcode or call number)
#[utoipa::path(
    post,
    path = "/loans/items/{item_id}/return",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("item_id" = String, Path, description = "Item barcode or call number")),
    responses(
        (status = 200, description = "Item returned", body = ReturnResponse),
        (status = 404, description = "Item or active loan not found"),
        (status = 409, description = "Already returned")
    )
)]
pub async fn return_loan_by_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<String>,
) -> AppResult<Json<ReturnResponse>> {
    claims.require_write_loans()?;
    match state
        .services
        .loans
        .return_loan_by_item(&item_id, Some(claims.user_id), ip.clone())
        .await
    {
        Ok(loan) => {
            let loan_id = loan.id;
            state.services.audit.log(
                audit::event::LOAN_RETURNED,
                Some(claims.user_id),
                Some("loan"),
                Some(loan_id),
                ip,
                Some((item_id.as_str(), &loan)),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(ReturnResponse {
                status: "returned".to_string(),
                loan,
            }))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::LOAN_RETURNED,
                Some(claims.user_id),
                Some("loan"),
                None,
                ip,
                Some(serde_json::json!({ "item_identification": item_id })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Renew a loan by item identification (barcode or call number)
#[utoipa::path(
    post,
    path = "/loans/items/{item_id}/renew",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(
        ("item_id" = String, Path, description = "Item barcode or call number"),
        RenewLoanQuery,
        ("Idempotency-Key" = Option<String>, Header, description = "Optional 1–255 char key (`A–Z a–z 0–9 _ - . ~`). Same actor+key+payload replays the stored 2xx for 24h. Same key with a different payload returns 409.")
    ),
    responses(
        (status = 200, description = "Loan renewed", body = LoanResponse),
        (status = 404, description = "Item or active loan not found"),
        (status = 409, description = "Idempotency-Key reused with a different payload"),
        (status = 422, description = "Business rule: max renewals reached, already returned, hold waiting, or unpaid fines exceed the threshold")
    )
)]
pub async fn renew_loan_by_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    OptionalIdempotencyKey(idempotency_key): OptionalIdempotencyKey,
    Path(item_id): Path<String>,
    Query(query): Query<RenewLoanQuery>,
) -> AppResult<Json<LoanResponse>> {
    claims.require_write_loans()?;
    let force = query.force.unwrap_or(false);
    let fingerprint = serde_json::json!({
        "op": "loans.renew_by_item",
        "itemId": item_id,
        "force": force,
    });

    let (_status, body) = idempotency::execute(
        state.services.repository.as_ref(),
        claims.user_id,
        idempotency_key.as_deref(),
        "loans.renew_by_item",
        &fingerprint,
        || {
            let state = state.clone();
            let ip = ip.clone();
            let item_id = item_id.clone();
            async move { renew_loan_by_item_inner(state, claims.user_id, ip, item_id, force).await }
        },
    )
    .await?;
    Ok(Json(body))
}

async fn renew_loan_by_item_inner(
    state: crate::AppState,
    actor_id: i64,
    ip: Option<String>,
    item_id: String,
    force: bool,
) -> AppResult<(StatusCode, LoanResponse)> {
    let (loan_id, new_expiry_date, renew_count) = state
        .services
        .loans
        .renew_loan_by_item(&item_id, force, Some(actor_id), ip.clone())
        .await?;

    state.services.audit.log(
        audit::event::LOAN_RENEWED,
        Some(actor_id),
        Some("loan"),
        Some(loan_id),
        ip,
        Some(RenewLoanByItemAudit {
            item_identification: item_id,
            new_expiry_at: new_expiry_date,
            renew_count,
        }),
        audit::AuditLogMeta::success(),
    );

    Ok((
        StatusCode::OK,
        LoanResponse {
            id: loan_id,
            expiry_at: new_expiry_date,
            message: format!("Loan renewed ({} renewals)", renew_count),
        },
    ))
}

/// Get all overdue loans (admin dashboard)
#[utoipa::path(
    get,
    path = "/loans/overdue",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(OverdueLoansQuery),
    responses(
        (status = 200, description = "Paginated overdue loans", body = OverdueLoansPage),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn get_overdue_loans(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<OverdueLoansQuery>,
) -> AppResult<Json<OverdueLoansPage>> {
    claims.require_read_loans()?;

    let page = state
        .services
        .reminders
        .get_overdue_loans(query.page.unwrap_or(1), query.per_page.unwrap_or(50))
        .await?;

    Ok(Json(page))
}

/// Trigger overdue reminder emails (admin only)
#[utoipa::path(
    post,
    path = "/loans/send-overdue-reminders",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(SendRemindersQuery),
    responses(
        (status = 200, description = "Reminder report", body = ReminderReport),
        (status = 403, description = "Insufficient permissions")
    )
)]
pub async fn send_overdue_reminders(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Query(query): Query<SendRemindersQuery>,
) -> AppResult<Json<ReminderReport>> {
    claims.require_admin()?;

    let dry_run = query.dry_run.unwrap_or(false);

    let report = state
        .services
        .reminders
        .send_overdue_reminders(dry_run, Some(claims.user_id), ip.clone())
        .await?;

    if !dry_run {
        state.services.audit.log(
            audit::event::SYSTEM_REMINDERS_BATCH_COMPLETED,
            Some(claims.user_id),
            None,
            None,
            ip,
            Some(ReminderBatchManualAudit {
                triggered_by: "manual",
                emails_sent: report.emails_sent,
                loans_reminded: report.loans_reminded,
                errors: report.errors.len(),
            }),
            audit::AuditLogMeta::success(),
        );
    }

    Ok(Json(report))
}

/// Mark the loan's copy lost: close the loan, item not borrowable, optional replacement bill.
#[utoipa::path(
    post,
    path = "/loans/{id}/lost",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    request_body = MarkLostRequest,
    responses(
        (status = 200, description = "Item marked lost", body = CirculationExceptionResponse),
        (status = 404, description = "Active loan not found"),
        (status = 422, description = "Invalid status transition or missing bill amount")
    )
)]
pub async fn mark_loan_lost(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(loan_id): Path<i64>,
    Json(body): Json<MarkLostRequest>,
) -> AppResult<Json<CirculationExceptionResponse>> {
    claims.require_write_loans()?;
    let result = state
        .services
        .circulation
        .mark_lost(loan_id, body, Some(claims.user_id), ip)
        .await?;
    Ok(Json(result))
}

/// Mark the loan's copy damaged. `disposition` chooses whether the loan is closed.
#[utoipa::path(
    post,
    path = "/loans/{id}/damaged",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    request_body = MarkDamagedRequest,
    responses(
        (status = 200, description = "Item marked damaged", body = CirculationExceptionResponse),
        (status = 404, description = "Active loan not found"),
        (status = 422, description = "Invalid status transition or missing damage fee")
    )
)]
pub async fn mark_loan_damaged(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(loan_id): Path<i64>,
    Json(body): Json<MarkDamagedRequest>,
) -> AppResult<Json<CirculationExceptionResponse>> {
    claims.require_write_loans()?;
    let result = state
        .services
        .circulation
        .mark_damaged(loan_id, body, Some(claims.user_id), ip)
        .await?;
    Ok(Json(result))
}

/// Flag the loan/item for the claims-returned queue. Does not close the loan.
#[utoipa::path(
    post,
    path = "/loans/{id}/claimed-returned",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    request_body = MarkClaimedReturnedRequest,
    responses(
        (status = 200, description = "Flagged as claimed returned", body = CirculationExceptionResponse),
        (status = 404, description = "Active loan not found"),
        (status = 422, description = "Invalid status transition")
    )
)]
pub async fn mark_loan_claimed_returned(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(loan_id): Path<i64>,
    Json(body): Json<MarkClaimedReturnedRequest>,
) -> AppResult<Json<CirculationExceptionResponse>> {
    claims.require_write_loans()?;
    let result = state
        .services
        .circulation
        .mark_claimed_returned(loan_id, body, Some(claims.user_id), ip)
        .await?;
    Ok(Json(result))
}

/// Resolve a claims-returned case after an inventory check. Never bills.
#[utoipa::path(
    post,
    path = "/loans/{id}/claims-returned/resolve",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    request_body = ResolveClaimsReturnedRequest,
    responses(
        (status = 200, description = "Claim resolved (no charge)", body = CirculationExceptionResponse),
        (status = 404, description = "Active loan not found"),
        (status = 422, description = "Inventory check required, billing rejected, or invalid transition")
    )
)]
pub async fn resolve_loan_claims_returned(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(loan_id): Path<i64>,
    Json(body): Json<ResolveClaimsReturnedRequest>,
) -> AppResult<Json<CirculationExceptionResponse>> {
    claims.require_write_loans()?;
    let result = state
        .services
        .circulation
        .resolve_claims_returned(loan_id, body, Some(claims.user_id), ip)
        .await?;
    Ok(Json(result))
}

/// Staff work queue of items/loans flagged claimed-returned.
#[utoipa::path(
    get,
    path = "/loans/claims-returned",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(OverdueLoansQuery),
    responses(
        (status = 200, description = "Claims-returned queue", body = PaginatedResponse<ClaimsReturnedQueueItem>),
        (status = 403, description = "Insufficient loans write rights")
    )
)]
pub async fn list_claims_returned(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<OverdueLoansQuery>,
) -> AppResult<Json<PaginatedResponse<ClaimsReturnedQueueItem>>> {
    claims.require_write_loans()?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 200);
    let (items, total) = state
        .services
        .circulation
        .list_claims_returned(page, per_page)
        .await?;
    Ok(Json(PaginatedResponse::new(items, total, page, per_page)))
}
