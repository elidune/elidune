//! LLM provider backends (one implementation per native / compat API).

mod anthropic;
mod gemini;
mod grok;
mod http;
mod ollama;
mod openai;
mod chat_completions;
mod chat_completions_sse;
mod gemini_messages;

pub use anthropic::AnthropicBackend;
pub use gemini::GeminiBackend;
pub use grok::GrokBackend;
pub use ollama::OllamaBackend;
pub use openai::OpenAiBackend;
pub use chat_completions::ChatCompletionsBackend;

use std::sync::Arc;

use crate::error::{AppError, AppResult};
use crate::models::chat::{LlmProviderKind, LlmProviderRow};

use super::llm::LlmBackend;

pub fn resolve_api_key(row: &LlmProviderRow) -> Option<String> {
    if let Some(ref k) = row.api_key {
        if !k.is_empty() {
            return Some(k.clone());
        }
    }
    if let Some(ref env) = row.api_key_env {
        if !env.is_empty() {
            return std::env::var(env).ok().filter(|s| !s.is_empty());
        }
    }
    None
}

pub fn build_backend(row: &LlmProviderRow, timeout_secs: u64) -> AppResult<Arc<dyn LlmBackend>> {
    let api_key = resolve_api_key(row);

    let kind = LlmProviderKind::from_db(&row.kind)
        .ok_or_else(|| AppError::Internal(format!("Unknown LLM kind: {}", row.kind)))?;

    match kind {
        LlmProviderKind::OpenAi | LlmProviderKind::OpenaiCompat => Ok(Arc::new(OpenAiBackend::new(
            row.base_url.clone(),
            api_key,
            timeout_secs,
        )) as Arc<dyn LlmBackend>),
        LlmProviderKind::Ollama => Ok(Arc::new(OllamaBackend::new(row.base_url.clone(), timeout_secs)) as Arc<dyn LlmBackend>),
        LlmProviderKind::Gemini => Ok(Arc::new(GeminiBackend::new(
            row.base_url.clone(),
            api_key,
            timeout_secs,
        )) as Arc<dyn LlmBackend>),
        LlmProviderKind::Grok => Ok(Arc::new(GrokBackend::new(
            row.base_url.clone(),
            api_key,
            timeout_secs,
        )) as Arc<dyn LlmBackend>),
        LlmProviderKind::Anthropic => {
            let key = api_key.ok_or_else(|| AppError::BusinessRule("Anthropic provider requires an API key".into()))?;
            Ok(Arc::new(AnthropicBackend::new(row.base_url.clone(), key, timeout_secs)) as Arc<dyn LlmBackend>)
        }
    }
}
