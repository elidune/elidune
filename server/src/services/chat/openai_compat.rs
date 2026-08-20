//! OpenAI-compatible chat completions (OpenAI, Ollama, Gemini OpenAI endpoint, …).

use std::collections::BTreeMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::io::StreamReader;

use super::llm::{LlmBackend, LlmChatMessage, LlmStream, LlmStreamChunk, LlmToolCall, LlmToolDef};
use crate::error::{AppError, AppResult};

pub struct OpenAiCompatBackend {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    timeout_secs: u64,
}

impl OpenAiCompatBackend {
    pub fn new(base_url: String, api_key: Option<String>, timeout_secs: u64) -> Self {
        let client = Client::builder().timeout(std::time::Duration::from_secs(timeout_secs)).build().unwrap_or_else(|_| Client::new());
        let base = base_url.trim_end_matches('/').to_string();
        Self {
            client,
            base_url: base,
            api_key,
            timeout_secs,
        }
    }

    fn url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.base_url)
    }

    fn build_body(model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> Value {
        let msgs: Vec<Value> = messages
            .iter()
            .map(|m| {
                let mut obj = json!({ "role": m.role });
                if let Some(ref c) = m.content {
                    obj["content"] = json!(c);
                }
                if let Some(ref tcs) = m.tool_calls {
                    obj["tool_calls"] = json!(tcs
                        .iter()
                        .map(|tc| {
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
                        })
                        .collect::<Vec<_>>());
                }
                if let Some(ref id) = m.tool_call_id {
                    obj["tool_call_id"] = json!(id);
                }
                if let Some(ref name) = m.name {
                    obj["name"] = json!(name);
                }
                obj
            })
            .collect();

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

#[derive(Debug, Deserialize)]
struct StreamChoiceDelta {
    content: Option<String>,
    tool_calls: Option<Vec<StreamToolCallDelta>>,
    extra_content: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StreamMessage {
    content: Option<String>,
    tool_calls: Option<Vec<StreamToolCallDelta>>,
    extra_content: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCallDelta {
    index: Option<usize>,
    id: Option<String>,
    function: Option<StreamFunctionDelta>,
    extra_content: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StreamFunctionDelta {
    name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_args_fragment")]
    arguments: Option<String>,
}

fn thought_signature_from_extra(extra: Option<&Value>) -> Option<String> {
    extra?.get("google")?.get("thought_signature")?.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

fn deserialize_args_fragment<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.map(|v| match v {
        Value::String(s) => s,
        other => other.to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Option<Vec<StreamChoice>>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<StreamChoiceDelta>,
    message: Option<StreamMessage>,
    finish_reason: Option<String>,
}

#[derive(Default)]
struct PendingTool {
    id: String,
    name: String,
    arguments: String,
    thought_signature: String,
}

struct OpenAiSseParser {
    buffer: String,
    pending_tools: BTreeMap<usize, PendingTool>,
    pending_thought_signature: Option<String>,
    full_content: String,
    finished_tool_calls: Vec<LlmToolCall>,
    emitted_done: bool,
}

impl OpenAiSseParser {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            pending_tools: BTreeMap::new(),
            pending_thought_signature: None,
            full_content: String::new(),
            finished_tool_calls: Vec::new(),
            emitted_done: false,
        }
    }

    fn push(&mut self, chunk: &str) -> Vec<LlmStreamChunk> {
        self.buffer.push_str(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buffer.find('\n') {
            let raw = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 1..].to_string();
            if self.handle_line(raw.trim(), &mut out) {
                break;
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<LlmStreamChunk> {
        let mut out = Vec::new();
        if !self.buffer.trim().is_empty() {
            let leftover = std::mem::take(&mut self.buffer);
            self.handle_line(leftover.trim(), &mut out);
        }
        out.extend(self.flush_pending_tools());
        if !self.emitted_done {
            out.push(self.done_chunk());
            self.emitted_done = true;
        }
        out
    }

    /// Returns true when a terminal `[DONE]` chunk was emitted.
    fn handle_line(&mut self, raw: &str, out: &mut Vec<LlmStreamChunk>) -> bool {
        if raw.is_empty() || raw.starts_with(':') {
            return false;
        }
        let data = raw.strip_prefix("data:").map(str::trim).unwrap_or(raw);
        if data == "[DONE]" {
            out.extend(self.flush_pending_tools());
            if !self.emitted_done {
                out.push(self.done_chunk());
                self.emitted_done = true;
            }
            return true;
        }
        let parsed: StreamChunk = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let Some(choices) = parsed.choices else {
            return false;
        };
        let Some(choice) = choices.into_iter().next() else {
            return false;
        };

        if let Some(delta) = choice.delta {
            self.ingest_content(delta.content, out);
            if let Some(tcs) = delta.tool_calls {
                self.ingest_tool_deltas(tcs);
            }
            self.apply_thought_signature(delta.extra_content.as_ref());
        }
        if let Some(message) = choice.message {
            self.ingest_content(message.content, out);
            if let Some(tcs) = message.tool_calls {
                self.ingest_tool_deltas(tcs);
            }
            self.apply_thought_signature(message.extra_content.as_ref());
        }
        if choice.finish_reason.is_some() {
            out.extend(self.flush_pending_tools());
        }
        false
    }

    fn ingest_content(&mut self, content: Option<String>, out: &mut Vec<LlmStreamChunk>) {
        let Some(content) = content else {
            return;
        };
        if content.is_empty() {
            return;
        }
        self.full_content.push_str(&content);
        out.push(LlmStreamChunk::Delta(content));
    }

    fn ingest_tool_deltas(&mut self, tcs: Vec<StreamToolCallDelta>) {
        for (fallback_idx, tc) in tcs.into_iter().enumerate() {
            let idx = tc.index.unwrap_or(fallback_idx);
            let entry = self.pending_tools.entry(idx).or_default();
            if let Some(id) = tc.id {
                if !id.is_empty() {
                    entry.id = id;
                }
            }
            if entry.thought_signature.is_empty() {
                if let Some(sig) = thought_signature_from_extra(tc.extra_content.as_ref()) {
                    entry.thought_signature = sig;
                } else if let Some(sig) = self.pending_thought_signature.take() {
                    entry.thought_signature = sig;
                }
            }
            let Some(f) = tc.function else {
                continue;
            };
            if let Some(n) = f.name {
                entry.name.push_str(&n);
            }
            if let Some(a) = f.arguments {
                entry.arguments.push_str(&a);
            }
        }
    }

    fn apply_thought_signature(&mut self, extra: Option<&Value>) {
        let Some(sig) = thought_signature_from_extra(extra) else {
            return;
        };
        if let Some(tool) = self.pending_tools.values_mut().find(|t| t.thought_signature.is_empty()) {
            tool.thought_signature = sig;
            return;
        }
        if let Some(tc) = self.finished_tool_calls.iter_mut().find(|t| t.thought_signature.is_none()) {
            tc.thought_signature = Some(sig);
            return;
        }
        self.pending_thought_signature = Some(sig);
    }

    fn flush_pending_tools(&mut self) -> Vec<LlmStreamChunk> {
        let pending = std::mem::take(&mut self.pending_tools);
        let mut out = Vec::new();
        for (idx, tool) in pending {
            if tool.name.is_empty() {
                continue;
            }
            let id = if tool.id.is_empty() { format!("call_{idx}") } else { tool.id };
            let arguments: Value = serde_json::from_str(&tool.arguments).unwrap_or_else(|_| if tool.arguments.is_empty() { json!({}) } else { json!({ "raw": tool.arguments }) });
            let tc = LlmToolCall {
                id,
                name: tool.name,
                arguments,
                thought_signature: if tool.thought_signature.is_empty() { None } else { Some(tool.thought_signature) },
            };
            self.finished_tool_calls.push(tc.clone());
            out.push(LlmStreamChunk::ToolCall(tc));
        }
        out
    }

    fn done_chunk(&self) -> LlmStreamChunk {
        LlmStreamChunk::Done {
            content: if self.full_content.is_empty() { None } else { Some(self.full_content.clone()) },
            tool_calls: self.finished_tool_calls.clone(),
        }
    }
}

#[async_trait]
impl LlmBackend for OpenAiCompatBackend {
    async fn stream_chat(&self, model: &str, messages: &[LlmChatMessage], tools: &[LlmToolDef]) -> AppResult<LlmStream> {
        let body = Self::build_body(model, messages, tools);
        let mut req = self.client.post(self.url()).json(&body);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let resp = req.send().await.map_err(|e| AppError::BusinessRule(format!("LLM request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::BusinessRule(format!("LLM returned {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        let reader = StreamReader::new(byte_stream.map(|r| r.map_err(std::io::Error::other)));
        let lines = tokio_util::io::ReaderStream::new(reader);

        let stream = async_stream::stream! {
            use futures_util::TryStreamExt;
            let mut lines = lines.map_ok(|bytes| {
                String::from_utf8_lossy(&bytes).to_string()
            });
            let mut parser = OpenAiSseParser::new();

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
        let mut req = self.client.get(self.models_url());
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }
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

    fn parse(body: &str) -> Vec<LlmStreamChunk> {
        let mut parser = OpenAiSseParser::new();
        let mut out = parser.push(body);
        if !parser.emitted_done {
            out.extend(parser.finish());
        }
        out
    }

    fn done_tool_calls(chunks: &[LlmStreamChunk]) -> Vec<LlmToolCall> {
        chunks
            .iter()
            .find_map(|c| match c {
                LlmStreamChunk::Done { tool_calls, .. } => Some(tool_calls.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn gemini_stop_finish_reason_emits_tool_calls() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"query","arguments":"{\"sql\":\"SELECT 1\"}"}}]}}]}"#,
            "\n\n",
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let chunks = parse(body);
        let tools = done_tool_calls(&chunks);
        assert_eq!(tools.len(), 1, "expected tool call, got {chunks:?}");
        assert_eq!(tools[0].id, "call_1");
        assert_eq!(tools[0].name, "query");
        assert_eq!(tools[0].arguments["sql"], "SELECT 1");
        assert!(chunks.iter().any(|c| matches!(c, LlmStreamChunk::ToolCall(_))));
    }

    #[test]
    fn gemini_uppercase_stop_emits_tool_calls() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"list_tables","arguments":"{}"}}]},"finish_reason":"STOP"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "list_tables");
    }

    #[test]
    fn openai_tool_calls_finish_reason_still_works() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"query","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "query");
    }

    #[test]
    fn parses_text_deltas() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"content":"Hello "}}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"content":"world"},"finish_reason":"stop"}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let chunks = parse(body);
        let text: String = chunks
            .iter()
            .filter_map(|c| match c {
                LlmStreamChunk::Delta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hello world");
        assert!(done_tool_calls(&chunks).is_empty());
        assert!(chunks.iter().any(|c| matches!(
            c,
            LlmStreamChunk::Done {
                content: Some(s),
                ..
            } if s == "Hello world"
        )));
    }

    #[test]
    fn stop_without_tools_does_not_invent_tool_calls() {
        let body = concat!(r#"data: {"choices":[{"delta":{"content":"ok"},"finish_reason":"stop"}]}"#, "\n", "data: [DONE]\n",);
        assert!(done_tool_calls(&parse(body)).is_empty());
    }

    #[test]
    fn parses_trailing_json_without_newline() {
        let body = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"query","arguments":"{\"sql\":\"SELECT 1\"}"}}]},"finish_reason":"stop"}]}"#;
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1, "leftover buffer must be parsed");
        assert_eq!(tools[0].name, "query");
    }

    #[test]
    fn parses_non_streaming_message_tool_calls() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"query","arguments":"{\"sql\":\"SELECT 1\"}"}}]},"finish_reason":"stop"}]}"#;
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "query");
        assert_eq!(tools[0].arguments["sql"], "SELECT 1");
    }

    #[test]
    fn parses_object_arguments() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"query","arguments":{"sql":"SELECT 1"}}}]},"finish_reason":"stop"}]}"#,
            "\n\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].arguments["sql"], "SELECT 1");
    }

    #[test]
    fn concatenates_streamed_argument_fragments() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"query","arguments":"{\"sql\":"}}]}}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"SELECT 1\"}"}}]},"finish_reason":"stop"}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].arguments["sql"], "SELECT 1");
    }

    #[test]
    fn captures_thought_signature_on_tool_call() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","extra_content":{"google":{"thought_signature":"sig_abc"}},"function":{"name":"list_tables","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "list_tables");
        assert_eq!(tools[0].thought_signature.as_deref(), Some("sig_abc"));
    }

    #[test]
    fn captures_thought_signature_from_delta_extra_content() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"extra_content":{"google":{"thought_signature":"sig_delta"}}}}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"query","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].thought_signature.as_deref(), Some("sig_delta"));
    }

    #[test]
    fn captures_thought_signature_after_tool_call_flush() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"list_tables","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"extra_content":{"google":{"thought_signature":"sig_late"}}}}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let tools = done_tool_calls(&parse(body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].thought_signature.as_deref(), Some("sig_late"));
    }

    #[test]
    fn build_body_echoes_thought_signature() {
        let messages = [LlmChatMessage {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "call_1".into(),
                name: "list_tables".into(),
                arguments: json!({}),
                thought_signature: Some("sig_abc".into()),
            }]),
            tool_call_id: None,
            name: None,
        }];
        let body = OpenAiCompatBackend::build_body("gemini-2.5-flash", &messages, &[]);
        let sig = body["messages"][0]["tool_calls"][0]["extra_content"]["google"]["thought_signature"].as_str();
        assert_eq!(sig, Some("sig_abc"));
    }
}
