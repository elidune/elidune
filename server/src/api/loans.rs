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
        catalog_view::{LoanDetailsPublic, LoanDetailsStaff, UserShortStaff},
        loan::{CreateLoan, LoanDetails, LoanMarcExportEncoding, LoanMarcExportFormat},
        my_account::MyLoanView,
        user::Rights,
    },
    services::{
        audit::{self},
        idempotency,
        reminders::{OverdueLoansPage, ReminderReport},
    },
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, OptionalIdempotencyKey};

pub use crate::models::dto::circulation::{
    CirculationExceptionResponse, ClaimsReturnedQueueItem, LostFineDeskStatus, LostFoundResolution,
    MarkClaimedReturnedRequest, MarkDamagedRequest, MarkLostRequest, ResolveClaimsReturnedRequest,
    ResolveLostFoundRequest,
};
pub use crate::models::dto::loans::{LoanSettingsDto as LoanSettings, UpdateLoanSettingsRequest};

/// Build the loans routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/me/loans", get(list_my_loans))
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
        .route("/loans/items/:item_id/lost-found", post(resolve_lost_found))
        .route("/loans/items/:item_id/renew", post(renew_loan_by_item))
}

/// Borrower of an active loan.
///
/// Callers with `users_rights` read or write receive [`UserShortStaff`].
/// Everyone else who may call this route (`loans_rights` write only) receives
/// [`LoanBorrowerMinimized`]. `loans_rights` does not unlock the staff profile.
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum LoanBorrowerResponse {
    Full(UserShortStaff),
    Minimized(LoanBorrowerMinimized),
}

/// Id, library card, and initials. No name, login, email, phone, or address.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoanBorrowerMinimized {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// `users.barcode`. Null when the patron has no card on file.
    pub card_number: Option<String>,
    /// Uppercase first letter of `firstname` plus first letter of `lastname`.
    /// A missing or blank part is skipped.
    pub initials: String,
}

impl From<crate::models::user::UserShort> for LoanBorrowerMinimized {
    fn from(user: crate::models::user::UserShort) -> Self {
        Self {
            id: user.id,
            card_number: user.barcode.filter(|value| !value.trim().is_empty()),
            initials: borrower_initials(user.firstname.as_deref(), user.lastname.as_deref()),
        }
    }
}

/// Uppercase first letter of each name part. Blank and missing parts are skipped.
fn borrower_initials(firstname: Option<&str>, lastname: Option<&str>) -> String {
    let mut initials = String::new();
    for part in [firstname, lastname] {
        let Some(ch) = part.and_then(|value| value.trim().chars().next()) else {
            continue;
        };
        initials.extend(ch.to_uppercase());
    }
    initials
}

/// Get the patron linked to an active loan (circulation desk).
#[utoipa::path(
    get,
    path = "/loans/{id}/user",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Active loan ID")),
    responses(
        (status = 200, description = "Borrower identity. `users_rights` read or write: `UserShortStaff` (includes `login` and `barcode`). Otherwise only `id`, `cardNumber`, and `initials`. `loans_rights` write does not return the staff profile.", body = LoanBorrowerResponse),
        (status = 403, description = "Insufficient loans write rights"),
        (status = 404, description = "Loan not found")
    )
)]
pub async fn get_loan_borrower(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(loan_id): Path<i64>,
) -> AppResult<Json<LoanBorrowerResponse>> {
    claims.require_write_loans()?;
    let user = state.services.loans.get_loan_borrower(loan_id).await?;
    // `loans_rights` write is enough to call this route. It is not enough to
    // read the patron's name. `users_rights` own is below read.
    let body = if claims.rights.users_rights.rank() >= Rights::Read.rank() {
        LoanBorrowerResponse::Full(UserShortStaff::from(user))
    } else {
        LoanBorrowerResponse::Minimized(LoanBorrowerMinimized::from(user))
    };
    Ok(Json(body))
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
    /// When true, bypasses patron/subscription/limits/unpaid-fine checks and hold-queue rules.
    /// Pinned `pending`/`ready` holds on the copy are cancelled. A `to_pick` hold is released
    /// back to `pending`, or reassigned to another shelf copy.
    ///
    /// Never overrides an open loan: a copy already on loan returns 409
    /// (`item_on_loan_to_other_borrower` or `item_already_on_loan_to_borrower`)
    /// whether or not this flag is set, including for admins. Reserved-item override
    /// is tracked in #167.
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

/// Return response. `loan` is set when a loan was closed. `hold` is set when a
/// shelf `to_pick` copy was checked in or a missing copy was found.
/// `status` is `returned`, `lost_found`, `ready`, `in_transit`, or `available`.
/// `destinationSiteId` / `destinationSiteName` are set when status is `in_transit`.
///
/// When the copy is lost, `wasLost` is true and the replacement charge (if any)
/// is described in cents. The lost status itself is not changed.
#[serde_as]
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReturnResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan: Option<LoanDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<crate::models::hold::Hold>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub destination_site_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_site_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub was_lost: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fine_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_status: Option<LostFineDeskStatus>,
    /// Replacement charge in cents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_amount: Option<i64>,
    /// Amount already collected, in cents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_amount: Option<i64>,
}

