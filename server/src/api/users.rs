//! User management endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    error::AppResult,
    models::catalog_view::{borrower_initials, UserShortStaff},
    models::dto::patron_card::{CardLookupResponse, DeskCardLookup, StaffCardLookup},
    models::user::{
        EventsConsentSource, Rights, UpdateAccountType, UpdateProfile, User, UserPayload, UserQuery,
    },
    services::audit,
};

use super::{biblios::PaginatedResponse, AuthenticatedUser, ClientIp, ValidatedJson};

/// Build the users routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/by-card/:card_number", get(get_user_by_card))
        .route(
            "/users/:id",
            get(get_user).put(update_user).delete(delete_user),
        )
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
        (status = 200, description = "User details", body = User),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<User>> {
    claims.require_read_users()?;

    let user = state.services.users.get_by_id(id).await?;
    Ok(Json(user))
}

/// Create a new user
#[utoipa::path(
    post,
    path = "/users",
    tag = "users",
    security(("bearer_auth" = [])),
    request_body = UserPayload,
    responses(
        (status = 201, description = "User created", body = User),
        (status = 400, description = "Invalid input, including missing guardianId for child patrons"),
        (status = 409, description = "Login already exists")
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
        (status = 200, description = "User updated", body = User),
        (status = 400, description = "Invalid input, including clearing guardianId while the patron remains a child"),
        (status = 404, description = "User not found")
    )
)]
pub async fn update_user(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(user): ValidatedJson<UserPayload>,
) -> AppResult<Json<User>> {
    claims.require_write_users()?;
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
