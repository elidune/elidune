//! Error types for Elidune server

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

use crate::models::biblio::BiblioShort;
use crate::models::item::ItemShort;

/// Machine-readable string error codes used in API responses.
///
/// Using string codes instead of legacy integers makes the API self-documenting
/// and avoids tight coupling with a specific numbering scheme.
pub mod error_code {
    pub const AUTHENTICATION: &str = "authentication_failed";
    pub const AUTHORIZATION: &str = "authorization_failed";
    pub const NOT_FOUND: &str = "not_found";
    pub const GONE: &str = "gone";
    pub const VALIDATION: &str = "validation_error";
    /// `library_info.timezone` is empty or not an IANA name known to chrono-tz.
    pub const INVALID_TIMEZONE: &str = "invalid_timezone";
    /// Timezone is a known IANA name but not one of the country's zones.
    pub const TIMEZONE_COUNTRY_MISMATCH: &str = "timezone_country_mismatch";
    /// A country with several zones was saved without choosing one.
    pub const TIMEZONE_REQUIRED: &str = "timezone_required";
    pub const DATABASE: &str = "database_error";
    pub const CONFLICT: &str = "conflict";
    pub const BAD_REQUEST: &str = "bad_request";
    pub const INTERNAL: &str = "internal_error";
    pub const Z3950: &str = "z3950_error";
    pub const BUSINESS_RULE: &str = "business_rule_violation";
    /// Renewal requested before `due date - renewWindowDays`.
    pub const RENEWAL_TOO_EARLY: &str = "renewal_too_early";
    pub const DUPLICATE_ISBN: &str = "duplicate_isbn_needs_confirmation";
    pub const DUPLICATE_BARCODE: &str = "duplicate_barcode_needs_confirmation";
    pub const RATE_LIMITED: &str = "rate_limited";
    /// Checkout of a copy that is `to_pick` for a different patron.
    pub const ITEM_TO_PICK_FOR_OTHER: &str = "item_to_pick_for_other";
    /// Check-in of a known copy that has no active loan.
    pub const ITEM_NOT_ON_LOAN: &str = "item_not_on_loan";
    /// Checkout refused: the copy already has an open loan for a different borrower.
    pub const ITEM_ON_LOAN_TO_OTHER_BORROWER: &str = "item_on_loan_to_other_borrower";
    /// Checkout refused: the copy already has an open loan for this borrower. Renew it.
    pub const ITEM_ALREADY_ON_LOAN_TO_BORROWER: &str = "item_already_on_loan_to_borrower";
    /// Delete refused because the copy has a loan, hold, or fine. Weed it instead.
    pub const ITEM_HAS_HISTORY_USE_WEEDING: &str = "item_has_history_use_weeding";
    /// Delete refused because the bibliographic record still has copies.
    pub const BIBLIO_HAS_ITEMS: &str = "biblio_has_items";
    /// Delete refused because the bibliographic record has holds.
    pub const BIBLIO_HAS_HOLDS: &str = "biblio_has_holds";
    pub const ITEM_ON_LOAN: &str = "item_on_loan";
    pub const ITEM_IN_TRANSIT: &str = "item_in_transit";
    pub const ITEM_ON_HOLD: &str = "item_on_hold";
    pub const WITHDRAWAL_REASON_DETAIL_REQUIRED: &str = "withdrawal_reason_detail_required";
    pub const WITHDRAWAL_DESTINATION_REQUIRED: &str = "withdrawal_destination_required";
    pub const WITHDRAWAL_ALREADY_ACTIVE: &str = "withdrawal_already_active";
    pub const WITHDRAWAL_NOT_ACTIVE: &str = "withdrawal_not_active";
    pub const WITHDRAWAL_NOT_LEGACY: &str = "withdrawal_not_legacy";
    /// `POST /items/{id}/weeding` with `status: withdrawn`.
    pub const WITHDRAWN_VIA_WITHDRAWAL: &str = "withdrawn_via_withdrawal";
    /// Leaving `withdrawn` requires cancelling the active withdrawal.
    pub const WITHDRAWAL_USE_CANCEL: &str = "withdrawal_use_cancel";
    /// Cancel or dispose of a withdrawal whose disposal is already recorded.
    pub const WITHDRAWAL_ALREADY_DISPOSED: &str = "withdrawal_already_disposed";
    /// Batch disposal rejected. The body lists one reason per rejected id.
    pub const WITHDRAWAL_DISPOSE_REJECTED: &str = "withdrawal_dispose_rejected";
}

