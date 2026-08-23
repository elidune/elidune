//! xAI Grok backend (`/chat/completions` protocol).

use std::sync::Arc;

use super::chat_completions::{ChatCompletionsBackend, GrokAdapter};

pub struct GrokBackend(ChatCompletionsBackend);

impl GrokBackend {
    pub fn new(base_url: String, api_key: Option<String>, timeout_secs: u64) -> Self {
        Self(ChatCompletionsBackend::new(
            base_url,
            api_key,
            timeout_secs,
            Arc::new(GrokAdapter),
        ))
    }
}

#[async_trait::async_trait]
impl super::super::llm::LlmBackend for GrokBackend {
    async fn stream_chat(
        &self,
        model: &str,
        messages: &[super::super::llm::LlmChatMessage],
        tools: &[super::super::llm::LlmToolDef],
    ) -> crate::error::AppResult<super::super::llm::LlmStream> {
        self.0.stream_chat(model, messages, tools).await
    }

    async fn test_connection(&self, model: &str) -> crate::error::AppResult<()> {
        self.0.test_connection(model).await
    }
}
