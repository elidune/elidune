//! Events API endpoints (cultural actions, school visits, animations)

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult},
    events_consent,
    models::event::{CreateEvent, Event, EventQuery, UpdateEvent},
    services::{
        audit,
        events::{AnnouncementReport, SendAnnouncementRequest},
    },
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp};

/// Build the events routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/events", get(list_events).post(create_event))
        .route(
            "/events/:id",
            get(get_event).put(update_event).delete(delete_event),
        )
        .route(
            "/events/:id/send-announcement",
            post(send_event_announcement),
        )
        .route(
            "/events/:id/announcement-recipients/count",
            get(count_announcement_recipients),
        )
}

/// Public unsubscribe route (no login). Mounted on the public rate limiter.
///
/// The confirmation page is the UI at `/events/unsubscribe`. This route only
/// performs the withdrawal, so a mail prefetch cannot opt the patron out.
pub fn public_router() -> axum::Router<crate::AppState> {
    use axum::routing::post;
    axum::Router::new().route("/events/unsubscribe", post(unsubscribe_events))
}

/// Distinct patrons who would receive an announcement for this event.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementRecipientCount {
    /// Recipients after guardian routing. A guardian of several children counts once.
    pub count: i64,
}

/// JSON body for `POST /events/unsubscribe`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeEventsRequest {
    /// Signed token from the email link (`/events/unsubscribe?token=`).
    pub token: String,
}

/// Paginated events response
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventsListResponse {
    pub events: Vec<Event>,
    pub total: i64,
}

