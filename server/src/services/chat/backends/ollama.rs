//! Ollama local backend (`/chat/completions` protocol, no auth).

use std::sync::Arc;

use super::chat_completions::{ChatCompletionsBackend, OllamaAdapter};

pub struct OllamaBackend(ChatCompletionsBackend);

impl OllamaBackend {
    pub fn new(base_url: String, timeout_secs: u64) -> Self {
        Self(ChatCompletionsBackend::new(
            base_url,
            None,
            timeout_secs,
            Arc::new(OllamaAdapter),
        ))
    }
}

#[async_trait::async_trait]
impl super::super::llm::LlmBackend for OllamaBackend {
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
