//! Chat assistant HTTP API.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

use crate::{
    error::AppResult,
    models::chat::{ChatConversation, ConversationDetail, CreateConversationRequest, LlmProviderPublic, SendChatMessageRequest, UpdateConversationRequest},
    AppState,
};

use super::AuthenticatedUser;

pub fn router() -> Router<AppState> {
    let chat_governor_conf: &'static _ = Box::leak(Box::new(GovernorConfigBuilder::default().per_second(2).burst_size(4).finish().expect("chat rate limit")));

    Router::new()
        .route("/chat/providers", get(list_providers))
        .route("/chat/conversations", get(list_conversations).post(create_conversation))
        .route("/chat/conversations/:id", get(get_conversation).patch(update_conversation).delete(delete_conversation))
        .route("/chat/conversations/:id/messages", post(send_message))
        .route("/chat/conversations/:id/cancel", post(cancel_message))
        .layer(GovernorLayer { config: chat_governor_conf })
}

#[utoipa::path(
    get,
    path = "/chat/providers",
    tag = "chat",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Enabled LLM providers", body = Vec<LlmProviderPublic>),
        (status = 403, description = "Chat access required")
    )
)]
pub async fn list_providers(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser) -> AppResult<Json<Vec<LlmProviderPublic>>> {
    claims.require_chat()?;
    if !state.config.chat.enabled {
        return Ok(Json(vec![]));
    }
    let list = state.services.llm_providers.list_public().await?;
    Ok(Json(list))
}

#[utoipa::path(
    get,
    path = "/chat/conversations",
    tag = "chat",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "User conversations", body = Vec<ChatConversation>),
        (status = 403, description = "Chat access required")
    )
)]
pub async fn list_conversations(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser) -> AppResult<Json<Vec<ChatConversation>>> {
    claims.require_chat()?;
    let list = state.services.chat.list_conversations(claims.user_id).await?;
    Ok(Json(list))
}

#[utoipa::path(
    post,
    path = "/chat/conversations",
    tag = "chat",
    security(("bearer_auth" = [])),
    request_body = CreateConversationRequest,
    responses(
        (status = 200, description = "Created conversation", body = ChatConversation),
        (status = 403, description = "Chat access required")
    )
)]
pub async fn create_conversation(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, Json(req): Json<CreateConversationRequest>) -> AppResult<Json<ChatConversation>> {
    claims.require_chat()?;
    if !state.config.chat.enabled {
        return Err(crate::error::AppError::BusinessRule("Chat is disabled".into()));
    }
    let conv = state.services.chat.create_conversation(claims.user_id, req).await?;
    Ok(Json(conv))
}

#[utoipa::path(
    get,
    path = "/chat/conversations/{id}",
    tag = "chat",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Conversation ID")),
    responses(
        (status = 200, description = "Conversation with messages", body = ConversationDetail),
        (status = 404, description = "Not found"),
        (status = 403, description = "Chat access required")
    )
)]
pub async fn get_conversation(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<Json<ConversationDetail>> {
    claims.require_chat()?;
    let detail = state.services.chat.get_conversation(id, claims.user_id).await?;
    Ok(Json(detail))
}

#[utoipa::path(
    patch,
    path = "/chat/conversations/{id}",
    tag = "chat",
    security(("bearer_auth" = [])),
    request_body = UpdateConversationRequest,
    responses(
        (status = 200, description = "Updated conversation", body = ChatConversation),
        (status = 404, description = "Not found")
    )
)]
pub async fn update_conversation(
    State(state): State<AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateConversationRequest>,
) -> AppResult<Json<ChatConversation>> {
    claims.require_chat()?;
    let conv = state.services.chat.update_conversation(id, claims.user_id, req).await?;
    Ok(Json(conv))
}

#[utoipa::path(
    delete,
    path = "/chat/conversations/{id}",
    tag = "chat",
    security(("bearer_auth" = [])),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found")
    )
)]
pub async fn delete_conversation(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<impl IntoResponse> {
    claims.require_chat()?;
    state.services.chat.delete_conversation(id, claims.user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/chat/conversations/{id}/messages",
    tag = "chat",
    security(("bearer_auth" = [])),
    request_body = SendChatMessageRequest,
    responses(
        (status = 200, description = "SSE stream (text/event-stream)"),
        (status = 404, description = "Not found")
    )
)]
pub async fn send_message(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>, Json(req): Json<SendChatMessageRequest>) -> AppResult<impl IntoResponse> {
    claims.require_chat()?;
    if !state.config.chat.enabled {
        return Err(crate::error::AppError::BusinessRule("Chat is disabled".into()));
    }

    let library_name = state.services.library_info.get().await.ok().and_then(|i| i.name).unwrap_or_else(|| "Elidune".into());

    let audit = state.services.audit.clone();
    let chat_cfg = state.config.chat.clone();
    let agent_stream = state.services.chat.send_message_stream(&state, audit, &chat_cfg, &claims, id, req, &library_name).await?;

    let conv_id = id;
    let chat_svc = state.services.chat.clone();

    let sse_stream = agent_stream.map(move |ev| {
        let data = serde_json::to_string(&ev).unwrap_or_default();
        Ok::<_, Infallible>(Event::default().event("message").data(data))
    });

    let stream = sse_stream.chain(futures_util::stream::once(async move {
        chat_svc.clear_session(conv_id).await;
        Ok(Event::default().comment("done"))
    }));

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

#[utoipa::path(
    post,
    path = "/chat/conversations/{id}/cancel",
    tag = "chat",
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Cancellation requested"))
)]
pub async fn cancel_message(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(id): Path<i64>) -> AppResult<impl IntoResponse> {
    claims.require_chat()?;
    state.services.chat.cancel_generation(id, claims.user_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