/// List events with filters and pagination
#[utoipa::path(
    get,
    path = "/events",
    tag = "events",
    security(("bearer_auth" = [])),
    params(EventQuery),
    responses(
        (status = 200, description = "Events list", body = EventsListResponse),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn list_events(
    State(state): State<crate::AppState>,
    Query(query): Query<EventQuery>,
) -> AppResult<Json<EventsListResponse>> {
    let (events, total) = state.services.events.list(&query).await?;
    Ok(Json(EventsListResponse { events, total }))
}

/// Get event by ID (includes `attachmentDataBase64` when an attachment exists)
#[utoipa::path(
    get,
    path = "/events/{id}",
    tag = "events",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Event ID")),
    responses(
        (status = 200, description = "Event details", body = Event),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn get_event(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Event>> {
    let event = state.services.events.get_by_id_with_attachment(id).await?;
    Ok(Json(event))
}

/// Create an event
#[utoipa::path(
    post,
    path = "/events",
    tag = "events",
    security(("bearer_auth" = [])),
    request_body = CreateEvent,
    responses(
        (status = 201, description = "Event created", body = Event),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn create_event(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Json(data): Json<CreateEvent>,
) -> AppResult<(StatusCode, Json<Event>)> {
    claims.require_write_events()?;
    match state.services.events.create(&data).await {
        Ok(event) => {
            state.services.audit.log(
                audit::event::EVENT_CREATED,
                Some(claims.user_id),
                Some("event"),
                Some(event.id),
                ip,
                Some((&data, &event)),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(event)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::EVENT_CREATED,
                Some(claims.user_id),
                Some("event"),
                None,
                ip.clone(),
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Update an event (optional `attachment` / `removeAttachment` same as create semantics)
#[utoipa::path(
    put,
    path = "/events/{id}",
    tag = "events",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Event ID")),
    request_body = UpdateEvent,
    responses(
        (status = 200, description = "Event updated", body = Event),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn update_event(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(data): Json<UpdateEvent>,
) -> AppResult<Json<Event>> {
    claims.require_write_events()?;
    match state.services.events.update(id, &data).await {
        Ok(event) => {
            state.services.audit.log(
                audit::event::EVENT_UPDATED,
                Some(claims.user_id),
                Some("event"),
                Some(id),
                ip,
                Some((id, &data, &event)),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(event))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::EVENT_UPDATED,
                Some(claims.user_id),
                Some("event"),
                Some(id),
                ip.clone(),
                Some((id, &data)),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Delete an event
#[utoipa::path(
    delete,
    path = "/events/{id}",
    tag = "events",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Event ID")),
    responses(
        (status = 204, description = "Event deleted"),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn delete_event(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    claims.require_write_events()?;
    match state.services.events.delete(id).await {
        Ok(()) => {
            state.services.audit.log(
                audit::event::EVENT_DELETED,
                Some(claims.user_id),
                Some("event"),
                Some(id),
                ip,
                Some(serde_json::json!({ "id": id })),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::EVENT_DELETED,
                Some(claims.user_id),
                Some("event"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({ "id": id })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

/// Send an announcement email to patrons in any selected audience (`publicTypes`), once each.
/// Recipients are chosen in SQL. A non-child patron is emailed when `eventsConsentAt` is
/// set and either `allAudiences` is set or their public type is one of `publicTypes`.
/// A child is not emailed: the message goes to their legal guardian, using the guardian's
/// `eventsConsentAt`, and names the child or children concerned.
/// The child's own consent is ignored. `receiveReminders` is not consulted.
///
/// Each message includes an unsubscribe link. The first message queued for a patron whose
/// consent source is `migration` and who has not yet received the notice also explains
/// the opt-in.
///
/// The default `event_announcement` template is used unless `subject`/`body_plain`
/// (and optionally `body_html`) are supplied in the request body, in which case the
/// supplied text overrides the template. A guardian message still names the children
/// when that text does not already include them.
#[utoipa::path(
    post,
    path = "/events/{id}/send-announcement",
    tag = "events",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Event ID")),
    request_body = SendAnnouncementRequest,
    responses(
        (status = 200, description = "Announcement report", body = AnnouncementReport),
        (status = 400, description = "Bad request", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn send_event_announcement(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Json(payload): Json<SendAnnouncementRequest>,
) -> AppResult<Json<AnnouncementReport>> {
    claims.require_write_events()?;
    let report = state
        .services
        .events
        .send_announcement(id, &payload, Some(claims.user_id), ip)
        .await?;
    Ok(Json(report))
}

/// Count patrons who would receive an announcement for this event.
///
/// Same SQL as `POST /events/{id}/send-announcement`: audience filter, guardian
/// routing, and `eventsConsentAt` on the adult or the guardian. Requires write
/// access to events, the same permission as sending.
#[utoipa::path(
    get,
    path = "/events/{id}/announcement-recipients/count",
    tag = "events",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Event ID")),
    responses(
        (status = 200, description = "Distinct recipient count", body = AnnouncementRecipientCount),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient permissions", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
    )
)]
pub async fn count_announcement_recipients(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<AnnouncementRecipientCount>> {
    claims.require_write_events()?;
    let count = state
        .services
        .events
        .count_announcement_recipients(id)
        .await?;
    Ok(Json(AnnouncementRecipientCount { count }))
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeQuery {
    token: Option<String>,
}

/// Withdraw event-announcement consent. No login.
///
/// The UI posts JSON `{ "token" }` from `/events/unsubscribe` after the patron
/// clicks. RFC 8058 one-click posts `List-Unsubscribe=One-Click` to the
/// `List-Unsubscribe` URL, which carries `?token=`. A second call for a patron
/// who is already withdrawn returns 204.
#[utoipa::path(
    post,
    path = "/events/unsubscribe",
    tag = "events",
    params(("token" = Option<String>, Query, description = "Signed token. Required for RFC 8058 one-click; the JSON body carries it otherwise.")),
    request_body(content = UnsubscribeEventsRequest, description = "JSON token from the UI. Omit when posting the one-click form body.", content_type = "application/json"),
    responses(
        (status = 204, description = "Consent withdrawn, or already withdrawn"),
        (status = 400, description = "Invalid request"),
    )
)]
pub async fn unsubscribe_events(
    State(state): State<crate::AppState>,
    headers: axum::http::HeaderMap,
    Query(query): Query<UnsubscribeQuery>,
    body: axum::body::Bytes,
) -> Response {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let Some(token) = unsubscribe_token(content_type, query.token.as_deref(), &body) else {
        return invalid_unsubscribe();
    };
    let Some(user_id) = events_consent::verify(state.config.users.jwt_secret.as_bytes(), &token)
    else {
        return invalid_unsubscribe();
    };
    match state.services.users.get_by_id(user_id).await {
        Ok(_) => {}
        Err(AppError::NotFound(_)) => return invalid_unsubscribe(),
        Err(err) => return err.into_response(),
    }
    if let Err(err) = state
        .services
        .repository
        .users_unsubscribe_events(user_id)
        .await
    {
        return err.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

/// JSON `{ "token" }`, or `?token=` together with the RFC 8058 form body.
fn unsubscribe_token(content_type: &str, query_token: Option<&str>, body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let json = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("application/json")
        || text.trim_start().starts_with('{');
    if json {
        let parsed: UnsubscribeEventsRequest = serde_json::from_slice(body).ok()?;
        let token = parsed.token.trim();
        if token.is_empty() {
            return None;
        }
        return Some(token.to_string());
    }
    let query_token = query_token
        .map(str::trim)
        .filter(|token| !token.is_empty())?;
    if is_one_click_body(text) {
        Some(query_token.to_string())
    } else {
        None
    }
}

fn is_one_click_body(body: &str) -> bool {
    let trimmed = body.trim();
    trimmed == "List-Unsubscribe=One-Click"
        || trimmed
            .split('&')
            .any(|pair| pair.trim() == "List-Unsubscribe=One-Click")
}

fn invalid_unsubscribe() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        events_consent::INVALID_REQUEST,
    )
        .into_response()
}
