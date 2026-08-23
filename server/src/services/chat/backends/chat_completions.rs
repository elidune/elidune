//! Shared `/chat/completions` engine with per-provider adapter hooks.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder};
use serde_json::{json, Value};
use tokio_util::io::StreamReader;

use super::http::{llm_client, trim_base_url};
use super::chat_completions_sse::{ChatCompletionsSseMode, ChatCompletionsSseParser};
use super::gemini_messages::normalize_gemini_messages;
use super::super::llm::{LlmBackend, LlmChatMessage, LlmStream, LlmStreamChunk, LlmToolCall, LlmToolDef};
use crate::error::{AppError, AppResult};

/// Provider-specific behaviour for `/chat/completions` APIs.
pub trait ChatCompletionsAdapter: Send + Sync {
    fn sse_parser_mode(&self) -> ChatCompletionsSseMode {
        ChatCompletionsSseMode::Standard
    }

    fn apply_auth(&self, req: RequestBuilder, api_key: Option<&str>) -> RequestBuilder {
        if let Some(key) = api_key {
            req.bearer_auth(key)
        } else {
            req
        }
    }

    fn encode_tool_call(&self, tc: &LlmToolCall) -> Value {
        json!({
            "id": tc.id,
            "type": "function",
            "function": {
                "name": tc.name,
                "arguments": tc.arguments.to_string()
            }
        })
    }

    fn encode_message(&self, m: &LlmChatMessage) -> Value;

    fn test_connection_path(&self) -> &'static str {
        "/models"
    }
}

fn default_encode_message(adapter: &dyn ChatCompletionsAdapter, m: &LlmChatMessage) -> Value {
    let mut obj = json!({ "role": m.role });
    if let Some(ref c) = m.content {
        obj["content"] = json!(c);
    }
    if let Some(ref tcs) = m.tool_calls {
        obj["tool_calls"] = json!(tcs.iter().map(|tc| adapter.encode_tool_call(tc)).collect::<Vec<_>>());
    }
    if let Some(ref id) = m.tool_call_id {
        obj["tool_call_id"] = json!(id);
    }
    if let Some(ref name) = m.name {
        if !name.is_empty() {
            obj["name"] = json!(name);
        }
    }
    obj
}

pub struct StandardChatCompletionsAdapter;

impl ChatCompletionsAdapter for StandardChatCompletionsAdapter {
    fn encode_message(&self, m: &LlmChatMessage) -> Value {
        default_encode_message(self, m)
    }
}

pub struct OllamaAdapter;

impl ChatCompletionsAdapter for OllamaAdapter {
    fn apply_auth(&self, req: RequestBuilder, _api_key: Option<&str>) -> RequestBuilder {
        req
    }

    fn encode_message(&self, m: &LlmChatMessage) -> Value {
        default_encode_message(self, m)
    }
}

pub struct GeminiAdapter;

impl ChatCompletionsAdapter for GeminiAdapter {
    fn sse_parser_mode(&self) -> ChatCompletionsSseMode {
        ChatCompletionsSseMode::Gemini
    }

    fn encode_tool_call(&self, tc: &LlmToolCall) -> Value {
        let mut call = json!({
            "id": tc.id,
            "type": "function",
            "function": {
                "name": tc.name,
                "arguments": tc.arguments.to_string()
            }
        });
        if let Some(ref sig) = tc.thought_signature {
            call["extra_content"] = json!({
                "google": { "thought_signature": sig }
            });
        }
        call
    }

    fn encode_message(&self, m: &LlmChatMessage) -> Value {
        if m.role == "tool" {
            let name = m.name.as_deref().filter(|n| !n.is_empty()).unwrap_or_else(|| {
                tracing::warn!(tool_call_id = ?m.tool_call_id, "Gemini tool message missing name; using placeholder");
                "unknown_tool"
            });
            let mut obj = json!({
                "role": "tool",
                "name": name,
                "content": m.content.as_deref().unwrap_or(""),
            });
            if let Some(ref id) = m.tool_call_id {
                obj["tool_call_id"] = json!(id);
            }
            return obj;
        }
        if m.role == "assistant" {
            let mut obj = json!({ "role": "assistant" });
            if let Some(ref tcs) = m.tool_calls {
                if !tcs.is_empty() {
                    obj["tool_calls"] = json!(tcs.iter().map(|tc| self.encode_tool_call(tc)).collect::<Vec<_>>());
                }
            }
            match m.content.as_deref() {
                Some("") | None if m.tool_calls.as_ref().is_some_and(|t| !t.is_empty()) => {}
                Some(text) => {
                    obj["content"] = json!(text);
                }
                None => {}
            }
            return obj;
        }
        default_encode_message(self, m)
    }
}

