//! Gemini OpenAI-compat message normalization (strict turn ordering for tool calls).

use std::collections::HashSet;

use super::super::llm::LlmChatMessage;

/// Prepare messages for Gemini `/chat/completions` (no `system` role, complete tool rounds only).
pub fn normalize_gemini_messages(messages: &[LlmChatMessage]) -> Vec<LlmChatMessage> {
    let without_system = fold_system_into_first_user(messages);
    strip_invalid_tool_rounds(without_system)
}

fn fold_system_into_first_user(messages: &[LlmChatMessage]) -> Vec<LlmChatMessage> {
    let mut system_parts: Vec<String> = Vec::new();
    let mut rest: Vec<LlmChatMessage> = Vec::new();

    for m in messages {
        if m.role == "system" {
            if let Some(ref c) = m.content {
                if !c.is_empty() {
                    system_parts.push(c.clone());
                }
            }
        } else {
            rest.push(m.clone());
        }
    }

    if system_parts.is_empty() {
        return rest;
    }

    let system = system_parts.join("\n\n");
    if let Some(first_user) = rest.iter_mut().find(|m| m.role == "user") {
        let user_text = first_user.content.take().unwrap_or_default();
        first_user.content = Some(if user_text.is_empty() {
            system
        } else {
            format!("{system}\n\n{user_text}")
        });
        return rest;
    }

    rest.insert(
        0,
        LlmChatMessage {
            role: "user".into(),
            content: Some(system),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    );
    rest
}

/// Drop assistant tool-call turns without matching tool responses, and orphan tool messages.
fn strip_invalid_tool_rounds(messages: Vec<LlmChatMessage>) -> Vec<LlmChatMessage> {
    let mut out = Vec::with_capacity(messages.len());
    let mut i = 0;
    while i < messages.len() {
        let m = &messages[i];
        if m.role == "assistant" && m.tool_calls.as_ref().is_some_and(|t| !t.is_empty()) {
            let tool_calls = m.tool_calls.as_ref().unwrap();
            let expected: HashSet<String> = tool_calls.iter().map(|tc| tc.id.clone()).collect();
            let mut j = i + 1;
            let mut seen: HashSet<String> = HashSet::new();
            while j < messages.len() && messages[j].role == "tool" {
                if let Some(id) = messages[j].tool_call_id.as_ref() {
                    seen.insert(id.clone());
                }
                j += 1;
            }
            if seen != expected {
                i = j;
                continue;
            }
            out.push(messages[i].clone());
            for msg in &messages[i + 1..j] {
                out.push(msg.clone());
            }
            i = j;
            continue;
        }
        if m.role == "tool" {
            i += 1;
            continue;
        }
        out.push(m.clone());
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chat::llm::LlmToolCall;
    use serde_json::json;

    fn user(content: &str) -> LlmChatMessage {
        LlmChatMessage {
            role: "user".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    fn system(content: &str) -> LlmChatMessage {
        LlmChatMessage {
            role: "system".into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    fn assistant_tools(calls: Vec<LlmToolCall>) -> LlmChatMessage {
        LlmChatMessage {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(calls),
            tool_call_id: None,
            name: None,
        }
    }

    fn tool(id: &str, name: &str) -> LlmChatMessage {
        LlmChatMessage {
            role: "tool".into(),
            content: Some("{}".into()),
            tool_calls: None,
            tool_call_id: Some(id.into()),
            name: Some(name.into()),
        }
    }

    #[test]
    fn folds_system_into_first_user() {
        let msgs = vec![system("SYS"), user("hello")];
        let out = normalize_gemini_messages(&msgs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, "user");
        assert!(out[0].content.as_ref().unwrap().contains("SYS"));
        assert!(out[0].content.as_ref().unwrap().contains("hello"));
    }

    #[test]
    fn drops_assistant_tool_calls_without_responses_before_next_user() {
        let msgs = vec![
            user("q1"),
            assistant_tools(vec![LlmToolCall {
                id: "c1".into(),
                name: "query".into(),
                arguments: json!({}),
                thought_signature: None,
            }]),
            user("q2"),
        ];
        let out = normalize_gemini_messages(&msgs);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].role, "user");
        assert_eq!(out[0].content.as_deref(), Some("q1"));
        assert_eq!(out[1].role, "user");
        assert_eq!(out[1].content.as_deref(), Some("q2"));
    }

    #[test]
    fn keeps_complete_tool_round() {
        let msgs = vec![
            user("q1"),
            assistant_tools(vec![LlmToolCall {
                id: "c1".into(),
                name: "query".into(),
                arguments: json!({}),
                thought_signature: None,
            }]),
            tool("c1", "query"),
            user("q2"),
        ];
        let out = normalize_gemini_messages(&msgs);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1].role, "assistant");
        assert_eq!(out[2].role, "tool");
    }
}