/// Main application error type
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Authorization failed: {0}")]
    Authorization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Gone: {0}")]
    Gone(String),

    #[error("Validation error: {0}")]
    Validation(String),

    /// Library timezone is not a known IANA name (HTTP 400, `invalid_timezone`).
    #[error("{0}")]
    InvalidTimezone(String),

    /// Timezone is not one of the selected country's zones (HTTP 400, `timezone_country_mismatch`).
    #[error("{0}")]
    TimezoneCountryMismatch(String),

    /// Multi-zone country saved without a timezone (HTTP 400, `timezone_required`).
    #[error("{0}")]
    TimezoneRequired(String),

    /// Boxed so `AppError` stays small enough for `Result<T, AppError>`
    /// (`clippy::result_large_err`, 128-byte threshold).
    #[error("Database error: {0}")]
    Database(Box<sqlx::Error>),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Z39.50 error: {0}")]
    Z3950(String),

    #[error("Business rule violation: {0}")]
    BusinessRule(String),

    /// Renewal is outside the configured window before the due date (HTTP 422, `renewal_too_early`).
    #[error("{0}")]
    RenewalTooEarly(String),

    /// Renewal refused for a stable code shared with `GET /me/loans` (HTTP 422).
    #[error("{message}")]
    RenewalRefused { code: &'static str, message: String },

    /// Confirmation payload is boxed: `BiblioShort` is far larger than the
    /// `result_large_err` limit and would otherwise inflate every `AppResult`.
    #[error("Duplicate ISBN requires confirmation")]
    DuplicateNeedsConfirmation {
        existing_id: i64,
        existing_item: Box<BiblioShort>,
        message: String,
    },

    #[error("Duplicate barcode requires confirmation")]
    DuplicateBarcodeNeedsConfirmation {
        existing_id: i64,
        existing_item: Box<ItemShort>,
        message: String,
    },

    /// Temporary auth throttle. `retry_after` is seconds for `Retry-After`.
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    /// Copy is on the shelf-pick list for another patron. `force=true` still overrides.
    #[error("This copy is on the pick list for another patron")]
    ItemToPickForOther {
        hold_id: i64,
        user_id: i64,
        user_name: String,
    },

    /// Check-in of a known copy that is not on loan (HTTP 404, `item_not_on_loan`).
    #[error("{0}")]
    ItemNotOnLoan(String),

    /// Stable `error_code` with an explicit HTTP status (400, 409, or 422).
    #[error("{message}")]
    Coded {
        status: u16,
        code: &'static str,
        message: String,
    },

    /// Batch disposal applied to none of the ids. `failures` is one reason per rejected id.
    #[error("No withdrawal was marked disposed")]
    WithdrawalDisposeRejected {
        status: u16,
        failures: Box<Vec<crate::models::withdrawal::WithdrawalDisposeFailure>>,
    },
}

impl AppError {
    #[must_use]
    pub fn coded(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self::Coded {
            status,
            code,
            message: message.into(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        Self::Database(Box::new(err))
    }
}

/// Error response body returned for all API errors.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    /// Machine-readable error code (e.g. `"not_found"`, `"validation_error"`)
    pub code: String,
    /// Human-readable error category
    pub error: String,
    /// Detailed error message
    pub message: String,
}

/// `429` body. Same fields as [`ErrorResponse`] plus `retryAfter` seconds.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitedBody {
    /// Always `"rate_limited"`.
    pub code: String,
    /// Human-readable category (`"Too Many Requests"`).
    pub error: String,
    /// How long the client should wait, in words.
    pub message: String,
    /// Seconds until the client may retry. Matches the `Retry-After` header.
    pub retry_after: u64,
}