pub struct GrokAdapter;

impl ChatCompletionsAdapter for GrokAdapter {
    fn encode_message(&self, m: &LlmChatMessage) -> Value {
        default_encode_message(self, m)
    }
}

pub struct ChatCompletionsBackend {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    adapter: Arc<dyn ChatCompletionsAdapter>,
}

impl ChatCompletionsBackend {
    pub fn new(base_url: String, api_key: Option<String>, timeout_secs: u64, adapter: Arc<dyn ChatCompletionsAdapter>) -> Self {
        Self {
            client: llm_client(timeout_secs),
            base_url: trim_base_url(&base_url),
            api_key,
            adapter,
        }
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn test_url(&self) -> String {
        format!("{}{}", self.base_url, self.adapter.test_connection_path())
    }

    fn build_body(model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef], adapter: &dyn ChatCompletionsAdapter) -> Value {
        let normalized = if adapter.sse_parser_mode() == ChatCompletionsSseMode::Gemini {
            normalize_gemini_messages(messages)
        } else {
            messages.to_vec()
        };
        let msgs: Vec<Value> = normalized.iter().map(|m| adapter.encode_message(m)).collect();

        let mut body = json!({
            "model": model,
            "messages": msgs,
            "stream": true
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect::<Vec<_>>());
        }

        body
    }
}

#[async_trait]
impl LlmBackend for ChatCompletionsBackend {
    async fn stream_chat(&self, model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> AppResult<LlmStream> {
        let body = Self::build_body(model, messages, tools, self.adapter.as_ref());
        let req = self
            .client
            .post(self.chat_url())
            .json(&body);
        let req = self.adapter.apply_auth(req, self.api_key.as_deref());

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::BusinessRule(format!("LLM request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::BusinessRule(format!("LLM returned {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        let reader = StreamReader::new(byte_stream.map(|r| r.map_err(std::io::Error::other)));
        let lines = tokio_util::io::ReaderStream::new(reader);
        let parser_mode = self.adapter.sse_parser_mode();

        let stream = async_stream::stream! {
            use futures_util::TryStreamExt;
            let mut lines = lines.map_ok(|bytes| String::from_utf8_lossy(&bytes).to_string());
            let mut parser = ChatCompletionsSseParser::new(parser_mode);

            while let Some(chunk_result) = lines.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(AppError::BusinessRule(format!("LLM stream read error: {e}")));
                        return;
                    }
                };
                for item in parser.push(&chunk) {
                    let done = matches!(item, LlmStreamChunk::Done { .. });
                    yield Ok(item);
                    if done {
                        return;
                    }
                }
            }
            for item in parser.finish() {
                yield Ok(item);
            }
        };

        Ok(Box::pin(stream))
    }

    async fn test_connection(&self, _model: &str) -> AppResult<()> {
        let req = self.client.get(self.test_url());
        let req = self.adapter.apply_auth(req, self.api_key.as_deref());
        let resp = req
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| AppError::BusinessRule(format!("LLM test failed: {e}")))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::BusinessRule(format!("LLM test returned {}", resp.status())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_tool_message_always_includes_name() {
        let msg = LlmChatMessage {
            role: "tool".into(),
            content: Some("{}".into()),
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
            name: None,
        };
        let encoded = GeminiAdapter.encode_message(&msg);
        assert_eq!(encoded["role"], "tool");
        assert_eq!(encoded["name"], "unknown_tool");
        assert_eq!(encoded["tool_call_id"], "call_1");
    }

    #[test]
    fn gemini_adapter_echoes_thought_signature() {
        let tc = LlmToolCall {
            id: "call_1".into(),
            name: "list_tables".into(),
            arguments: json!({}),
            thought_signature: Some("sig_abc".into()),
        };
        let call = GeminiAdapter.encode_tool_call(&tc);
        let sig = call["extra_content"]["google"]["thought_signature"].as_str();
        assert_eq!(sig, Some("sig_abc"));
    }
}
