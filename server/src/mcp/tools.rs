//! MCP tool dispatch (domain tools + SQL introspection).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::models::user::UserClaims;
use crate::services::audit::{self, AuditLogMeta};
use crate::AppState;

use super::domain;
use super::executor;

const SQL_AUDIT_MAX: usize = 500;

/// Per-call options (chat agent passes stricter limits than raw MCP HTTP).
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolCallOptions {
    /// Override row cap for `query` (e.g. chat uses a lower limit than MCP default).
    pub query_max_rows: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct QueryArgs {
    pub sql: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeArgs {
    pub table_name: String,
}

/// JSON Schema snippets advertised in `tools/list`.
pub fn tool_defs() -> Value {
    json!([
        {
            "name": "list_my_loans",
            "description": "List active loans (emprunts en cours) for the connected user only. Use for « mes emprunts », « quels livres ai-je empruntés ». Do NOT use query or list_tables for this.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max rows (default 25, max 50)" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "list_my_holds",
            "description": "List holds/reservations for the connected user. Use for « mes réservations ». Do NOT use query for this.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "activeOnly": { "type": "boolean", "description": "When true (default), only pending/ready holds" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "search_biblios",
            "description": "Search the library catalog by free text (title, author, ISBN, keywords). Use for « cherche un livre sur … ».",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": { "type": "string", "description": "Search terms" },
                    "limit": { "type": "integer", "description": "Max results (default 15, max 25)" }
                },
                "required": ["q"],
                "additionalProperties": false
            }
        },
        {
            "name": "schema_overview",
            "description": "Return the Elidune domain schema map and tool-selection policy. Prefer this over list_tables when unsure which table to use.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "list_tables",
            "description": "List database tables/views the current account can SELECT. Use only when schema_overview and describe_table are insufficient.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "describe_table",
            "description": "Describe columns (types, comments, foreign keys) for one table or view. Example table_name: loans, holds, biblios, items.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "Table or view name (e.g. loans, biblios, users)" }
                },
                "required": ["table_name"],
                "additionalProperties": false
            }
        },
        {
            "name": "query",
            "description": "Run a single read-only SELECT with explicit columns (never SELECT *). Row-level security applies. Prefer domain tools (list_my_loans, etc.) when they fit.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "A single SELECT / WITH / EXPLAIN SELECT with named columns and LIMIT" }
                },
                "required": ["sql"],
                "additionalProperties": false
            }
        }
    ])
}

pub async fn call_tool(state: &AppState, claims: &UserClaims, name: &str, arguments: Value, options: ToolCallOptions) -> AppResult<Value> {
    match name {
        "list_my_loans" => domain::list_my_loans(state, claims, arguments).await,
        "list_my_holds" => domain::list_my_holds(state, claims, arguments).await,
        "search_biblios" => domain::search_biblios(state, arguments).await,
        "schema_overview" => Ok(domain::schema_overview_value()),
        "list_tables" => executor::list_tables(state, claims).await,
        "describe_table" => {
            let args: DescribeArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid describe_table arguments: {e}")))?;
            executor::describe_table(state, claims, &args.table_name).await
        }
        "query" => {
            let args: QueryArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid query arguments: {e}")))?;
            let result = executor::execute_query(state, claims, &args.sql, options.query_max_rows).await;
            audit_query(state, claims, &args.sql, &result);
            let result = result?;
            Ok(json!({
                "rows": result.rows,
                "rowCount": result.rows.len(),
                "truncated": result.truncated,
            }))
        }
        other => Err(AppError::NotFound(format!("Unknown MCP tool: {other}"))),
    }
}

fn audit_query(state: &AppState, claims: &UserClaims, sql: &str, result: &AppResult<executor::QueryResult>) {
    let mut snippet = sql.trim().replace('\n', " ");
    if snippet.len() > SQL_AUDIT_MAX {
        snippet.truncate(SQL_AUDIT_MAX);
        snippet.push('…');
    }
    let (payload, meta) = match result {
        Ok(rows) => (
            json!({
                "sql": snippet,
                "rowCount": rows.rows.len(),
                "truncated": rows.truncated,
                "role": super::roles::pg_role(&claims.account_type),
            }),
            AuditLogMeta::success(),
        ),
        Err(e) => (
            json!({
                "sql": snippet,
                "role": super::roles::pg_role(&claims.account_type),
            }),
            AuditLogMeta::from_app_error(e),
        ),
    };
    state.services.audit.log(audit::event::MCP_QUERY, Some(claims.user_id), Some("mcp"), None, None, Some(payload), meta);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_defs_include_domain_tools() {
        let defs = tool_defs();
        let names: Vec<&str> = defs.as_array().unwrap().iter().filter_map(|t| t.get("name").and_then(|n| n.as_str())).collect();
        assert!(names.contains(&"list_my_loans"));
        assert!(names.contains(&"list_my_holds"));
        assert!(names.contains(&"search_biblios"));
        assert!(names.contains(&"schema_overview"));
    }

    #[test]
    fn list_my_loans_description_discourages_query() {
        let defs = tool_defs();
        let loan = defs.as_array().unwrap().iter().find(|t| t["name"] == "list_my_loans").unwrap();
        let desc = loan["description"].as_str().unwrap();
        assert!(desc.contains("Do NOT use query"));
    }
}