/// `429` with `Retry-After` and a JSON body `{ code: "rate_limited", retryAfter }`.
pub fn rate_limited_response(retry_after_secs: u64) -> Response {
    let secs = retry_after_secs.max(1);
    let body = Json(RateLimitedBody {
        code: error_code::RATE_LIMITED.to_string(),
        error: "Too Many Requests".to_string(),
        message: format!("Too many attempts. Try again in {secs} seconds."),
        retry_after: secs,
    });
    let mut response = (StatusCode::TOO_MANY_REQUESTS, body).into_response();
    if let Ok(value) = HeaderValue::from_str(&secs.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

/// Patron waiting on a `to_pick` copy, for the desk warning.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemToPickDetails {
    pub hold_id: String,
    pub user_id: String,
    pub user_name: String,
}

/// Checkout refused because the copy is `to_pick` for someone else.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemToPickForOtherResponse {
    pub code: String,
    pub error: String,
    pub message: String,
    pub details: ItemToPickDetails,
}

struct HttpErrorFields {
    status: StatusCode,
    code: &'static str,
    label: &'static str,
    message: String,
}

impl AppError {
    fn http_fields(&self) -> HttpErrorFields {
        use error_code as ec;

        match self {
            AppError::Authentication(msg) => HttpErrorFields {
                status: StatusCode::UNAUTHORIZED,
                code: ec::AUTHENTICATION,
                label: "Unauthorized",
                message: msg.clone(),
            },
            AppError::Authorization(msg) => HttpErrorFields {
                status: StatusCode::FORBIDDEN,
                code: ec::AUTHORIZATION,
                label: "Forbidden",
                message: msg.clone(),
            },
            AppError::NotFound(msg) => HttpErrorFields {
                status: StatusCode::NOT_FOUND,
                code: ec::NOT_FOUND,
                label: "Not Found",
                message: msg.clone(),
            },
            AppError::Gone(msg) => HttpErrorFields {
                status: StatusCode::GONE,
                code: ec::GONE,
                label: "Gone",
                message: msg.clone(),
            },
            AppError::Validation(msg) => HttpErrorFields {
                status: StatusCode::BAD_REQUEST,
                code: ec::VALIDATION,
                label: "Validation Error",
                message: msg.clone(),
            },
            AppError::InvalidTimezone(msg) => HttpErrorFields {
                status: StatusCode::BAD_REQUEST,
                code: ec::INVALID_TIMEZONE,
                label: "Validation Error",
                message: msg.clone(),
            },
            AppError::TimezoneCountryMismatch(msg) => HttpErrorFields {
                status: StatusCode::BAD_REQUEST,
                code: ec::TIMEZONE_COUNTRY_MISMATCH,
                label: "Validation Error",
                message: msg.clone(),
            },
            AppError::TimezoneRequired(msg) => HttpErrorFields {
                status: StatusCode::BAD_REQUEST,
                code: ec::TIMEZONE_REQUIRED,
                label: "Validation Error",
                message: msg.clone(),
            },
            AppError::Database(e) => {
                tracing::error!("Database error: {:?}", e);
                HttpErrorFields {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: ec::DATABASE,
                    label: "Internal Server Error",
                    message: "A database error occurred".to_string(),
                }
            }
            AppError::Conflict(msg) => HttpErrorFields {
                status: StatusCode::CONFLICT,
                code: ec::CONFLICT,
                label: "Conflict",
                message: msg.clone(),
            },
            AppError::BadRequest(msg) => HttpErrorFields {
                status: StatusCode::BAD_REQUEST,
                code: ec::BAD_REQUEST,
                label: "Bad Request",
                message: msg.clone(),
            },
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                HttpErrorFields {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: ec::INTERNAL,
                    label: "Internal Server Error",
                    message: "An unexpected error occurred".to_string(),
                }
            }
            AppError::Z3950(msg) => HttpErrorFields {
                status: StatusCode::BAD_GATEWAY,
                code: ec::Z3950,
                label: "Z39.50 Error",
                message: msg.clone(),
            },
            AppError::BusinessRule(msg) => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: ec::BUSINESS_RULE,
                label: "Business Rule Violation",
                message: msg.clone(),
            },
            AppError::RenewalTooEarly(msg) => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: ec::RENEWAL_TOO_EARLY,
                label: "Business Rule Violation",
                message: msg.clone(),
            },
            AppError::RenewalRefused { code, message } => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code,
                label: "Business Rule Violation",
                message: message.clone(),
            },
            AppError::DuplicateNeedsConfirmation { .. }
            | AppError::DuplicateBarcodeNeedsConfirmation { .. } => HttpErrorFields {
                status: StatusCode::CONFLICT,
                code: ec::CONFLICT,
                label: "Conflict",
                message: String::new(),
            },
            AppError::RateLimited { retry_after } => HttpErrorFields {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: ec::RATE_LIMITED,
                label: "Too Many Requests",
                message: format!("Too many attempts. Try again in {retry_after} seconds."),
            },
            AppError::ItemToPickForOther { .. } => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: ec::ITEM_TO_PICK_FOR_OTHER,
                label: "Business Rule Violation",
                message:
                    "This copy is on the pick list for another patron — use force=true to override"
                        .to_string(),
            },
            AppError::ItemNotOnLoan(msg) => HttpErrorFields {
                status: StatusCode::NOT_FOUND,
                code: ec::ITEM_NOT_ON_LOAN,
                label: "Not Found",
                message: msg.clone(),
            },
            AppError::Coded {
                status,
                code,
                message,
            } => HttpErrorFields {
                status: StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                code,
                label: match *status {
                    400 => "Validation Error",
                    409 => "Conflict",
                    _ => "Business Rule Violation",
                },
                message: message.clone(),
            },
            AppError::WithdrawalDisposeRejected { status, .. } => HttpErrorFields {
                status: StatusCode::from_u16(*status).unwrap_or(StatusCode::CONFLICT),
                code: ec::WITHDRAWAL_DISPOSE_REJECTED,
                label: if *status == 409 {
                    "Conflict"
                } else {
                    "Business Rule Violation"
                },
                message: "No withdrawal was marked disposed".to_string(),
            },
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        use error_code as ec;

        match self {
            AppError::DuplicateNeedsConfirmation {
                existing_id,
                existing_item,
                message,
            } => {
                let body = Json(
                    crate::models::import_report::DuplicateConfirmationRequired {
                        code: ec::DUPLICATE_ISBN.to_string(),
                        existing_id,
                        existing_biblio: *existing_item,
                        message,
                    },
                );
                (StatusCode::CONFLICT, body).into_response()
            }
            AppError::DuplicateBarcodeNeedsConfirmation {
                existing_id,
                existing_item,
                message,
            } => {
                let body = Json(crate::models::import_report::DuplicateItemBarcodeRequired {
                    code: ec::DUPLICATE_BARCODE.to_string(),
                    existing_id,
                    existing_item: *existing_item,
                    message,
                });
                (StatusCode::CONFLICT, body).into_response()
            }
            AppError::RateLimited { retry_after } => rate_limited_response(retry_after),
            AppError::WithdrawalDisposeRejected { status, failures } => {
                let http = StatusCode::from_u16(status).unwrap_or(StatusCode::CONFLICT);
                let label = if status == 409 {
                    "Conflict"
                } else {
                    "Business Rule Violation"
                };
                let body = Json(crate::models::withdrawal::WithdrawalDisposeRejectedBody {
                    code: ec::WITHDRAWAL_DISPOSE_REJECTED.to_string(),
                    error: label.to_string(),
                    message: "No withdrawal was marked disposed".to_string(),
                    failures: *failures,
                });
                (http, body).into_response()
            }
            AppError::ItemToPickForOther {
                hold_id,
                user_id,
                user_name,
            } => {
                let body = Json(ItemToPickForOtherResponse {
                    code: ec::ITEM_TO_PICK_FOR_OTHER.to_string(),
                    error: "Business Rule Violation".to_string(),
                    message: "This copy is on the pick list for another patron — use force=true to override"
                        .to_string(),
                    details: ItemToPickDetails {
                        hold_id: hold_id.to_string(),
                        user_id: user_id.to_string(),
                        user_name,
                    },
                });
                (StatusCode::UNPROCESSABLE_ENTITY, body).into_response()
            }
            other => {
                let fields = other.http_fields();
                let body = Json(ErrorResponse {
                    code: fields.code.to_string(),
                    error: fields.label.to_string(),
                    message: fields.message,
                });
                (fields.status, body).into_response()
            }
        }
    }
}

