//! Agent loop: LLM + in-process MCP tools.

use std::sync::Arc;

use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::llm::{LlmBackend, LlmChatMessage, LlmStreamChunk, LlmToolCall, LlmToolDef};
use super::build_backend;
use crate::{
    config::ChatConfig,
    error::{AppError, AppResult},
    mcp::{
        schema_memo,
        tools::{self, ToolCallOptions},
    },
    models::chat::ChatMessage,
    models::user::UserClaims,
    repository::{ChatRepository, LlmProvidersRepository},
    services::audit::{self, AuditService},
    AppState,
};

const USER_AUDIT_MAX: usize = 500;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentSseEvent {
    #[serde(rename = "delta")]
    Delta { text: String },
    #[serde(rename = "tool.start")]
    ToolStart { id: String, name: String },
    #[serde(rename = "tool.result")]
    ToolResult {
        id: String,
        name: String,
        ok: bool,
        summary: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "done")]
    Done { assistant_message_id: String, conversation_id: String, title: String },
}

pub fn mcp_tool_defs() -> Vec<LlmToolDef> {
    let defs = tools::tool_defs();
    let arr = defs.as_array().cloned().unwrap_or_default();
    arr.into_iter()
        .filter_map(|t| {
            Some(LlmToolDef {
                name: t.get("name")?.as_str()?.to_string(),
                description: t.get("description")?.as_str()?.to_string(),
                parameters: t.get("inputSchema")?.clone(),
            })
        })
        .collect()
}

fn build_system_prompt(library_name: &str, claims: &UserClaims) -> String {
    let role = claims.account_type.as_str();
    format!(
        "You are the conversational assistant for library {library_name}. \
         The signed-in user has role \"{role}\" (user_id={}). \
         Always reply in the same language as the user's latest message. \
         Be concise and helpful.\n\n\
         {overview}\n\n\
         {tool_policy}\n\n\
         Never invent availability or status: use the tools. \
         Permissions and RLS already filter data (a patron only sees their own loans and holds). \
         Never ask for passwords or secrets. \
         If you cannot answer with the tools, say so clearly.",
        claims.user_id,
        overview = schema_memo::overview(),
        tool_policy = schema_memo::tool_policy(),
    )
}

