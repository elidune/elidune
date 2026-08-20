//! Chat conversations service.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::{
    config::ChatConfig,
    error::{AppError, AppResult},
    models::chat::{ChatConversation, ConversationDetail, CreateConversationRequest, SendChatMessageRequest, UpdateConversationRequest},
    models::user::UserClaims,
    repository::{ChatRepository, LlmProvidersRepository},
    services::{audit::AuditService, chat::run_agent_turn, llm_providers::LlmProvidersService},
    AppState,
};

pub use crate::services::chat_session_registry::ChatSessionRegistry;

#[derive(Clone)]
pub struct ChatService {
    pub(crate) chat_repo: Arc<dyn ChatRepository>,
    llm_repo: Arc<dyn LlmProvidersRepository>,
    llm_svc: LlmProvidersService,
    sessions: ChatSessionRegistry,
}

impl ChatService {
    pub fn new(chat_repo: Arc<dyn ChatRepository>, llm_repo: Arc<dyn LlmProvidersRepository>) -> Self {
        let llm_svc = LlmProvidersService::new(llm_repo.clone());
        Self {
            chat_repo,
            llm_repo,
            llm_svc,
            sessions: ChatSessionRegistry::new(),
        }
    }

    pub fn llm(&self) -> &LlmProvidersService {
        &self.llm_svc
    }

    pub async fn list_conversations(&self, user_id: i64) -> AppResult<Vec<ChatConversation>> {
        self.chat_repo.chat_list_conversations(user_id).await
    }

    pub async fn get_conversation(&self, id: i64, user_id: i64) -> AppResult<ConversationDetail> {
        let conversation = self.chat_repo.chat_get_conversation(id, user_id).await?;
        let messages = self.chat_repo.chat_list_messages(id).await?;
        Ok(ConversationDetail { conversation, messages })
    }

    pub async fn create_conversation(&self, user_id: i64, req: CreateConversationRequest) -> AppResult<ChatConversation> {
        let row = self.llm_svc.get_row(req.provider_id).await?;
        if !row.enabled {
            return Err(AppError::Validation("LLM provider is disabled".into()));
        }
        let model = LlmProvidersService::resolve_model(&row, req.model.as_deref())?;
        let title = req.title.unwrap_or_else(|| "Nouvelle conversation".to_string());
        self.chat_repo.chat_create_conversation(user_id, req.provider_id, &model, &title).await
    }

    pub async fn update_conversation(&self, id: i64, user_id: i64, req: UpdateConversationRequest) -> AppResult<ChatConversation> {
        if let Some(title) = req.title {
            return self.chat_repo.chat_update_conversation_title(id, user_id, &title).await;
        }
        self.chat_repo.chat_get_conversation(id, user_id).await
    }

    pub async fn delete_conversation(&self, id: i64, user_id: i64) -> AppResult<()> {
        self.sessions.cancel(id).await;
        self.chat_repo.chat_delete_conversation(id, user_id).await
    }

    pub async fn cancel_generation(&self, conversation_id: i64, user_id: i64) -> AppResult<()> {
        self.chat_repo.chat_get_conversation(conversation_id, user_id).await?;
        self.sessions.cancel(conversation_id).await;
        Ok(())
    }

    pub async fn clear_session(&self, conversation_id: i64) {
        self.sessions.clear(conversation_id).await;
    }

    pub async fn send_message_stream(
        &self,
        state: &AppState,
        audit: AuditService,
        chat_cfg: &ChatConfig,
        claims: &UserClaims,
        conversation_id: i64,
        req: SendChatMessageRequest,
        library_name: &str,
    ) -> AppResult<impl futures_core::Stream<Item = crate::services::chat::AgentSseEvent> + Send> {
        let content = req.content.trim();
        if content.is_empty() {
            return Err(AppError::Validation("content must not be empty".into()));
        }

        let conv = self.chat_repo.chat_get_conversation(conversation_id, claims.user_id).await?;

        let provider_id = req.provider_id.unwrap_or(conv.provider_id);
        let row = self.llm_svc.get_row(provider_id).await?;
        if !row.enabled {
            return Err(AppError::Validation("LLM provider is disabled".into()));
        }
        let model = LlmProvidersService::resolve_model(&row, req.model.as_deref().or(Some(conv.model.as_str())))?;

        if provider_id != conv.provider_id || model != conv.model {
            self.chat_repo.chat_update_conversation_provider(conversation_id, claims.user_id, provider_id, &model).await?;
        }

        let cancel = self.sessions.register(conversation_id).await;
        let show_tool_detail = claims.is_librarian();

        run_agent_turn(
            state,
            self.chat_repo.clone(),
            self.llm_repo.clone(),
            audit,
            chat_cfg,
            claims,
            conversation_id,
            claims.user_id,
            content,
            provider_id,
            &model,
            library_name,
            show_tool_detail,
            cancel,
        )
        .await
    }
}
