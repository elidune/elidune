//! `chat_conversations` and `chat_messages` tables.

use async_trait::async_trait;
use serde_json::Value;
use snowflaked::Generator;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::chat::{ChatConversation, ChatMessage},
};

#[async_trait]
pub trait ChatRepository: Send + Sync {
    async fn chat_list_conversations(&self, user_id: i64) -> AppResult<Vec<ChatConversation>>;
    async fn chat_get_conversation(&self, id: i64, user_id: i64) -> AppResult<ChatConversation>;
    async fn chat_create_conversation(&self, user_id: i64, provider_id: i64, model: &str, title: &str) -> AppResult<ChatConversation>;
    async fn chat_update_conversation_title(&self, id: i64, user_id: i64, title: &str) -> AppResult<ChatConversation>;
    async fn chat_update_conversation_provider(&self, id: i64, user_id: i64, provider_id: i64, model: &str) -> AppResult<()>;
    async fn chat_touch_conversation(&self, id: i64, user_id: i64) -> AppResult<()>;
    async fn chat_delete_conversation(&self, id: i64, user_id: i64) -> AppResult<()>;
    async fn chat_list_messages(&self, conversation_id: i64) -> AppResult<Vec<ChatMessage>>;
    async fn chat_insert_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: Option<&str>,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        tool_arguments: Option<Value>,
        tool_result: Option<Value>,
    ) -> AppResult<ChatMessage>;
}

#[async_trait]
impl ChatRepository for Repository {
    async fn chat_list_conversations(&self, user_id: i64) -> AppResult<Vec<ChatConversation>> {
        Repository::chat_list_conversations(self, user_id).await
    }
    async fn chat_get_conversation(&self, id: i64, user_id: i64) -> AppResult<ChatConversation> {
        Repository::chat_get_conversation(self, id, user_id).await
    }
    async fn chat_create_conversation(&self, user_id: i64, provider_id: i64, model: &str, title: &str) -> AppResult<ChatConversation> {
        Repository::chat_create_conversation(self, user_id, provider_id, model, title).await
    }
    async fn chat_update_conversation_title(&self, id: i64, user_id: i64, title: &str) -> AppResult<ChatConversation> {
        Repository::chat_update_conversation_title(self, id, user_id, title).await
    }
    async fn chat_update_conversation_provider(&self, id: i64, user_id: i64, provider_id: i64, model: &str) -> AppResult<()> {
        Repository::chat_update_conversation_provider(self, id, user_id, provider_id, model).await
    }
    async fn chat_touch_conversation(&self, id: i64, user_id: i64) -> AppResult<()> {
        Repository::chat_touch_conversation(self, id, user_id).await
    }
    async fn chat_delete_conversation(&self, id: i64, user_id: i64) -> AppResult<()> {
        Repository::chat_delete_conversation(self, id, user_id).await
    }
    async fn chat_list_messages(&self, conversation_id: i64) -> AppResult<Vec<ChatMessage>> {
        Repository::chat_list_messages(self, conversation_id).await
    }
    async fn chat_insert_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: Option<&str>,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        tool_arguments: Option<Value>,
        tool_result: Option<Value>,
    ) -> AppResult<ChatMessage> {
        Repository::chat_insert_message(self, conversation_id, role, content, tool_name, tool_call_id, tool_arguments, tool_result).await
    }
}

impl Repository {
    pub async fn chat_list_conversations(&self, user_id: i64) -> AppResult<Vec<ChatConversation>> {
        sqlx::query_as::<_, ChatConversation>(
            r#"
            SELECT id, user_id, title, provider_id, model, created_at, updated_at
            FROM chat_conversations
            WHERE user_id = $1
            ORDER BY updated_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn chat_get_conversation(&self, id: i64, user_id: i64) -> AppResult<ChatConversation> {
        sqlx::query_as::<_, ChatConversation>(
            r#"
            SELECT id, user_id, title, provider_id, model, created_at, updated_at
            FROM chat_conversations
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".into()))
    }

    pub async fn chat_create_conversation(&self, user_id: i64, provider_id: i64, model: &str, title: &str) -> AppResult<ChatConversation> {
        let id: i64 = Generator::new(5).generate();
        sqlx::query_as::<_, ChatConversation>(
            r#"
            INSERT INTO chat_conversations (id, user_id, title, provider_id, model)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, user_id, title, provider_id, model, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(title)
        .bind(provider_id)
        .bind(model)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn chat_update_conversation_title(&self, id: i64, user_id: i64, title: &str) -> AppResult<ChatConversation> {
        sqlx::query_as::<_, ChatConversation>(
            r#"
            UPDATE chat_conversations
            SET title = $3, updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING id, user_id, title, provider_id, model, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(title)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".into()))
    }

    pub async fn chat_update_conversation_provider(&self, id: i64, user_id: i64, provider_id: i64, model: &str) -> AppResult<()> {
        let r = sqlx::query(
            r#"
            UPDATE chat_conversations
            SET provider_id = $3, model = $4, updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(provider_id)
        .bind(model)
        .execute(&self.pool)
        .await?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound("Conversation not found".into()));
        }
        Ok(())
    }

    pub async fn chat_touch_conversation(&self, id: i64, user_id: i64) -> AppResult<()> {
        let r = sqlx::query("UPDATE chat_conversations SET updated_at = NOW() WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound("Conversation not found".into()));
        }
        Ok(())
    }

    pub async fn chat_delete_conversation(&self, id: i64, user_id: i64) -> AppResult<()> {
        let r = sqlx::query("DELETE FROM chat_conversations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound("Conversation not found".into()));
        }
        Ok(())
    }

    pub async fn chat_list_messages(&self, conversation_id: i64) -> AppResult<Vec<ChatMessage>> {
        sqlx::query_as::<_, ChatMessage>(
            r#"
            SELECT id, conversation_id, role, content, tool_name, tool_call_id,
                   tool_arguments, tool_result, created_at
            FROM chat_messages
            WHERE conversation_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn chat_insert_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: Option<&str>,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        tool_arguments: Option<Value>,
        tool_result: Option<Value>,
    ) -> AppResult<ChatMessage> {
        let id: i64 = Generator::new(5).generate();
        sqlx::query_as::<_, ChatMessage>(
            r#"
            INSERT INTO chat_messages
                (id, conversation_id, role, content, tool_name, tool_call_id, tool_arguments, tool_result)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, conversation_id, role, content, tool_name, tool_call_id,
                      tool_arguments, tool_result, created_at
            "#,
        )
        .bind(id)
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(tool_name)
        .bind(tool_call_id)
        .bind(tool_arguments)
        .bind(tool_result)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }
}
