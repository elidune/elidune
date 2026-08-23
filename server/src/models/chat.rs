//! Chat assistant models (conversations, messages, LLM providers).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

/// LLM backend kind (one variant per supported provider API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum LlmProviderKind {
    OpenAi,
    Ollama,
    Gemini,
    Grok,
    Anthropic,
    /// Legacy / generic OpenAI-compatible endpoint (custom base URL).
    #[serde(rename = "openaiCompat")]
    OpenaiCompat,
}

impl LlmProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Ollama => "ollama",
            Self::Gemini => "gemini",
            Self::Grok => "grok",
            Self::Anthropic => "anthropic",
            Self::OpenaiCompat => "openaiCompat",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(Self::OpenAi),
            "ollama" => Some(Self::Ollama),
            "gemini" => Some(Self::Gemini),
            "grok" => Some(Self::Grok),
            "anthropic" => Some(Self::Anthropic),
            "openaiCompat" => Some(Self::OpenaiCompat),
            _ => None,
        }
    }
}

/// Full row from `llm_providers` (includes secret; never expose via API).
#[derive(Debug, Clone, FromRow)]
pub struct LlmProviderRow {
    pub id: i64,
    pub slug: String,
    pub label: String,
    pub kind: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub models: Value,
    pub default_model: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub slug: String,
    pub label: String,
    pub kind: LlmProviderKind,
    pub models: Vec<String>,
    pub default_model: Option<String>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderAdmin {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub slug: String,
    pub label: String,
    pub kind: LlmProviderKind,
    pub base_url: String,
    pub api_key_set: bool,
    pub api_key_env: Option<String>,
    pub models: Vec<String>,
    pub default_model: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLlmProviderRequest {
    pub slug: String,
    pub label: String,
    pub kind: LlmProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    pub models: Vec<String>,
    pub default_model: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLlmProviderRequest {
    pub slug: Option<String>,
    pub label: Option<String>,
    pub kind: Option<LlmProviderKind>,
    pub base_url: Option<String>,
    /// When omitted, the stored key is unchanged. Empty string clears the key.
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub models: Option<Vec<String>>,
    pub default_model: Option<String>,
    pub enabled: Option<bool>,
    pub sort_order: Option<i32>,
}

fn default_true() -> bool {
    true
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatConversation {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    pub title: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub provider_id: i64,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageRole {
    User,
    Assistant,
    Tool,
}

impl ChatMessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "tool" => Some(Self::Tool),
            _ => None,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub conversation_id: i64,
    pub role: String,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_arguments: Option<Value>,
    pub tool_result: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationRequest {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub provider_id: i64,
    pub model: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConversationRequest {
    pub title: Option<String>,
}

#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendChatMessageRequest {
    pub content: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub provider_id: Option<i64>,
    pub model: Option<String>,
}

#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    #[serde(flatten)]
    pub conversation: ChatConversation,
    pub messages: Vec<ChatMessage>,
}
