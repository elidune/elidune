//! Batch operations API — bulk loan returns and creations for scanner workflows

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use serde_with::{serde_as, DisplayFromStr};

use crate::{
    error::AppResult,
    models::{dto::circulation::LostFineDeskStatus, loan::LoanDetails},
    services::audit,
};

use super::{AuthenticatedUser, ClientIp};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::post;
    axum::Router::new()
        .route("/loans/batch-return", post(batch_return))
        .route("/loans/batch-create", post(batch_create_loans))
}

/// Batch return request — list of barcodes to return
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchReturnRequest {
    /// List of specimen barcodes to return
    pub barcodes: Vec<String>,
}

/// Result for a single barcode in a batch operation.
/// A lost copy sets `wasLost` and does not change the lost status.
#[serde_as]
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchReturnItemResult {
    pub barcode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan: Option<LoanDetails>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub was_lost: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fine_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_status: Option<LostFineDeskStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fine_amount: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_amount: Option<i64>,
}

/// Batch return response
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchReturnResponse {
    pub returned: u32,
    pub errors: u32,
    pub results: Vec<BatchReturnItemResult>,
}

/// Batch return by barcodes — for scanner return stations
///
/// Returns all items in the list. Per-barcode errors are collected
/// and returned inline (partial success is possible).
#[utoipa::path(
    post,
    path = "/loans/batch-return",
    tag = "loans",
    security(("bearer_auth" = [])),
    request_body = BatchReturnRequest,
    responses(
        (status = 200, description = "Batch return results (partial success possible)", body = BatchReturnResponse),
        (status = 400, description = "Empty list or invalid input", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn batch_return(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(req): Json<BatchReturnRequest>,
) -> AppResult<Json<BatchReturnResponse>> {
    claims.require_write_loans()?;

    if req.barcodes.is_empty() {
        return Err(crate::error::AppError::Validation(
            "barcodes list cannot be empty".to_string(),
        ));
    }

    let mut results = Vec::with_capacity(req.barcodes.len());
    let mut returned = 0u32;
    let mut errors = 0u32;

    for barcode in &req.barcodes {
        match state
            .services
            .loans
            .return_loan_by_item(barcode, Some(claims.user_id), ip.clone())
            .await
        {
            Ok(check_in) => {
                if let Some(ref loan) = check_in.loan {
                    state.services.audit.log(
                        audit::event::LOAN_RETURNED,
                        Some(claims.user_id),
                        Some("loan"),
                        Some(loan.id),
                        ip.clone(),
                        Some(serde_json::json!({ "barcode": barcode, "batch": true })),
                        audit::AuditLogMeta::success(),
                    );
                }
                returned += 1;
                let lost = check_in.lost.as_ref();
                results.push(BatchReturnItemResult {
                    barcode: barcode.clone(),
                    loan: check_in.loan,
                    success: true,
                    error: None,
                    was_lost: lost.map(|l| l.was_lost),
                    fine_id: lost.and_then(|l| l.fine_id),
                    fine_status: lost.and_then(|l| l.fine_status),
                    fine_amount: lost.and_then(|l| l.fine_amount),
                    paid_amount: lost.and_then(|l| l.paid_amount),
                });
            }
            Err(e) => {
                errors += 1;
                state.services.audit.log(
                    audit::event::LOAN_RETURNED,
                    Some(claims.user_id),
                    Some("loan"),
                    None,
                    ip.clone(),
                    Some(serde_json::json!({ "barcode": barcode, "batch": true })),
                    audit::AuditLogMeta::from_app_error(&e),
                );
                results.push(BatchReturnItemResult {
                    barcode: barcode.clone(),
                    loan: None,
                    success: false,
                    error: Some(e.to_string()),
                    was_lost: None,
                    fine_id: None,
                    fine_status: None,
                    fine_amount: None,
                    paid_amount: None,
                });
            }
        }
    }

    Ok(Json(BatchReturnResponse {
        returned,
        errors,
        results,
    }))
}

/// Batch loan creation request
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateLoanItem {
    pub barcode: String,
}

/// Batch create loans request — assign multiple items to the same user
#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateLoansRequest {
    pub user_id: String,
    pub barcodes: Vec<String>,
    /// Same checkout override as `POST /loans`. Never overrides an open loan
    /// (reserved-item override is tracked in #167).
    #[serde(default)]
    pub force: bool,
}

/// Batch create response
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateLoansResponse {
    pub created: u32,
    pub errors: u32,
    pub results: Vec<BatchCreateLoanItemResult>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateLoanItemResult {
    pub barcode: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loan_id: Option<String>,
    /// Human-readable failure text. Omitted when the checkout succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Machine-readable failure code, the same string as `ErrorResponse.code`
    /// (`item_on_loan_to_other_borrower`, `item_already_on_loan_to_borrower`,
    /// `not_found`, `business_rule_violation`, …). Omitted when the checkout succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Batch create loans — check out multiple items for one patron at once
#[utoipa::path(
    post,
    path = "/loans/batch-create",
    tag = "loans",
    security(("bearer_auth" = [])),
    request_body = BatchCreateLoansRequest,
    responses(
        (status = 200, description = "Batch loan results (partial success possible). A failed item keeps `error` and adds machine-readable `code` (same values as `ErrorResponse.code`).", body = BatchCreateLoansResponse),
        (status = 400, description = "Invalid input", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn batch_create_loans(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(req): Json<BatchCreateLoansRequest>,
) -> AppResult<Json<BatchCreateLoansResponse>> {
    claims.require_write_loans()?;

    let user_id: i64 = req
        .user_id
        .parse()
        .map_err(|_| crate::error::AppError::Validation("Invalid userId format".to_string()))?;

    if req.barcodes.is_empty() {
        return Err(crate::error::AppError::Validation(
            "barcodes list cannot be empty".to_string(),
        ));
    }

    let mut results = Vec::with_capacity(req.barcodes.len());
    let mut created = 0u32;
    let mut errors = 0u32;

    for barcode in &req.barcodes {
        let loan_data = crate::models::loan::CreateLoan {
            user_id,
            item_id: None,
            item_identification: Some(barcode.clone()),
            force: req.force,
        };
        match state
            .services
            .loans
            .create_loan(loan_data, Some(claims.user_id), ip.clone())
            .await
        {
            Ok(outcome) => {
                let loan_id = outcome.loan_id;
                let expiry_at = outcome.expiry_at;
                state.services.audit.log(
                    audit::event::LOAN_CREATED,
                    Some(claims.user_id),
                    Some("loan"),
                    Some(loan_id),
                    ip.clone(),
                    Some(serde_json::json!({ "barcode": barcode, "userId": user_id.to_string(), "batch": true, "expiryAt": expiry_at })),
                    audit::AuditLogMeta::success(),
                );
                created += 1;
                results.push(BatchCreateLoanItemResult {
                    barcode: barcode.clone(),
                    success: true,
                    loan_id: Some(loan_id.to_string()),
                    error: None,
                    code: None,
                });
            }
            Err(e) => {
                errors += 1;
                let (_, code, _) = e.audit_http_fields();
                state.services.audit.log(
                    audit::event::LOAN_CREATED,
                    Some(claims.user_id),
                    Some("loan"),
                    None,
                    ip.clone(),
                    Some(serde_json::json!({ "barcode": barcode, "userId": user_id.to_string(), "batch": true })),
                    audit::AuditLogMeta::from_app_error(&e),
                );
                results.push(BatchCreateLoanItemResult {
                    barcode: barcode.clone(),
                    success: false,
                    loan_id: None,
                    error: Some(e.to_string()),
                    code: Some(code.to_string()),
                });
            }
        }
    }

    Ok(Json(BatchCreateLoansResponse {
        created,
        errors,
        results,
    }))
}
