//! User management endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    error::{AppError, AppResult},
    models::catalog_view::{borrower_initials, UserShortStaff},
    models::dto::card::{
        ReplaceCardRequest, ReplaceCardResponse, ReportCardLostDeskBody, ReportCardLostRequest,
        ReportCardLostResponse, ReportCardLostStaffBody,
    },
    models::dto::patron_card::{CardLookupResponse, DeskCardLookup, StaffCardLookup},
    models::own_contact::parse_own_contact,
    models::user::{
        EventsConsentSource, Rights, UpdateAccountType, UpdateProfile, User, UserClaims,
        UserPayload, UserQuery,
    },
    services::audit,
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, ValidatedJson};
use validator::Validate;

/// Build the users routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/by-card/:card_number", get(get_user_by_card))
        .route(
            "/users/:id",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/users/:id/card-lost", post(report_card_lost))
        .route("/users/:id/replace-card", post(replace_card))
        .route("/users/:id/account-type", put(update_account_type))
        .route(
            "/users/:id/force-password-change",
            put(force_password_change),
        )
        .route("/users/:id/loans", get(super::loans::get_user_loans))
        .route(
            "/users/:id/loans/export",
            get(super::loans::export_user_loans_marc),
        )
}

/// List users with search and pagination
#[utoipa::path(
    get,
    path = "/users",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("name" = Option<String>, Query, description = "Search by name"),
        ("barcode" = Option<String>, Query, description = "Search by barcode"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("per_page" = Option<i64>, Query, description = "Items per page")
    ),
    responses(
        (status = 200, description = "List of users. Staff summary includes `login` and `barcode`. Readers cannot call this route.", body = PaginatedResponse<UserShortStaff>),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn list_users(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<UserQuery>,
) -> AppResult<Json<PaginatedResponse<UserShortStaff>>> {
    claims.require_read_users()?;

    let (users, total) = state.services.users.search_users(&query).await?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    Ok(Json(PaginatedResponse::new(
        users.into_iter().map(UserShortStaff::from).collect(),
        total,
        page,
        per_page,
    )))
}

/// Exact library-card lookup for the circulation desk.
///
/// The calling agent has no site on the user row or the JWT, so `loanStatus.readyHolds`
/// counts every pickup site. Name search stays on `GET /users` and still requires users read.
#[utoipa::path(
    get,
    path = "/users/by-card/{card_number}",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("card_number" = String, Path, description = "Exact library card (`users.barcode`). Not a prefix or a name.")
    ),
    responses(
        (status = 200, description = "Patron for this card. `users_rights` read or write: full `User` plus `loanStatus`. `loans_rights` read or write without users read: `id`, `cardNumber`, `initials`, and `loanStatus` only. `loanStatus.blocked` matches checkout without force. `blockReasons` adds `overdue_loans` when a loan is past due; that code does not set `blocked`. `readyHolds` counts `ready` holds at every site because the agent has no site.", body = CardLookupResponse),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Caller has neither users read nor loans read"),
        (status = 404, description = "No active patron with this exact card")
    )
)]
pub async fn get_user_by_card(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(card_number): Path<String>,
) -> AppResult<Json<CardLookupResponse>> {
    let users_read = claims.rights.users_rights.rank() >= Rights::Read.rank();
    let loans_read = claims.rights.loans_rights.rank() >= Rights::Read.rank();
    if !users_read && !loans_read {
        return Err(crate::error::AppError::Authorization(
            "Insufficient rights to look up a patron card".to_string(),
        ));
    }

    // No site is stored on the staff user or the JWT. None counts every pickup site.
    let lookup = crate::services::patron_card::lookup_patron_by_card(
        &state.services.repository,
        &state.services.fines,
        &card_number,
        None,
    )
    .await?;

    if users_read {
        return Ok(Json(CardLookupResponse::Staff(Box::new(StaffCardLookup {
            user: lookup.user,
            loan_status: lookup.loan_status,
        }))));
    }

    let card_number = lookup.user.barcode.filter(|value| !value.trim().is_empty());
    let initials = borrower_initials(
        lookup.user.firstname.as_deref(),
        lookup.user.lastname.as_deref(),
    );
    Ok(Json(CardLookupResponse::Desk(DeskCardLookup {
        id: lookup.user.id,
        card_number,
        initials,
        loan_status: lookup.loan_status,
    })))
}