impl AppError {
    /// HTTP status, stable [`error_code`] string, and client-safe message (aligned with [`ErrorResponse`]).
    pub fn audit_http_fields(&self) -> (u16, &'static str, String) {
        use error_code as ec;

        match self {
            AppError::DuplicateNeedsConfirmation { message, .. } => {
                (409, ec::DUPLICATE_ISBN, message.clone())
            }
            AppError::DuplicateBarcodeNeedsConfirmation { message, .. } => {
                (409, ec::DUPLICATE_BARCODE, message.clone())
            }
            other => {
                let f = other.http_fields();
                (f.status.as_u16(), f.code, f.message)
            }
        }
    }
}

/// Result type alias for application operations
pub type AppResult<T> = Result<T, AppError>;

/// Guard: keep `AppError` under Clippy's `result_large_err` threshold (128 bytes).
const _: () = assert!(
    std::mem::size_of::<AppError>() <= 128,
    "AppError exceeds 128 bytes; box large variants so AppResult stays cheap to move"
);

#[cfg(test)]
mod tests {
    use super::{error_code, AppError};

    #[test]
    fn renewal_too_early_uses_a_dedicated_code() {
        let err = AppError::RenewalTooEarly(
            "Renewal is too early. Renewal is possible from 2026-10-25.".into(),
        );
        let (status, code, message) = err.audit_http_fields();
        assert_eq!(status, 422);
        assert_eq!(code, error_code::RENEWAL_TOO_EARLY);
        assert!(message.contains("2026-10-25"));
    }

