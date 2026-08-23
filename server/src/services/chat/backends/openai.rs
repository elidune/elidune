//! OpenAI API backend (`https://api.openai.com/v1`).

use std::sync::Arc;

use super::chat_completions::{ChatCompletionsBackend, StandardChatCompletionsAdapter};

pub struct OpenAiBackend(ChatCompletionsBackend);

impl OpenAiBackend {
    pub fn new(base_url: String, api_key: Option<String>, timeout_secs: u64) -> Self {
        Self(ChatCompletionsBackend::new(
            base_url,
            api_key,
            timeout_secs,
            Arc::new(StandardChatCompletionsAdapter),
        ))
    }
}

impl std::ops::Deref for OpenAiBackend {
    type Target = ChatCompletionsBackend;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for OpenAiBackend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[async_trait::async_trait]
impl super::super::llm::LlmBackend for OpenAiBackend {
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