/// Get user details by ID
#[utoipa::path(
    get,
    path = "/users/{id}",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User details. `users_rights` read or write may load any id. `users_rights` own may load only the caller's id.", body = User),
        (status = 403, description = "Own rights on another id, or no users rights"),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<User>> {
    require_read_user(&claims, id)?;

    let user = state.services.users.get_by_id(id).await?;
    Ok(Json(user))
}

/// `users_rights` read or write, or own rights on the caller's id.
fn require_read_user(claims: &UserClaims, id: i64) -> Result<(), AppError> {
    if claims.rights.users_rights.rank() >= Rights::Read.rank()
        || (claims.rights.users_rights == Rights::Own && claims.user_id == id)
    {
        Ok(())
    } else {
        Err(AppError::Authorization(
            "Insufficient rights to read this user".to_string(),
        ))
    }
}

/// Create a new user
#[utoipa::path(
    post,
    path = "/users",
    tag = "users",
    security(("bearer_auth" = [])),
    request_body = UserPayload,
    responses(
        (status = 201, description = "User created. `language` omitted or null is stored as no preference.", body = User),
        (status = 400, description = "Invalid input, including missing guardianId for child patrons", body = crate::error::ErrorResponse),
        (status = 409, description = "Login already exists, card number already assigned, or a retired card number (`card_number_retired`)", body = crate::error::ErrorResponse),
        (status = 422, description = "`language` is not `english`, `french`, `german`, or `spanish`", body = crate::error::ErrorResponse)
    )
)]
pub async fn create_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(user): ValidatedJson<UserPayload>,
) -> AppResult<(StatusCode, Json<User>)> {
    claims.require_write_users()?;
    let for_audit = user.clone();
    match state
        .services
        .users
        .create_user(user, EventsConsentSource::Desk)
        .await
    {
        Ok(created) => {
            state.services.audit.log(
                audit::event::USER_CREATED,
                Some(claims.user_id),
                Some("user"),
                Some(created.id),
                ip,
                Some(&created),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(created)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_CREATED,
                Some(claims.user_id),
                Some("user"),
                None,
                ip.clone(),
                Some(&for_audit),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Update an existing user
#[utoipa::path(
    put,
    path = "/users/{id}",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "User ID")
    ),
    request_body = UserPayload,
    responses(
        (status = 200, description = "User updated. `users_rights` write may change the staff payload. `users_rights` own may change only the caller's contact details and preferences: `addrStreet`, `addrZipCode`, `addrCity`, `email`, `phone`, `language`, `eventsConsent`, `receiveReminders`. Any other key (account type, rights, card number, status, fines, name, login) returns 403 and writes nothing.", body = User),
        (status = 400, description = "Invalid input, including clearing guardianId while the patron remains a child", body = crate::error::ErrorResponse),
        (status = 403, description = "Own rights on another id, a forbidden own-record field, or no users write", body = crate::error::ErrorResponse),
        (status = 404, description = "User not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Login already exists, card number already assigned, or a retired card number (`card_number_retired`)", body = crate::error::ErrorResponse)
    )
)]
pub async fn update_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> AppResult<Json<User>> {
    if claims.rights.users_rights == Rights::Own {
        if claims.user_id != id {
            return Err(AppError::Authorization(
                "Insufficient rights to update this user".to_string(),
            ));
        }
        let patch = parse_own_contact(&body)?;
        let audit_payload = body.clone();
        return match state.services.users.update_own_contact(id, patch).await {
            Ok(updated) => {
                state.services.audit.log(
                    audit::event::USER_UPDATED,
                    Some(claims.user_id),
                    Some("user"),
                    Some(id),
                    ip,
                    Some(audit_payload),
                    audit::AuditLogMeta::success(),
                );
                Ok(Json(updated))
            }
            Err(e) => {
                state.services.audit.log(
                    audit::event::USER_UPDATED,
                    Some(claims.user_id),
                    Some("user"),
                    Some(id),
                    ip.clone(),
                    Some(serde_json::json!({ "id": id })),
                    audit::AuditLogMeta::from_app_error(&e),
                );
                Err(e)
            }
        };
    }

    claims.require_write_users()?;
    let user: UserPayload = serde_json::from_value(body)
        .map_err(|e| AppError::Validation(format!("Invalid JSON body: {e}")))?;
    user.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let audit_payload = user.clone();
    match state
        .services
        .users
        .update_user(id, user, EventsConsentSource::Desk)
        .await
    {
        Ok(updated) => {
            state.services.audit.log(
                audit::event::USER_UPDATED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some((id, audit_payload)),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(updated))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_UPDATED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({ "id": id })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Report the patron's library card lost or stolen.
///
/// Sets `users.card_lost`. Checkout and renewal refuse with `422` `card_lost` until
/// [`replace_card`] assigns a new number. Allowed for users write or loans write.
/// Loans write without users read receives only `{ id, cardNumber, initials }`.
#[utoipa::path(
    post,
    path = "/users/{id}/card-lost",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "User ID")
    ),
    request_body = ReportCardLostRequest,
    responses(
        (status = 200, description = "Card flagged lost. Users read or write: `userId`, `cardLost`, `reason` (`lost`, `stolen`, or `other`), and optional `detail`. Loans write without users read: `id`, `cardNumber`, and `initials` only.", body = ReportCardLostResponse),
        (status = 400, description = "Missing or unknown reason, or detail longer than 500 characters", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Caller has neither users write nor loans write", body = crate::error::ErrorResponse),
        (status = 404, description = "User not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn report_card_lost(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(body): ValidatedJson<ReportCardLostRequest>,
) -> AppResult<Json<ReportCardLostResponse>> {
    claims.require_report_card_lost()?;
    let requested = serde_json::json!({
        "reason": body.reason.as_str(),
        "detail": body.detail,
        "actorId": claims.user_id,
    });
    match state
        .services
        .users
        .report_card_lost(id, body.reason, body.detail)
        .await
    {
        Ok(reported) => {
            state.services.audit.log(
                audit::event::USER_CARD_LOST,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some(serde_json::json!({
                    "reason": reported.reason.as_str(),
                    "detail": reported.detail,
                    "actorId": claims.user_id,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(report_card_lost_body(&claims, reported)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_CARD_LOST,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(requested),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

fn report_card_lost_body(
    claims: &crate::models::user::UserClaims,
    reported: crate::models::dto::card::ReportedLostCard,
) -> ReportCardLostResponse {
    if claims.rights.users_rights.rank() >= Rights::Read.rank() {
        ReportCardLostResponse::Staff(ReportCardLostStaffBody {
            user_id: reported.user_id,
            card_lost: true,
            reason: reported.reason,
            detail: reported.detail,
        })
    } else {
        ReportCardLostResponse::Desk(ReportCardLostDeskBody {
            id: reported.user_id,
            card_number: reported.card_number,
            initials: borrower_initials(
                reported.firstname.as_deref(),
                reported.lastname.as_deref(),
            ),
        })
    }
}

/// Replace the patron's library card.
///
/// The new `barcode` must be unique and must not be a retired number. The previous
/// number is stored in `retired_card_numbers` and can never be assigned again.
/// `users.card_lost` is cleared so checkout is allowed again.
#[utoipa::path(
    post,
    path = "/users/{id}/replace-card",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "User ID")
    ),
    request_body = ReplaceCardRequest,
    responses(
        (status = 200, description = "New card number assigned, previous number retired, and card_lost cleared", body = ReplaceCardResponse),
        (status = 400, description = "Missing barcode, or the new number is the current number", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 403, description = "Caller does not have users write", body = crate::error::ErrorResponse),
        (status = 404, description = "User not found", body = crate::error::ErrorResponse),
        (status = 409, description = "Barcode already assigned, or a retired card number (`card_number_retired`)", body = crate::error::ErrorResponse)
    )
)]
pub async fn replace_card(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(body): ValidatedJson<ReplaceCardRequest>,
) -> AppResult<Json<ReplaceCardResponse>> {
    claims.require_write_users()?;
    let requested = serde_json::json!({
        "barcode": body.barcode,
        "actorId": claims.user_id,
    });
    match state
        .services
        .users
        .replace_card(id, &body.barcode, claims.user_id)
        .await
    {
        Ok(replaced) => {
            state.services.audit.log(
                audit::event::USER_CARD_REPLACED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some(serde_json::json!({
                    "previousBarcode": replaced.previous_barcode,
                    "barcode": replaced.barcode,
                    "actorId": claims.user_id,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(replaced))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_CARD_REPLACED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(requested),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Delete a user
#[utoipa::path(
    delete,
    path = "/users/{id}",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "User ID"),
        ("force" = Option<bool>, Query, description = "Force anonymize even with active loans (loans are returned first)")
    ),
    responses(
        (status = 204, description = "Patron anonymized for stats"),
        (status = 404, description = "User not found"),
        (status = 422, description = "User has active loans")
    )
)]
pub async fn delete_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Query(params): Query<DeleteUserParams>,
) -> AppResult<StatusCode> {
    claims.require_write_users()?;
    let force = params.force.unwrap_or(false);
    match state.services.users.delete_user(id, force).await {
        Ok(erasure) => {
            state.services.audit.log(
                audit::event::USER_DELETED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some(serde_json::json!({
                    "id": id,
                    "force": erasure.force,
                    "anonymized": true,
                    "loansForceReturned": erasure.loans_force_returned,
                    "holdsCancelled": erasure.holds_cancelled,
                    "archivesUnlinked": erasure.archives_unlinked,
                    "finesUnlinked": erasure.fines_unlinked,
                })),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_DELETED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({ "id": id, "force": force, "anonymized": false })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[derive(Deserialize)]
pub struct DeleteUserParams {
    pub force: Option<bool>,
}

/// Update own profile (name, password)
#[utoipa::path(
    put,
    path = "/auth/profile",
    tag = "auth",
    security(("bearer_auth" = [])),
    request_body = UpdateProfile,
    responses(
        (status = 200, description = "Profile updated", body = User),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Not authenticated or wrong current password")
    )
)]
pub async fn update_my_profile(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ValidatedJson(profile): ValidatedJson<UpdateProfile>,
) -> AppResult<Json<User>> {
    let updated = state
        .services
        .users
        .update_profile(claims.user_id, profile)
        .await?;
    Ok(Json(updated))
}

/// Update user's account type (admin only)
#[utoipa::path(
    put,
    path = "/users/{id}/account-type",
    tag = "users",
    security(("bearer_auth" = [])),
    params(
        ("id" = i32, Path, description = "User ID")
    ),
    request_body = UpdateAccountType,
    responses(
        (status = 200, description = "Account type updated", body = User),
        (status = 403, description = "Admin privileges required"),
        (status = 404, description = "User not found")
    )
)]
pub async fn update_account_type(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(request): Json<UpdateAccountType>,
) -> AppResult<Json<User>> {
    claims.require_admin()?;
    match state
        .services
        .users
        .update_account_type(id, &request.account_type)
        .await
    {
        Ok(updated) => {
            state.services.audit.log(
                audit::event::USER_ACCOUNT_TYPE_CHANGED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some(serde_json::json!({ "new_account_type": request.account_type })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(updated))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_ACCOUNT_TYPE_CHANGED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({ "new_account_type": request.account_type })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Force the user to change their password on next login (admin only).
#[utoipa::path(
    put,
    path = "/users/{id}/force-password-change",
    tag = "users",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "Flag updated"),
        (status = 401, description = "Not authenticated"),
        (status = 403, description = "Admin privileges required"),
        (status = 404, description = "User not found")
    )
)]
pub async fn force_password_change(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    claims.require_admin()?;

    match state
        .services
        .users
        .set_must_change_password(id, true)
        .await
    {
        Ok(()) => {
            state.services.audit.log(
                audit::event::USER_UPDATED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip,
                Some(serde_json::json!({ "must_change_password": true })),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(
                serde_json::json!({ "message": "User must change password on next login" }),
            ))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::USER_UPDATED,
                Some(claims.user_id),
                Some("user"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({
                    "must_change_password": true,
                    "action": "force_password_change"
                })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}
