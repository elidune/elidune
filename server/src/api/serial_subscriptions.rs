//! Serial subscription CRUD, search, and horizon generation.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::AppResult,
    models::serial::{
        CreateSerialSupplement, ReceiveSerialIssue, SerialIssue, SerialIssueGenerateResponse,
        SerialIssuesMarkedLate, SerialSubscription, SerialSubscriptionListResponse,
        SerialSubscriptionQuery, SerialSubscriptionWrite, SetSerialIssueStatus,
    },
    services::audit,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

#[utoipa::path(
    get,
    path = "/serials/subscriptions",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(SerialSubscriptionQuery),
    responses(
        (status = 200, description = "Serial subscription list", body = SerialSubscriptionListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
    )
)]
pub async fn list_subscriptions(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<SerialSubscriptionQuery>,
) -> AppResult<Json<SerialSubscriptionListResponse>> {
    claims.require_read_serials()?;
    Ok(Json(
        state.services.serials.list_subscriptions(&query).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/serials/subscriptions/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    responses(
        (status = 200, description = "Serial subscription and its stored issues", body = SerialSubscription),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
    )
)]
pub async fn get_subscription(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<SerialSubscription>> {
    claims.require_read_serials()?;
    Ok(Json(state.services.serials.get_subscription(id).await?))
}

#[utoipa::path(
    get,
    path = "/serials/subscriptions/{id}/issues",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    responses(
        (status = 200, description = "Stored issues, including display labels", body = Vec<SerialIssue>),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
    )
)]
pub async fn list_subscription_issues(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Vec<SerialIssue>>> {
    claims.require_read_serials()?;
    Ok(Json(
        state.services.serials.list_subscription_issues(id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions",
    tag = "serials",
    security(("bearer_auth" = [])),
    request_body = SerialSubscriptionWrite,
    responses(
        (status = 201, description = "Serial subscription created and the first horizon stored", body = SerialSubscription),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Bibliographic record, frequency, or numbering pattern not found", body = ErrorResponse),
        (status = 409, description = "An open serial subscription already exists for this periodical at this site", body = ErrorResponse),
        (status = 422, description = "The frequency cannot be predicted or the horizon is empty", body = ErrorResponse),
    )
)]
pub async fn create_subscription(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<SerialSubscriptionWrite>,
) -> AppResult<(StatusCode, Json<SerialSubscription>)> {
    claims.require_write_serials()?;
    match state.services.serials.create_subscription(&data).await {
        Ok(subscription) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_CREATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(subscription.id),
                ip,
                Some(&subscription),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(subscription)))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_CREATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                None,
                ip,
                Some(&data.biblio_id),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    put,
    path = "/serials/subscriptions/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    request_body = SerialSubscriptionWrite,
    responses(
        (status = 200, description = "Serial subscription updated", body = SerialSubscription),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
        (status = 409, description = "Prediction inputs cannot change after issues exist, or another open subscription uses this periodical and site", body = ErrorResponse),
        (status = 422, description = "The frequency cannot be predicted or the horizon is empty", body = ErrorResponse),
    )
)]
pub async fn update_subscription(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<SerialSubscriptionWrite>,
) -> AppResult<Json<SerialSubscription>> {
    claims.require_write_serials()?;
    match state.services.serials.update_subscription(id, &data).await {
        Ok(subscription) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_UPDATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                Some(&subscription),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(subscription))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_UPDATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                Some(&data.biblio_id),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/serials/subscriptions/{id}",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    responses(
        (status = 204, description = "Serial subscription and its expected issues deleted"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
        (status = 409, description = "Received or supplemental issues are still attached", body = ErrorResponse),
    )
)]
pub async fn delete_subscription(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    claims.require_write_serials()?;
    match state.services.serials.delete_subscription(id).await {
        Ok(subscription) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_DELETED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                Some(&subscription),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_DELETED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions/{id}/generate",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    responses(
        (status = 200, description = "Next horizon appended. Existing issue rows are unchanged.", body = SerialIssueGenerateResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
        (status = 409, description = "The horizon was already generated", body = ErrorResponse),
        (status = 422, description = "Nothing left to predict, or the subscription is not active", body = ErrorResponse),
    )
)]
pub async fn generate_subscription(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<SerialIssueGenerateResponse>> {
    claims.require_write_serials()?;
    match state.services.serials.generate_subscription(id).await {
        Ok(generated) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_ISSUES_GENERATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                Some(&generated.issues.len()),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(generated))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_SUBSCRIPTION_ISSUES_GENERATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions/{id}/issues/{issueId}/receive",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Serial subscription id"),
        ("issueId" = i64, Path, description = "Serial issue id"),
    ),
    request_body = ReceiveSerialIssue,
    responses(
        (status = 200, description = "Issue received. Other issues are not marked arrived. When create_items is set, a copy is created through the shared receipt item constructor.", body = SerialIssue),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription or issue not found", body = ErrorResponse),
        (status = 409, description = "The issue changed and could not be received", body = ErrorResponse),
        (status = 422, description = "The issue is not expected or late, or the subscription does not create copies", body = ErrorResponse),
    )
)]
pub async fn receive_issue(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path((id, issue_id)): Path<(i64, i64)>,
    ValidatedJson(data): ValidatedJson<ReceiveSerialIssue>,
) -> AppResult<Json<SerialIssue>> {
    claims.require_write_serials()?;
    match state
        .services
        .serials
        .receive_issue(id, issue_id, &data)
        .await
    {
        Ok(issue) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_RECEIVED,
                Some(claims.user_id),
                Some("serial_issue"),
                Some(issue_id),
                ip,
                Some(&id),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(issue))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_RECEIVED,
                Some(claims.user_id),
                Some("serial_issue"),
                Some(issue_id),
                ip,
                Some(&id),
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions/{id}/issues/{issueId}/status",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Serial subscription id"),
        ("issueId" = i64, Path, description = "Serial issue id"),
    ),
    request_body = SetSerialIssueStatus,
    responses(
        (status = 200, description = "Status updated. missing is claim-ready. notAvailable means the issue will not be published. claimed does not send a vendor claim.", body = SerialIssue),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription or issue not found", body = ErrorResponse),
        (status = 422, description = "That status change is not allowed", body = ErrorResponse),
    )
)]
pub async fn set_issue_status(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path((id, issue_id)): Path<(i64, i64)>,
    Json(data): Json<SetSerialIssueStatus>,
) -> AppResult<Json<SerialIssue>> {
    claims.require_write_serials()?;
    match state
        .services
        .serials
        .set_issue_status(id, issue_id, &data)
        .await
    {
        Ok(issue) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_STATUS_CHANGED,
                Some(claims.user_id),
                Some("serial_issue"),
                Some(issue_id),
                ip,
                Some(issue.status.as_db_str()),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(issue))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_STATUS_CHANGED,
                Some(claims.user_id),
                Some("serial_issue"),
                Some(issue_id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions/{id}/supplements",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    request_body = CreateSerialSupplement,
    responses(
        (status = 201, description = "Supplement stored with a null slot and a null sequence", body = SerialIssue),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
    )
)]
pub async fn create_supplement(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<CreateSerialSupplement>,
) -> AppResult<(StatusCode, Json<SerialIssue>)> {
    claims.require_write_serials()?;
    match state.services.serials.create_supplement(id, &data).await {
        Ok(issue) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_SUPPLEMENT_CREATED,
                Some(claims.user_id),
                Some("serial_issue"),
                Some(issue.id),
                ip,
                Some(&id),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(issue)))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUE_SUPPLEMENT_CREATED,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}

#[utoipa::path(
    post,
    path = "/serials/subscriptions/{id}/mark-late",
    tag = "serials",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Serial subscription id")),
    responses(
        (status = 200, description = "Expected issues past planned date plus effective grace are now late. Receiving an issue does not mark another issue late.", body = SerialIssuesMarkedLate),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient serials rights", body = ErrorResponse),
        (status = 404, description = "Serial subscription not found", body = ErrorResponse),
    )
)]
pub async fn mark_late_issues(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<SerialIssuesMarkedLate>> {
    claims.require_write_serials()?;
    match state.services.serials.mark_late_issues(id).await {
        Ok(marked) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUES_MARKED_LATE,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                Some(&marked.issues.len()),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(marked))
        }
        Err(err) => {
            state.services.audit.log(
                audit::event::SERIAL_ISSUES_MARKED_LATE,
                Some(claims.user_id),
                Some("serial_subscription"),
                Some(id),
                ip,
                None::<&str>,
                audit::AuditLogMeta::from_app_error(&err),
            );
            Err(err)
        }
    }
}
