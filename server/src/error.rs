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
    /// Checkout and renewal refused because `users.card_lost` is set. `force` does not override.
    pub const CARD_LOST: &str = "card_lost";
    /// Checkout and renewal refused because `users.expiry_at` is before now.
    /// `force` overrides it only for a caller with circulation override write,
    /// and only when `overrideReason` is present.
    pub const SUBSCRIPTION_EXPIRED: &str = "subscription_expired";
    /// Checkout and renewal refused because unpaid fines are strictly above the effective threshold.
    /// Staff `force` overrides it.
    pub const FINES_OVER_THRESHOLD: &str = "fines_over_threshold";
    /// Checkout refused: active loans are at or above the total cap. Staff `force` overrides it.
    pub const MAX_LOANS: &str = "max_loans";
    /// Checkout refused: active loans of this document type are at or above that cap. Staff `force` overrides it.
    pub const MAX_MEDIA_LOANS: &str = "max_media_loans";
    /// Checkout refused: the copy is not borrowable, or its circulation status is damaged.
    /// Staff `force` overrides it. Lost, claimed returned, to search, and missing are
    /// [`ITEM_STATUS_ACTION_REQUIRED`]. A withdrawn copy stays `business_rule_violation`.
    pub const ITEM_NOT_LOANABLE: &str = "item_not_loanable";
    /// Checkout refused: this copy is `ready` or pinned `pending` for another patron.
    /// `force` overrides it only for a caller with circulation override write,
    /// and only when `overrideReason` is present. The hold is not cancelled.
    /// `to_pick` is [`ITEM_TO_PICK_FOR_OTHER`]. Withdrawal of a held copy stays [`ITEM_ON_HOLD`].
    pub const ITEM_HELD_FOR_OTHER: &str = "item_held_for_other";
    /// Checkout refused: circulation status is lost, claimed returned, to search, or missing.
    /// Staff `force` does not override it. The body includes `itemStatus`.
    pub const ITEM_STATUS_ACTION_REQUIRED: &str = "item_status_action_required";
    /// Create, update, or card replacement tried to assign a retired library card number.
    pub const CARD_NUMBER_RETIRED: &str = "card_number_retired";
    /// Client `message` for [`CARD_LOST`].
    pub const CARD_LOST_MESSAGE: &str = "Library card is reported lost or stolen";
    /// Renewal requested before `due date - renewWindowDays`.
    pub const RENEWAL_TOO_EARLY: &str = "renewal_too_early";
    pub const DUPLICATE_ISBN: &str = "duplicate_isbn_needs_confirmation";
    pub const DUPLICATE_BARCODE: &str = "duplicate_barcode_needs_confirmation";
    pub const RATE_LIMITED: &str = "rate_limited";
    /// Checkout of a copy that is `to_pick` for a different patron. Staff `force` does not override it.
    pub const ITEM_TO_PICK_FOR_OTHER: &str = "item_to_pick_for_other";
    /// Informational check-in code: a known available copy has no active loan.
    /// `POST /loans/items/{barcode}/return` returns HTTP 200 with this code in `notice`.
    pub const ITEM_NOT_ON_LOAN: &str = "item_not_on_loan";
    /// Warning check-in code: the copy is lost, damaged, missing, to-search, or withdrawn.
    /// HTTP 200. The scan does not change that status, assign a hold, or start transit.
    pub const ITEM_NOT_AVAILABLE_STATUS: &str = "item_not_available_status";
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
    /// `POST /loans/items/{barcode}/return` set-aside: `itemId` is not the barcode's copy.
    pub const SET_ASIDE_ITEM_MISMATCH: &str = "set_aside_item_mismatch";
    /// Set-aside: the copy is not an item of the held title.
    pub const SET_ASIDE_WRONG_TITLE: &str = "set_aside_wrong_title";
    /// Set-aside: `holdId` is not the oldest pending unassigned hold on the title.
    pub const SET_ASIDE_NOT_OLDEST: &str = "set_aside_not_oldest";
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
    /// `DELETE /account-types/{code}` while a user still references the code.
    pub const ACCOUNT_TYPE_IN_USE: &str = "account_type_in_use";
    /// `DELETE /account-types/{code}` on a built-in system type.
    pub const ACCOUNT_TYPE_SYSTEM: &str = "account_type_system";
    /// The change would leave no active account with settings write.
    pub const LAST_ACTIVE_ADMIN: &str = "last_active_admin";
    /// `POST /holds/{id}/transits` when the copy's site is already the pickup site.
    pub const TRANSIT_SAME_SITE: &str = "transit_same_site";
    /// `force` skipped a gate that requires `overrideReason`, and the reason was missing or blank.
    pub const OVERRIDE_REASON_REQUIRED: &str = "override_reason_required";
    /// Client `message` for [`OVERRIDE_REASON_REQUIRED`].
    pub const OVERRIDE_REASON_REQUIRED_MESSAGE: &str =
        "A reason is required to force this circulation action";
    /// `PUT /items/{id}` included `circulationStatus`. That column is not writable here.
    pub const CIRCULATION_STATUS_READ_ONLY: &str = "circulation_status_read_only";
    /// Client `message` for [`CIRCULATION_STATUS_READ_ONLY`].
    pub const CIRCULATION_STATUS_READ_ONLY_MESSAGE: &str =
        "circulationStatus is read-only. Use the dedicated actions (lost, missing, to-search).";
    /// `POST /users` for a reader omitted `birthdate`. The server needs it to set `publicType`.
    pub const BIRTH_DATE_REQUIRED: &str = "birth_date_required";
    /// Client `message` for [`BIRTH_DATE_REQUIRED`].
    pub const BIRTH_DATE_REQUIRED_MESSAGE: &str =
        "Birth date is required to determine the reader's public type";
    /// No age band covers the reader's completed years. The body asks an admin to complete the bands.
    pub const NO_PUBLIC_TYPE_FOR_AGE: &str = "no_public_type_for_age";
    /// Create or update of a public type whose age band intersects another band.
    pub const PUBLIC_TYPE_AGE_OVERLAP: &str = "public_type_age_overlap";
    /// `ageMin` is greater than `ageMax`.
    pub const PUBLIC_TYPE_AGE_RANGE_INVALID: &str = "public_type_age_range_invalid";
    /// `POST /fines/{id}/pay` is greater than `amount - paid_amount`.
    pub const PAYMENT_EXCEEDS_BALANCE: &str = "payment_exceeds_balance";
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

    /// Renewal refused on the HTTP boundary (HTTP 422).
    /// `forceable` comes from [`crate::renewal::renewal_refusal_forceable`].
    #[error("{message}")]
    RenewalDenied {
        code: &'static str,
        message: String,
        forceable: bool,
    },

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

    /// Checkout refused (HTTP 422). `forceable` comes from [`crate::checkout_refusal::checkout_refusal_forceable`].
    #[error("{0}")]
    CheckoutRefused(Box<crate::checkout_refusal::CheckoutRefusal>),

    /// Known copy with no active loan, when a caller still wants a hard error.
    /// The barcode return route does not use this variant: it returns HTTP 200
    /// with [`error_code::ITEM_NOT_ON_LOAN`] in `notice`.
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

    /// Payment is greater than the fine's unpaid balance (HTTP 422, `payment_exceeds_balance`).
    #[error("{message}")]
    PaymentExceedsBalance {
        remaining_cents: i64,
        message: String,
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

    /// `PUT /items/{id}` included a `circulationStatus` key.
    #[must_use]
    pub fn circulation_status_read_only() -> Self {
        Self::coded(
            422,
            error_code::CIRCULATION_STATUS_READ_ONLY,
            error_code::CIRCULATION_STATUS_READ_ONLY_MESSAGE,
        )
    }

    #[must_use]
    pub fn account_type_in_use() -> Self {
        Self::coded(
            409,
            error_code::ACCOUNT_TYPE_IN_USE,
            "Account type is still assigned to one or more accounts",
        )
    }

    #[must_use]
    pub fn account_type_system() -> Self {
        Self::coded(
            409,
            error_code::ACCOUNT_TYPE_SYSTEM,
            "System account types cannot be deleted",
        )
    }

    #[must_use]
    pub fn last_active_admin() -> Self {
        Self::coded(
            409,
            error_code::LAST_ACTIVE_ADMIN,
            "This change would leave no active account with admin rights",
        )
    }

    /// `remaining_cents` is `amount - paid_amount` in euro cents.
    #[must_use]
    pub fn payment_exceeds_balance(remaining_cents: i64) -> Self {
        Self::PaymentExceedsBalance {
            remaining_cents,
            message: format!("Payment exceeds the remaining balance of {remaining_cents} cents"),
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

/// `422` from `POST /fines/{id}/pay` when the payment is above the unpaid balance.
/// Same fields as [`ErrorResponse`] plus `remainingCents`.
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaymentExceedsBalanceBody {
    /// Always `"payment_exceeds_balance"`.
    pub code: String,
    /// Human-readable category (`"Business Rule Violation"`).
    pub error: String,
    /// Why the payment was refused.
    pub message: String,
    /// Unpaid balance in euro cents (`amount - paid_amount`).
    pub remaining_cents: i64,
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
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemToPickDetails {
    pub hold_id: String,
    pub user_id: String,
    /// Library card (`users.barcode`). Null when the patron has no card on file.
    pub card_number: Option<String>,
    /// Uppercase first letters of the patron's first and last name.
    pub initials: String,
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
            AppError::RenewalDenied { code, message, .. } => HttpErrorFields {
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
            AppError::CheckoutRefused(refusal) => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: refusal.code,
                label: "Business Rule Violation",
                message: refusal.message.clone(),
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
            AppError::PaymentExceedsBalance { message, .. } => HttpErrorFields {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                code: ec::PAYMENT_EXCEEDS_BALANCE,
                label: "Business Rule Violation",
                message: message.clone(),
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
            AppError::CheckoutRefused(refusal) => {
                let body = Json(refusal.to_body());
                (StatusCode::UNPROCESSABLE_ENTITY, body).into_response()
            }
            AppError::PaymentExceedsBalance {
                remaining_cents,
                message,
            } => {
                let body = Json(PaymentExceedsBalanceBody {
                    code: ec::PAYMENT_EXCEEDS_BALANCE.to_string(),
                    error: "Business Rule Violation".to_string(),
                    message,
                    remaining_cents,
                });
                (StatusCode::UNPROCESSABLE_ENTITY, body).into_response()
            }
            AppError::RenewalDenied {
                code,
                message,
                forceable,
            } => {
                let reason_required =
                    forceable && crate::checkout_refusal::checkout_override_reason_required(code);
                let body = Json(crate::renewal::RenewalRefusalBody {
                    code: code.to_string(),
                    error: "Business Rule Violation".to_string(),
                    message,
                    forceable,
                    reason_required: reason_required.then_some(true),
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

    #[test]
    fn payment_exceeds_balance_is_422_with_remaining_cents() {
        let err = AppError::payment_exceeds_balance(1100);
        let (status, code, message) = err.audit_http_fields();
        assert_eq!(status, 422);
        assert_eq!(code, error_code::PAYMENT_EXCEEDS_BALANCE);
        assert!(message.contains("1100"));
    }
}