fn history_to_llm(messages: &[ChatMessage], tool_result_max: usize) -> Vec<LlmChatMessage> {
    use std::collections::HashMap;

    let mut tool_names_by_id: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let m = &messages[i];
        match m.role.as_str() {
            "user" => {
                out.push(LlmChatMessage {
                    role: "user".into(),
                    content: m.content.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
            "assistant" => {
                if let Some(args) = m.tool_arguments.as_ref().and_then(|v| v.as_array()) {
                    let tool_calls: Vec<LlmToolCall> = args
                        .iter()
                        .filter_map(|tc| {
                            Some(LlmToolCall {
                                id: tc.get("id")?.as_str()?.to_string(),
                                name: tc.get("name")?.as_str()?.to_string(),
                                arguments: tc.get("arguments").cloned().unwrap_or(json!({})),
                                thought_signature: tc
                                    .get("thoughtSignature")
                                    .or_else(|| tc.get("thought_signature"))
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string),
                            })
                        })
                        .collect();
                    for tc in &tool_calls {
                        if !tc.id.is_empty() && !tc.name.is_empty() {
                            tool_names_by_id.insert(tc.id.clone(), tc.name.clone());
                        }
                    }
                    out.push(LlmChatMessage {
                        role: "assistant".into(),
                        content: m.content.clone(),
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        tool_call_id: None,
                        name: None,
                    });
                } else {
                    out.push(LlmChatMessage {
                        role: "assistant".into(),
                        content: m.content.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });
                }
            }
            "tool" => {
                let content = m.tool_result.as_ref().map(|v| truncate_json(v, tool_result_max)).or_else(|| m.content.clone());
                let name = m
                    .tool_name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .or_else(|| m.tool_call_id.as_ref().and_then(|id| tool_names_by_id.get(id).cloned()));
                out.push(LlmChatMessage {
                    role: "tool".into(),
                    content,
                    tool_calls: None,
                    tool_call_id: m.tool_call_id.clone(),
                    name,
                });
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn truncate_json(v: &Value, max: usize) -> String {
    let s = v.to_string();
    if s.len() <= max {
        s
    } else {
        format!("{}…", &s[..max])
    }
}

fn tool_summary(name: &str, result: &Result<Value, AppError>) -> (bool, String) {
    match result {
        Ok(v) => {
            if name == "query" {
                let count = v.get("rowCount").and_then(|c| c.as_u64()).unwrap_or(0);
                (true, format!("{count} rows"))
            } else if name == "list_tables" {
                let n = v.get("tables").and_then(|t| t.as_array()).map(|a| a.len()).unwrap_or(0);
                (true, format!("{n} tables"))
            } else if name == "list_my_loans" {
                let n = v.get("returned").and_then(|c| c.as_u64()).unwrap_or(0);
                (true, format!("{n} loans"))
            } else if name == "list_my_loan_history" {
                let n = v.get("returned").and_then(|c| c.as_u64()).unwrap_or(0);
                (true, format!("{n} past loans"))
            } else if name == "list_my_holds" {
                let n = v.get("holds").and_then(|h| h.as_array()).map(|a| a.len()).unwrap_or(0);
                (true, format!("{n} holds"))
            } else if name == "search_biblios" {
                let n = v.get("returned").and_then(|c| c.as_u64()).unwrap_or(0);
                (true, format!("{n} hits"))
            } else if name == "get_biblio" {
                (true, "biblio record".into())
            } else if name == "describe_table" {
                let n = v.get("columns").and_then(|c| c.as_array()).map(|a| a.len()).unwrap_or(0);
                (true, format!("{n} columns"))
            } else {
                (true, "ok".into())
            }
        }
        Err(e) => (false, e.to_string()),
    }
}

pub async fn run_agent_turn(
    state: &AppState,
    chat_repo: Arc<dyn ChatRepository>,
    llm_repo: Arc<dyn LlmProvidersRepository>,
    audit_svc: AuditService,
    chat_cfg: &ChatConfig,
    claims: &UserClaims,
    conversation_id: i64,
    user_id: i64,
    user_content: &str,
    provider_id: i64,
    model: &str,
    library_name: &str,
    show_tool_detail: bool,
    cancel: CancellationToken,
) -> AppResult<impl futures_core::Stream<Item = AgentSseEvent> + Send> {
    if !chat_cfg.enabled {
        return Err(AppError::BusinessRule("Chat is disabled".into()));
    }
    if state.mcp_pool.is_none() {
        return Err(AppError::Internal("MCP database pool is not configured".into()));
    }

    audit_svc.log(
        audit::event::CHAT_MESSAGE,
        Some(claims.user_id),
        None,
        None,
        None,
        Some(json!({
            "conversationId": conversation_id.to_string(),
            "content": truncate_audit(user_content),
        })),
        audit::AuditLogMeta::success(),
    );

    chat_repo.chat_insert_message(conversation_id, "user", Some(user_content), None, None, None, None).await?;

    let provider_row = llm_repo.llm_providers_get(provider_id).await?;
    let backend = build_backend(&provider_row, chat_cfg.request_timeout_secs)?;

    let mut history = chat_repo.chat_list_messages(conversation_id).await?;
    let max = chat_cfg.max_history_messages as usize;
    if history.len() > max {
        let mut start = history.len().saturating_sub(max);
        while start < history.len() && history[start].role != "user" {
            start += 1;
        }
        history = history.split_off(start);
    }

    let mut llm_messages = vec![LlmChatMessage {
        role: "system".into(),
        content: Some(build_system_prompt(library_name, claims)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];
    llm_messages.extend(history_to_llm(&history, chat_cfg.tool_result_max_chars));

    let tools = mcp_tool_defs();
    let max_rounds = chat_cfg.max_tool_rounds;
    let tool_result_max = chat_cfg.tool_result_max_chars;
    let query_max_rows = chat_cfg.query_max_rows;
    let tool_options = ToolCallOptions { query_max_rows: Some(query_max_rows) };
    let state = state.clone();
    let claims = claims.clone();
    let model = model.to_string();

    let stream = async_stream::stream! {
        let mut round = 0u32;
        loop {
            if cancel.is_cancelled() {
                yield AgentSseEvent::Error { message: "Cancelled".into() };
                return;
            }
            if round >= max_rounds {
                yield AgentSseEvent::Error { message: "Too many tool rounds".into() };
                return;
            }
            round += 1;

            let mut llm_stream = match backend.stream_chat(&model, &llm_messages, &tools).await {
                Ok(s) => s,
                Err(e) => {
                    yield AgentSseEvent::Error { message: e.to_string() };
                    return;
                }
            };

            let mut deltas = String::new();
            let mut tool_calls: Vec<LlmToolCall> = Vec::new();
            let mut done_content: Option<String> = None;

            while let Some(chunk) = llm_stream.next().await {
                if cancel.is_cancelled() {
                    yield AgentSseEvent::Error { message: "Cancelled".into() };
                    return;
                }
                match chunk {
                    Ok(LlmStreamChunk::Delta(t)) => {
                        deltas.push_str(&t);
                        yield AgentSseEvent::Delta { text: t };
                    }
                    Ok(LlmStreamChunk::ToolCall(tc)) => {
                        tool_calls.push(tc);
                    }
                    Ok(LlmStreamChunk::Done { content, tool_calls: tcs }) => {
                        done_content = content;
                        if !tcs.is_empty() {
                            tool_calls = tcs;
                        }
                        break;
                    }
                    Err(e) => {
                        yield AgentSseEvent::Error { message: e.to_string() };
                        return;
                    }
                }
            }

            let assistant_text = done_content.unwrap_or_else(|| deltas.clone());

            if !tool_calls.is_empty() {
                let tool_calls_json = json!(tool_calls.iter().map(|tc| {
                    let mut obj = json!({
                        "id": tc.id,
                        "name": tc.name,
                        "arguments": tc.arguments
                    });
                    if let Some(ref sig) = tc.thought_signature {
                        obj["thoughtSignature"] = json!(sig);
                    }
                    obj
                }).collect::<Vec<_>>());

                let _ = chat_repo
                    .chat_insert_message(
                        conversation_id,
                        "assistant",
                        if assistant_text.is_empty() { None } else { Some(&assistant_text) },
                        None,
                        None,
                        Some(tool_calls_json),
                        None,
                    )
                    .await;

                llm_messages.push(LlmChatMessage {
                    role: "assistant".into(),
                    content: if assistant_text.is_empty() { None } else { Some(assistant_text) },
                    tool_calls: Some(tool_calls.clone()),
                    tool_call_id: None,
                    name: None,
                });

                for tc in tool_calls {
                    if tc.name.is_empty() {
                        yield AgentSseEvent::Error { message: "LLM emitted a tool call without a name".into() };
                        return;
                    }
                    yield AgentSseEvent::ToolStart { id: tc.id.clone(), name: tc.name.clone() };

                    let mcp_result = tools::call_tool(&state, &claims, &tc.name, tc.arguments.clone(), tool_options).await;
                    let (ok, summary) = tool_summary(&tc.name, &mcp_result);
                    let result_value = mcp_result.as_ref().ok().cloned();
                    let detail = if show_tool_detail && tc.name == "query" {
                        tc.arguments.get("sql").and_then(|v| v.as_str()).map(str::to_string)
                    } else {
                        None
                    };

                    yield AgentSseEvent::ToolResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        ok,
                        summary: summary.clone(),
                        detail,
                    };

                    let result_str = result_value
                        .as_ref()
                        .map(|v| truncate_json(v, tool_result_max))
                        .unwrap_or_else(|| summary.clone());

                    let _ = chat_repo
                        .chat_insert_message(
                            conversation_id,
                            "tool",
                            Some(&result_str),
                            Some(&tc.name),
                            Some(&tc.id),
                            Some(tc.arguments.clone()),
                            result_value,
                        )
                        .await;

                    llm_messages.push(LlmChatMessage {
                        role: "tool".into(),
                        content: Some(result_str),
                        tool_calls: None,
                        tool_call_id: Some(tc.id),
                        name: Some(tc.name),
                    });
                }
                continue;
            }

            let msg = chat_repo
                .chat_insert_message(
                    conversation_id,
                    "assistant",
                    Some(&assistant_text),
                    None,
                    None,
                    None,
                    None,
                )
                .await;

            let title = if assistant_text.len() > 60 {
                format!("{}…", &assistant_text[..60])
            } else if assistant_text.is_empty() {
                "Conversation".into()
            } else {
                assistant_text.clone()
            };

            let _ = chat_repo
                .chat_update_conversation_title(conversation_id, user_id, &title)
                .await;
            let _ = chat_repo.chat_touch_conversation(conversation_id, user_id).await;

            match msg {
                Ok(m) => {
                    yield AgentSseEvent::Done {
                        assistant_message_id: m.id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        title,
                    };
                }
                Err(e) => {
                    yield AgentSseEvent::Error { message: e.to_string() };
                }
            }
            return;
        }
    };

    Ok(stream)
}

fn truncate_audit(s: &str) -> String {
    if s.len() <= USER_AUDIT_MAX {
        s.to_string()
    } else {
        format!("{}…", &s[..USER_AUDIT_MAX])
    }
}

pub fn models_from_json(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::{AccountTypeSlug, UserRights};

    fn reader_claims() -> UserClaims {
        UserClaims {
            sub: "reader".into(),
            user_id: 42,
            account_type: AccountTypeSlug::Reader,
            rights: UserRights::default(),
            exp: 0,
            iat: 0,
            token_version: 0,
            scope: None,
        }
    }

    #[test]
    fn system_prompt_prefers_domain_tools() {
        let prompt = build_system_prompt("TestLib", &reader_claims());
        assert!(prompt.contains("list_my_loans"));
        assert!(prompt.contains("list_my_loan_history"));
        assert!(prompt.contains("list_my_holds"));
        assert!(prompt.contains("search_biblios"));
        assert!(prompt.contains("get_biblio"));
        assert!(prompt.contains("abstract"));
        assert!(prompt.contains("loans"));
        assert!(prompt.contains("same language as the user's latest message"));
        assert!(prompt.to_lowercase().contains("starting with list_tables"));
        assert!(prompt.to_lowercase().contains("not summary"));
    }

    #[test]
    fn mcp_tool_defs_include_domain_tools() {
        let names: Vec<String> = mcp_tool_defs().into_iter().map(|t| t.name).collect();
        assert!(names.iter().any(|n| n == "list_my_loans"));
        assert!(names.iter().any(|n| n == "list_my_loan_history"));
        assert!(names.iter().any(|n| n == "get_biblio"));
    }

    #[test]
    fn history_to_llm_backfills_tool_name_from_assistant_tool_calls() {
        use chrono::Utc;
        use crate::models::chat::ChatMessage;

        let messages = vec![
            ChatMessage {
                id: 1,
                conversation_id: 1,
                role: "assistant".into(),
                content: None,
                tool_name: None,
                tool_call_id: None,
                tool_arguments: Some(json!([{
                    "id": "call_1",
                    "name": "list_my_loan_history",
                    "arguments": {}
                }])),
                tool_result: None,
                created_at: Utc::now(),
            },
            ChatMessage {
                id: 2,
                conversation_id: 1,
                role: "tool".into(),
                content: Some("{}".into()),
                tool_name: None,
                tool_call_id: Some("call_1".into()),
                tool_arguments: None,
                tool_result: Some(json!({ "loans": [] })),
                created_at: Utc::now(),
            },
        ];

        let llm = history_to_llm(&messages, 4000);
        assert_eq!(llm.len(), 2);
        assert_eq!(llm[1].role, "tool");
        assert_eq!(llm[1].name.as_deref(), Some("list_my_loan_history"));
    }
}