    #[test]
    fn item_not_on_loan_stays_not_found_with_a_dedicated_code() {
        let err = AppError::ItemNotOnLoan("Item ABC is not on loan".into());
        let (status, code, message) = err.audit_http_fields();
        assert_eq!(status, 404);
        assert_eq!(code, error_code::ITEM_NOT_ON_LOAN);
        assert_eq!(message, "Item ABC is not on loan");
    }

    #[test]
    fn open_loan_conflicts_are_409_with_stable_codes() {
        let other = AppError::coded(
            409,
            error_code::ITEM_ON_LOAN_TO_OTHER_BORROWER,
            "This item is on loan to another borrower.",
        );
        let (status, code, _) = other.audit_http_fields();
        assert_eq!(status, 409);
        assert_eq!(code, error_code::ITEM_ON_LOAN_TO_OTHER_BORROWER);

        let same = AppError::coded(
            409,
            error_code::ITEM_ALREADY_ON_LOAN_TO_BORROWER,
            "Renew the existing loan.",
        );
        let (status, code, message) = same.audit_http_fields();
        assert_eq!(status, 409);
        assert_eq!(code, error_code::ITEM_ALREADY_ON_LOAN_TO_BORROWER);
        assert!(message.to_lowercase().contains("renew"));
    }
}
