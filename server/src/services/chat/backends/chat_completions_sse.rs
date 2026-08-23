//! Chat-completions SSE parser (shared by providers using the `/chat/completions` protocol).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use super::super::llm::{LlmStreamChunk, LlmToolCall};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatCompletionsSseMode {
    /// Standard OpenAI / Ollama / Groq behaviour.
    Standard,
    /// Gemini OpenAI-compat: `thought_signature` + flush tools on any `finish_reason`.
    Gemini,
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

fn is_known_tool_name(name: &str) -> bool {
    matches!(
        name,
        "list_my_loans"
            | "list_my_loan_history"
            | "list_my_holds"
            | "search_biblios"
            | "get_biblio"
            | "schema_overview"
            | "list_tables"
            | "describe_table"
            | "query"
    )
}

fn thought_signature_from_extra(extra: Option<&Value>) -> Option<String> {
    extra?
        .get("google")?
        .get("thought_signature")?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
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

pub struct ChatCompletionsSseParser {
    mode: ChatCompletionsSseMode,
    buffer: String,
    pending_tools: BTreeMap<usize, PendingTool>,
    pending_thought_signature: Option<String>,
    full_content: String,
    finished_tool_calls: Vec<LlmToolCall>,
    emitted_done: bool,
}

impl ChatCompletionsSseParser {
    pub fn new(mode: ChatCompletionsSseMode) -> Self {
        Self {
            mode,
            buffer: String::new(),
            pending_tools: BTreeMap::new(),
            pending_thought_signature: None,
            full_content: String::new(),
            finished_tool_calls: Vec::new(),
            emitted_done: false,
        }
    }

    pub fn push(&mut self, chunk: &str) -> Vec<LlmStreamChunk> {
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

    pub fn finish(&mut self) -> Vec<LlmStreamChunk> {
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
                self.ingest_tool_deltas(tcs, out);
            }
            self.apply_thought_signature(delta.extra_content.as_ref());
        }
        if let Some(message) = choice.message {
            self.ingest_content(message.content, out);
            if let Some(tcs) = message.tool_calls {
                self.ingest_tool_deltas(tcs, out);
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

    fn ingest_tool_deltas(&mut self, tcs: Vec<StreamToolCallDelta>, out: &mut Vec<LlmStreamChunk>) {
        for (fallback_idx, tc) in tcs.into_iter().enumerate() {
            let idx = tc.index.unwrap_or(fallback_idx);

            let should_flush_for_id = tc.id.as_ref().is_some_and(|id| {
                !id.is_empty()
                    && self.pending_tools.get(&idx).is_some_and(|entry| {
                        !entry.id.is_empty() && entry.id != *id && !entry.name.is_empty()
                    })
            });
            if should_flush_for_id {
                if let Some(chunk) = self.flush_tool_at(idx) {
                    out.push(chunk);
                }
            }

            let new_name = tc.function.as_ref().and_then(|f| f.name.as_deref());
            let should_flush_for_name = new_name.is_some_and(|n| {
                self.pending_tools
                    .get(&idx)
                    .is_some_and(|entry| is_known_tool_name(&entry.name) && is_known_tool_name(n) && entry.name != n)
            });
            if should_flush_for_name {
                if let Some(chunk) = self.flush_tool_at(idx) {
                    out.push(chunk);
                }
            }

            let entry = self.pending_tools.entry(idx).or_default();
            if let Some(id) = tc.id {
                if !id.is_empty() {
                    entry.id = id;
                }
            }
            if entry.thought_signature.is_empty() && self.mode == ChatCompletionsSseMode::Gemini {
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
                if n.starts_with('_') || entry.name.is_empty() {
                    entry.name.push_str(&n);
                } else if is_known_tool_name(&entry.name) && is_known_tool_name(&n) && entry.name != n {
                    entry.name = n;
                } else {
                    entry.name.push_str(&n);
                }
            }
            if let Some(a) = f.arguments {
                if entry.arguments.is_empty() {
                    entry.arguments = a;
                } else if a.starts_with('{') || a.starts_with('[') {
                    // Some providers restart argument JSON on the same index.
                    entry.arguments = a;
                } else {
                    entry.arguments.push_str(&a);
                }
            }
        }
    }

    fn flush_tool_at(&mut self, idx: usize) -> Option<LlmStreamChunk> {
        let tool = self.pending_tools.remove(&idx)?;
        if tool.name.is_empty() {
            return None;
        }
        Some(self.tool_call_chunk(idx, tool))
    }

    fn tool_call_chunk(&mut self, idx: usize, tool: PendingTool) -> LlmStreamChunk {
        let id = if tool.id.is_empty() {
            format!("call_{idx}")
        } else {
            tool.id
        };
        let arguments: Value = serde_json::from_str(&tool.arguments).unwrap_or_else(|_| {
            if tool.arguments.is_empty() {
                json!({})
            } else {
                json!({ "raw": tool.arguments })
            }
        });
        let tc = LlmToolCall {
            id,
            name: tool.name,
            arguments,
            thought_signature: if tool.thought_signature.is_empty() {
                None
            } else {
                Some(tool.thought_signature)
            },
        };
        self.finished_tool_calls.push(tc.clone());
        LlmStreamChunk::ToolCall(tc)
    }

    fn apply_thought_signature(&mut self, extra: Option<&Value>) {
        if self.mode != ChatCompletionsSseMode::Gemini {
            return;
        }
        let Some(sig) = thought_signature_from_extra(extra) else {
            return;
        };
        if let Some(tool) = self
            .pending_tools
            .values_mut()
            .find(|t| t.thought_signature.is_empty())
        {
            tool.thought_signature = sig;
            return;
        }
        if let Some(tc) = self
            .finished_tool_calls
            .iter_mut()
            .find(|t| t.thought_signature.is_none())
        {
            tc.thought_signature = Some(sig);
            return;
        }
        self.pending_thought_signature = Some(sig);
    }

    fn flush_pending_tools(&mut self) -> Vec<LlmStreamChunk> {
        let pending = std::mem::take(&mut self.pending_tools);
        pending
            .into_iter()
            .filter_map(|(idx, tool)| {
                if tool.name.is_empty() {
                    None
                } else {
                    Some(self.tool_call_chunk(idx, tool))
                }
            })
            .collect()
    }

    fn done_chunk(&self) -> LlmStreamChunk {
        LlmStreamChunk::Done {
            content: if self.full_content.is_empty() {
                None
            } else {
                Some(self.full_content.clone())
            },
            tool_calls: self.finished_tool_calls.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(mode: ChatCompletionsSseMode, body: &str) -> Vec<LlmStreamChunk> {
        let mut parser = ChatCompletionsSseParser::new(mode);
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

    fn all_tool_calls(chunks: &[LlmStreamChunk]) -> Vec<LlmToolCall> {
        let mut calls: Vec<LlmToolCall> = chunks
            .iter()
            .filter_map(|c| match c {
                LlmStreamChunk::ToolCall(tc) => Some(tc.clone()),
                _ => None,
            })
            .collect();
        if calls.is_empty() {
            calls = done_tool_calls(chunks);
        }
        calls
    }

    #[test]
    fn gemini_same_index_second_tool_does_not_concatenate_names() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"list_my_loans","arguments":"{}"}}]}}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"query","arguments":"{\"sql\":\"SELECT 1 FROM loans\"}"}}]},"finish_reason":"stop"}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let chunks = parse(ChatCompletionsSseMode::Gemini, body);
        let tools = all_tool_calls(&chunks);
        assert_eq!(tools.len(), 2, "expected two tool calls, got {tools:?}");
        assert_eq!(tools[0].name, "list_my_loans");
        assert_eq!(tools[1].name, "query");
        assert!(!tools.iter().any(|t| t.name.contains("list_my_loansquery")));
    }

    #[test]
    fn streamed_tool_name_fragments_still_concatenate() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"list_my_"}}]}}]}"#,
            "\n",
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"loans","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n",
            "data: [DONE]\n",
        );
        let tools = all_tool_calls(&parse(ChatCompletionsSseMode::Gemini, body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "list_my_loans");
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
        let chunks = parse(ChatCompletionsSseMode::Gemini, body);
        let tools = done_tool_calls(&chunks);
        assert_eq!(tools.len(), 1, "expected tool call, got {chunks:?}");
        assert_eq!(tools[0].id, "call_1");
        assert_eq!(tools[0].name, "query");
        assert_eq!(tools[0].arguments["sql"], "SELECT 1");
        assert!(chunks.iter().any(|c| matches!(c, LlmStreamChunk::ToolCall(_))));
    }

    #[test]
    fn openai_tool_calls_finish_reason_still_works() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"query","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(ChatCompletionsSseMode::Standard, body));
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
        let chunks = parse(ChatCompletionsSseMode::Standard, body);
        let text: String = chunks
            .iter()
            .filter_map(|c| match c {
                LlmStreamChunk::Delta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hello world");
        assert!(done_tool_calls(&chunks).is_empty());
    }

    #[test]
    fn captures_thought_signature_on_tool_call() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","extra_content":{"google":{"thought_signature":"sig_abc"}},"function":{"name":"list_tables","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(ChatCompletionsSseMode::Gemini, body));
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].thought_signature.as_deref(), Some("sig_abc"));
    }

    #[test]
    fn standard_mode_ignores_thought_signature() {
        let body = concat!(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","extra_content":{"google":{"thought_signature":"sig_abc"}},"function":{"name":"query","arguments":"{}"}}]},"finish_reason":"stop"}]}"#,
            "\n\n",
            "data: [DONE]\n\n",
        );
        let tools = done_tool_calls(&parse(ChatCompletionsSseMode::Standard, body));
        assert_eq!(tools.len(), 1);
        assert!(tools[0].thought_signature.is_none());
    }
}
