//! LLM provider trait and shared types.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use serde_json::Value;

use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct LlmToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    /// Gemini OpenAI-compat `extra_content.google.thought_signature` (echoed on the next turn).
    pub thought_signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LlmChatMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlmStreamChunk {
    Delta(String),
    ToolCall(LlmToolCall),
    Done { content: Option<String>, tool_calls: Vec<LlmToolCall> },
}

pub type LlmStream = Pin<Box<dyn Stream<Item = AppResult<LlmStreamChunk>> + Send>>;

#[async_trait]
pub trait LlmBackend: Send + Sync {
    async fn stream_chat(&self, model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> AppResult<LlmStream>;

    async fn test_connection(&self, model: &str) -> AppResult<()>;
}
