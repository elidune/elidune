//! MCP tool dispatch (domain tools + SQL introspection).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::models::user::UserClaims;
use crate::services::audit::{self, AuditLogMeta};
use crate::AppState;

use super::domain;
use super::executor;
use super::schema_memo;

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

fn query_tool_description() -> String {
    format!(
        "Run a single read-only SELECT with explicit columns (never SELECT *). Row-level security applies. \
         Prefer domain tools (list_my_loans, list_my_loan_history, get_biblio, …) when they fit. \
         Call describe_table before querying unfamiliar columns.\n\n{}",
        schema_memo::query_digest()
    )
}

/// JSON Schema snippets advertised in `tools/list`.
pub fn tool_defs() -> Value {
    json!([
        {
            "name": "list_my_loans",
            "description": "List active/current loans for the connected user only (returned_at IS NULL). Use for « mes emprunts en cours ». For past/returned loans use list_my_loan_history. Do NOT use query.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max rows (default 25, max 50)" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "list_my_loan_history",
            "description": "List returned/past loans for the connected user (loans_archives). Use for « mon dernier emprunt », loan history, reading recommendations based on past borrows. Do NOT use query.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max rows (default 10, max 25)" }
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
            "name": "get_biblio",
            "description": "Get a full bibliographic record by id (title, abstract, subject, keywords, authors, …). Use after list_my_loan_history or search_biblios when details are needed. Column abstract holds the summary text (NOT summary).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "biblioId": { "type": "integer", "description": "biblios.id" }
                },
                "required": ["biblioId"],
                "additionalProperties": false
            }
        },
        {
            "name": "schema_overview",
            "description": "Return the Elidune domain schema map, tool-selection policy, and SQL digest. Prefer this over list_tables when unsure which table/column to use.",
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
            "description": "Describe columns (types, comments, foreign keys) for one table or view. Example table_name: loans, loans_archives, biblios, items.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "Table or view name (e.g. loans_archives, biblios)" }
                },
                "required": ["table_name"],
                "additionalProperties": false
            }
        },
        {
            "name": "query",
            "description": query_tool_description(),
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
        "list_my_loan_history" => domain::list_my_loan_history(state, claims, arguments).await,
        "list_my_holds" => domain::list_my_holds(state, claims, arguments).await,
        "search_biblios" => domain::search_biblios(state, arguments).await,
        "get_biblio" => domain::get_biblio(state, arguments).await,
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
        assert!(names.contains(&"list_my_loan_history"));
        assert!(names.contains(&"list_my_holds"));
        assert!(names.contains(&"search_biblios"));
        assert!(names.contains(&"get_biblio"));
        assert!(names.contains(&"schema_overview"));
    }

    #[test]
    fn list_my_loans_description_discourages_query() {
        let defs = tool_defs();
        let loan = defs.as_array().unwrap().iter().find(|t| t["name"] == "list_my_loans").unwrap();
        let desc = loan["description"].as_str().unwrap();
        assert!(desc.contains("Do NOT use query"));
        assert!(desc.contains("list_my_loan_history"));
    }

    #[test]
    fn query_tool_includes_schema_digest_and_abstract_hint() {
        let defs = tool_defs();
        let query = defs.as_array().unwrap().iter().find(|t| t["name"] == "query").unwrap();
        let desc = query["description"].as_str().unwrap();
        assert!(desc.contains("loans_archives"));
        assert!(desc.contains("abstract"));
        assert!(desc.contains("NOT summary") || desc.contains("not summary"));
    }

    #[test]
    fn schema_overview_includes_query_digest() {
        let v = domain::schema_overview_value();
        assert!(v.get("queryDigest").and_then(|d| d.as_str()).unwrap_or("").contains("biblios"));
    }
}
