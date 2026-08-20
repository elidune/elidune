//! MCP tool dispatch (list_tables, describe_table, query).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::models::user::UserClaims;
use crate::services::audit::{self, AuditLogMeta};
use crate::AppState;

use super::executor;

const SQL_AUDIT_MAX: usize = 500;

#[derive(Debug, Deserialize)]
pub struct QueryArgs {
    pub sql: String,
}

#[derive(Debug, Deserialize)]
pub struct DescribeArgs {
    pub table_name: String,
}

/// JSON Schema snippets advertised in `tools/list`.
pub fn tool_defs() -> Value {
    json!([
        {
            "name": "list_tables",
            "description": "List database tables and views the current Elidune account can SELECT via MCP (respects admin/staff/patron/guest grants).",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        },
        {
            "name": "describe_table",
            "description": "Describe columns the current account can read on a table or view. Use table_name without schema; mcp views (users, settings, z3950servers) shadow public tables.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "Table or view name (e.g. users, biblios, loans)" }
                },
                "required": ["table_name"],
                "additionalProperties": false
            }
        },
        {
            "name": "query",
            "description": "Run a single read-only SELECT (or EXPLAIN SELECT) against the Elidune database. Row-level security applies: patrons only see their own users/loans/holds rows; secret columns (passwords, TOTP) are never visible. Results are capped.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "A single SELECT / WITH / EXPLAIN SELECT statement" }
                },
                "required": ["sql"],
                "additionalProperties": false
            }
        }
    ])
}

pub async fn call_tool(state: &AppState, claims: &UserClaims, name: &str, arguments: Value) -> AppResult<Value> {
    match name {
        "list_tables" => executor::list_tables(state, claims).await,
        "describe_table" => {
            let args: DescribeArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid describe_table arguments: {e}")))?;
            executor::describe_table(state, claims, &args.table_name).await
        }
        "query" => {
            let args: QueryArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid query arguments: {e}")))?;
            let result = executor::execute_query(state, claims, &args.sql).await;
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