fn apply_lost(
    response: &mut ReturnResponse,
    lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
) {
    let Some(lost) = lost else {
        return;
    };
    response.was_lost = Some(lost.was_lost);
    response.fine_id = lost.fine_id;
    response.fine_status = lost.fine_status;
    response.fine_amount = lost.fine_amount;
    response.paid_amount = lost.paid_amount;
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

/// Staff or reader loan page. Selected with [`crate::models::user::UserClaims::is_librarian`].
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum UserLoanListResponse {
    Staff(PaginatedResponse<LoanDetailsStaff>),
    Public(PaginatedResponse<LoanDetailsPublic>),
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
        (status = 200, description = "User's loans. Librarians and admins receive `LoanDetailsStaff` (copy weeding, full availability, patron `login`). Readers receive `LoanDetailsPublic`.", body = UserLoanListResponse),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user_loans(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(user_id): Path<i64>,
    Query(query): Query<GetUserLoansQuery>,
) -> AppResult<Json<UserLoanListResponse>> {
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

    let body = if claims.is_librarian() {
        UserLoanListResponse::Staff(PaginatedResponse::new(
            items.into_iter().map(LoanDetailsStaff::from).collect(),
            total,
            page,
            per_page,
        ))
    } else {
        UserLoanListResponse::Public(PaginatedResponse::new(
            items.into_iter().map(LoanDetailsPublic::from).collect(),
            total,
            page,
            per_page,
        ))
    };
    Ok(Json(body))
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

/// Page query for `GET /me/loans`. Always the authenticated patron; no user id.
#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct MyLoansQuery {
    /// Page number (1-based, default 1)
    pub page: Option<i64>,
    /// Page size (default 20, max 200)
    pub per_page: Option<i64>,
}

/// The caller's active loans. Filtered in SQL by `user_id = caller`.
#[utoipa::path(
    get,
    path = "/me/loans",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(MyLoansQuery),
    responses(
        (status = 200, description = "The caller's active loans (`MyLoanView`)", body = PaginatedResponse<MyLoanView>),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn list_my_loans(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<MyLoansQuery>,
) -> AppResult<Json<PaginatedResponse<MyLoanView>>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 200);
    let (items, total) = state
        .services
        .repository
        .loans_list_for_owner(claims.user_id, page, per_page)
        .await?;
    Ok(Json(PaginatedResponse::new(items, total, page, per_page)))
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
        (status = 409, description = "Open loan on this copy (`item_on_loan_to_other_borrower` or `item_already_on_loan_to_borrower`; `force` never overrides an open loan), or Idempotency-Key conflict"),
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
        (status = 200, description = "Item returned. A lost copy adds wasLost, and fineId, fineStatus (unpaid|paid|partial), fineAmount, and paidAmount (cents) when a replacement charge is still open. Lost status is not changed.", body = ReturnResponse),
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
    let returned = match state
        .services
        .loans
        .return_loan(loan_id, Some(claims.user_id), ip.clone())
        .await
    {
        Ok(returned) => returned,
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

    if let Some(ref loan) = returned.loan {
        state.services.audit.log(
            audit::event::LOAN_RETURNED,
            Some(claims.user_id),
            Some("loan"),
            Some(loan_id),
            ip,
            Some(loan),
            audit::AuditLogMeta::success(),
        );
    }

    let mut response = ReturnResponse {
        status: if returned.loan.is_some() {
            "returned"
        } else {
            "lost_found"
        }
        .to_string(),
        loan: returned.loan,
        hold: None,
        destination_site_id: None,
        destination_site_name: None,
        was_lost: None,
        fine_id: None,
        fine_status: None,
        fine_amount: None,
        paid_amount: None,
    };
    apply_lost(&mut response, returned.lost);
    Ok(Json(response))
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
        (status = 422, description = "Renewal refused. `code` is `account_blocked`, `fines_over_threshold`, `withdrawn`, `claimed_returned`, `hold_waiting`, `max_renewals`, `renewal_too_early`, or `business_rule_violation` when the loan is already returned or the due date would not move")
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

/// Check in a barcode. Returns an active loan, or completes a `to_pick` hold on a shelf copy.
#[utoipa::path(
    post,
    path = "/loans/items/{item_id}/return",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("item_id" = String, Path, description = "Item barcode or call number")),
    responses(
        (status = 200, description = "Item returned, or a to_pick hold checked in", body = ReturnResponse),
        (status = 404, description = "Unknown barcode (`not_found`), or a known item that is not on loan (`item_not_on_loan`)", body = crate::error::ErrorResponse),
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
        Ok(check_in) => {
            if let Some(ref loan) = check_in.loan {
                let loan_id = loan.id;
                state.services.audit.log(
                    audit::event::LOAN_RETURNED,
                    Some(claims.user_id),
                    Some("loan"),
                    Some(loan_id),
                    ip,
                    Some((item_id.as_str(), loan)),
                    audit::AuditLogMeta::success(),
                );
            }
            let status = if check_in.lost.is_some() && check_in.loan.is_none() {
                "lost_found"
            } else if check_in.loan.is_some() {
                "returned"
            } else if check_in.transit_started {
                "in_transit"
            } else if check_in.hold.is_some() {
                "ready"
            } else {
                "available"
            };
            let mut response = ReturnResponse {
                status: status.to_string(),
                loan: check_in.loan,
                hold: check_in.hold,
                destination_site_id: check_in.destination_site_id,
                destination_site_name: check_in.destination_site_name,
                was_lost: None,
                fine_id: None,
                fine_status: None,
                fine_amount: None,
                paid_amount: None,
            };
            apply_lost(&mut response, check_in.lost);
            Ok(Json(response))
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

/// Resolve a lost copy that was found at the desk.
///
/// `circulate` puts it back on the shelf and serves holds. An unpaid replacement
/// charge is cancelled, a paid one is refunded, and a partial payment is
/// refunded with the remainder cancelled. Nothing is refunded that was not collected.
/// `damaged` sets the item status to damaged (not lendable) and leaves the
/// replacement charge exactly as it is: `fineAction` is `unchanged`, and both
/// `refundedAmount` and `cancelledAmount` are zero. No `fine.cancelled` or
/// `fine.refunded` event is written. A waiver is a separate explicit action
/// with a reason.
/// `leave` does not change the lost status or the fee.
#[utoipa::path(
    post,
    path = "/loans/items/{item_id}/lost-found",
    tag = "loans",
    security(("bearer_auth" = [])),
    params(("item_id" = i64, Path, description = "Item ID")),
    request_body = ResolveLostFoundRequest,
    responses(
        (status = 200, description = "Lost copy resolved", body = LostFoundResolution),
        (status = 403, description = "Insufficient loans write rights"),
        (status = 404, description = "Item not found"),
        (status = 422, description = "Item is not marked lost")
    )
)]
pub async fn resolve_lost_found(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(item_id): Path<i64>,
    Json(body): Json<ResolveLostFoundRequest>,
) -> AppResult<Json<LostFoundResolution>> {
    claims.require_write_loans()?;
    let result = state
        .services
        .circulation
        .resolve_lost_found(item_id, body, Some(claims.user_id), ip)
        .await?;
    Ok(Json(result))
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
        (status = 422, description = "Renewal refused. `code` is `account_blocked`, `fines_over_threshold`, `withdrawn`, `claimed_returned`, `hold_waiting`, `max_renewals`, `renewal_too_early`, or `business_rule_violation` when the loan is already returned or the due date would not move")
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

#[cfg(test)]
mod borrower_identity_tests {
    use super::borrower_initials;

    #[test]
    fn initials_use_the_first_letter_of_each_present_part() {
        assert_eq!(borrower_initials(Some("ada"), Some("lovelace")), "AL");
        assert_eq!(borrower_initials(Some(" Ada "), None), "A");
        assert_eq!(borrower_initials(None, Some("  lovelace")), "L");
        assert_eq!(borrower_initials(Some("   "), Some("")), "");
        assert_eq!(borrower_initials(Some("émile"), Some("zola")), "ÉZ");
    }
}
