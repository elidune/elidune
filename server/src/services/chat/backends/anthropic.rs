//! Anthropic Messages API backend.

use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::io::StreamReader;

use super::http::{llm_client, trim_base_url};
use super::super::llm::{LlmBackend, LlmChatMessage, LlmStream, LlmStreamChunk, LlmToolCall, LlmToolDef};
use crate::error::{AppError, AppResult};

pub struct AnthropicBackend {
    client: Client,
    base_url: String,
    api_key: String,
    timeout_secs: u64,
}

impl AnthropicBackend {
    pub fn new(base_url: String, api_key: String, timeout_secs: u64) -> Self {
        Self {
            client: llm_client(timeout_secs),
            base_url: trim_base_url(&base_url),
            api_key,
            timeout_secs,
        }
    }

    fn url(&self) -> String {
        if self.base_url.contains("/messages") {
            self.base_url.clone()
        } else {
            format!("{}/messages", self.base_url)
        }
    }

    fn build_body(model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> Value {
        let mut system = String::new();
        let mut msgs: Vec<Value> = Vec::new();

        for m in messages {
            if m.role == "system" {
                if let Some(ref c) = m.content {
                    system.push_str(c);
                }
                continue;
            }
            if m.role == "tool" {
                msgs.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id,
                        "content": m.content.as_deref().unwrap_or("")
                    }]
                }));
                continue;
            }
            if m.role == "assistant" {
                if let Some(ref tcs) = m.tool_calls {
                    let mut blocks: Vec<Value> = Vec::new();
                    if let Some(ref c) = m.content {
                        if !c.is_empty() {
                            blocks.push(json!({ "type": "text", "text": c }));
                        }
                    }
                    for tc in tcs {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments
                        }));
                    }
                    msgs.push(json!({ "role": "assistant", "content": blocks }));
                } else {
                    msgs.push(json!({ "role": "assistant", "content": m.content }));
                }
                continue;
            }
            msgs.push(json!({ "role": "user", "content": m.content }));
        }

        let mut body = json!({
            "model": model,
            "max_tokens": 4096,
            "messages": msgs,
            "stream": true
        });
        if !system.is_empty() {
            body["system"] = json!(system);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters
                    })
                })
                .collect::<Vec<_>>());
        }
        body
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<AnthropicDelta>,
    content_block: Option<AnthropicContentBlock>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
    partial_json: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    id: Option<String>,
    name: Option<String>,
}

#[async_trait]
impl LlmBackend for AnthropicBackend {
    async fn stream_chat(&self, model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> AppResult<LlmStream> {
        let body = Self::build_body(model, messages, tools);
        let resp = self
            .client
            .post(self.url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::BusinessRule(format!("Anthropic request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::BusinessRule(format!("Anthropic returned {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        let reader = StreamReader::new(byte_stream.map(|r| r.map_err(std::io::Error::other)));
        let lines = tokio_util::io::ReaderStream::new(reader);

        let stream = async_stream::stream! {
            use futures_util::TryStreamExt;
            let mut lines = lines.map_ok(|bytes| String::from_utf8_lossy(&bytes).to_string());
            let mut buffer = String::new();
            let mut full_content = String::new();
            let mut finished_tool_calls: Vec<LlmToolCall> = Vec::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_args = String::new();

            while let Some(line_result) = lines.next().await {
                let line = match line_result {
                    Ok(l) => l,
                    Err(e) => {
                        yield Err(AppError::BusinessRule(format!("Anthropic stream error: {e}")));
                        return;
                    }
                };
                buffer.push_str(&line);
                while let Some(pos) = buffer.find('\n') {
                    let raw = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();
                    if !raw.starts_with("data:") {
                        continue;
                    }
                    let data = raw.strip_prefix("data:").map(str::trim).unwrap_or("");
                    if data.is_empty() {
                        continue;
                    }
                    let parsed: AnthropicEvent = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    match parsed.event_type.as_str() {
                        "content_block_delta" => {
                            if let Some(delta) = parsed.delta {
                                if let Some(text) = delta.text {
                                    full_content.push_str(&text);
                                    yield Ok(LlmStreamChunk::Delta(text));
                                }
                                if let Some(pj) = delta.partial_json {
                                    current_tool_args.push_str(&pj);
                                }
                            }
                        }
                        "content_block_start" => {
                            if let Some(block) = parsed.content_block {
                                if block.block_type.as_deref() == Some("tool_use") {
                                    current_tool_id = block.id.unwrap_or_default();
                                    current_tool_name = block.name.unwrap_or_default();
                                    current_tool_args.clear();
                                }
                            }
                        }
                        "content_block_stop" => {
                            if !current_tool_id.is_empty() && !current_tool_name.is_empty() {
                                let arguments: Value = serde_json::from_str(&current_tool_args)
                                    .unwrap_or(json!({}));
                                let tc = LlmToolCall {
                                    id: current_tool_id.clone(),
                                    name: current_tool_name.clone(),
                                    arguments,
                                    thought_signature: None,
                                };
                                finished_tool_calls.push(tc.clone());
                                yield Ok(LlmStreamChunk::ToolCall(tc));
                                current_tool_id.clear();
                                current_tool_name.clear();
                                current_tool_args.clear();
                            }
                        }
                        "message_stop" => {
                            yield Ok(LlmStreamChunk::Done {
                                content: if full_content.is_empty() { None } else { Some(full_content.clone()) },
                                tool_calls: finished_tool_calls.clone(),
                            });
                            return;
                        }
                        _ => {}
                    }
                }
            }
            yield Ok(LlmStreamChunk::Done {
                content: if full_content.is_empty() { None } else { Some(full_content) },
                tool_calls: finished_tool_calls,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn test_connection(&self, model: &str) -> AppResult<()> {
        let body = json!({
            "model": model,
            "max_tokens": 16,
            "messages": [{ "role": "user", "content": "ping" }]
        });
        let resp = self
            .client
            .post(self.url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(15))
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::BusinessRule(format!("Anthropic test failed: {e}")))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::BusinessRule(format!("Anthropic test returned {}", resp.status())))
        }
    }
}
