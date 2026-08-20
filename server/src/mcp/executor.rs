//! Read-only SQL execution under a per-request PostgreSQL MCP role.

use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::error::{AppError, AppResult};
use crate::models::user::UserClaims;
use crate::AppState;

use super::roles::pg_role;
use super::sql_guard::{guard_sql, GuardedSql};

/// Run a guarded query as `claims` and return JSON rows plus truncation metadata.
pub async fn execute_query(state: &AppState, claims: &UserClaims, sql: &str) -> AppResult<QueryResult> {
    let pool = state.mcp_pool.as_ref().ok_or_else(|| AppError::Internal("MCP database pool is not configured".into()))?;

    let max_rows = state.config.mcp.max_rows;
    let timeout_ms = state.config.mcp.statement_timeout_ms;
    let guarded = guard_sql(sql, max_rows)?;
    let role = pg_role(&claims.account_type);

    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL transaction_read_only = on").execute(&mut *tx).await?;
    sqlx::query(&format!("SET LOCAL statement_timeout = '{timeout_ms}ms'")).execute(&mut *tx).await?;
    sqlx::query("SET LOCAL search_path = mcp, public").execute(&mut *tx).await?;
    let set_role_sql = match role {
        super::roles::ROLE_ADMIN => "SET LOCAL ROLE elidune_mcp_admin",
        super::roles::ROLE_STAFF => "SET LOCAL ROLE elidune_mcp_staff",
        super::roles::ROLE_PATRON => "SET LOCAL ROLE elidune_mcp_patron",
        super::roles::ROLE_CATALOG => "SET LOCAL ROLE elidune_mcp_catalog",
        _ => {
            return Err(AppError::Internal("Unknown MCP role".into()));
        }
    };
    sqlx::query(set_role_sql).execute(&mut *tx).await?;
    sqlx::query("SELECT set_config('elidune.current_user_id', $1, true)")
        .bind(claims.user_id.to_string())
        .execute(&mut *tx)
        .await?;

    let result = match &guarded {
        GuardedSql::Select { sql: wrapped } => fetch_json_rows(&mut tx, wrapped, max_rows).await,
        GuardedSql::Explain { sql: explain_sql } => fetch_explain(&mut tx, explain_sql).await,
    };

    match result {
        Ok(rows) => {
            tx.commit().await?;
            Ok(rows)
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}

/// Introspect tables the current MCP role can SELECT (including `mcp` views).
pub async fn list_tables(state: &AppState, claims: &UserClaims) -> AppResult<Value> {
    let sql = r#"
        SELECT DISTINCT table_schema, table_name
        FROM information_schema.table_privileges
        WHERE grantee = current_user
          AND privilege_type = 'SELECT'
          AND table_schema IN ('mcp', 'public')
        ORDER BY table_schema, table_name
    "#;
    let result = execute_query(state, claims, sql).await?;
    Ok(json!({
        "tables": result.rows,
        "truncated": result.truncated,
    }))
}

/// Introspect columns the current MCP role can SELECT on `table_name`.
pub async fn describe_table(state: &AppState, claims: &UserClaims, table_name: &str) -> AppResult<Value> {
    if !is_safe_ident(table_name) {
        return Err(AppError::Validation("table_name must be a simple identifier".into()));
    }
    let sql = format!(
        r#"
        SELECT c.table_schema, c.table_name, c.column_name, c.data_type, c.is_nullable
        FROM information_schema.columns c
        WHERE c.table_name = '{table_name}'
          AND c.table_schema IN ('mcp', 'public')
          AND EXISTS (
            SELECT 1
            FROM information_schema.table_privileges p
            WHERE p.grantee = current_user
              AND p.privilege_type = 'SELECT'
              AND p.table_schema = c.table_schema
              AND p.table_name = c.table_name
          )
        ORDER BY c.table_schema, c.ordinal_position
        "#
    );
    // table_name is identifier-validated; still go through the guard + RLS session.
    let result = execute_query(state, claims, &sql).await?;
    Ok(json!({
        "columns": result.rows,
        "truncated": result.truncated,
    }))
}

fn is_safe_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => chars.all(|c| c.is_ascii_alphanumeric() || c == '_'),
        _ => false,
    }
}

/// Rows returned by an MCP query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<Value>,
    pub truncated: bool,
}

async fn fetch_json_rows(conn: &mut sqlx::PgConnection, sql: &str, max_rows: u32) -> AppResult<QueryResult> {
    let rows: Vec<PgRow> = sqlx::query(sql).fetch_all(&mut *conn).await?;
    let truncated = rows.len() as u32 >= max_rows;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let value: Value = row.try_get("row")?;
        out.push(value);
    }
    Ok(QueryResult { rows: out, truncated })
}

async fn fetch_explain(conn: &mut sqlx::PgConnection, sql: &str) -> AppResult<QueryResult> {
    let rows: Vec<PgRow> = sqlx::query(sql).fetch_all(&mut *conn).await?;
    let mut plans = Vec::with_capacity(rows.len());
    for row in rows {
        let plan: String = row.try_get(0)?;
        plans.push(json!({ "plan": plan }));
    }
    Ok(QueryResult { rows: plans, truncated: false })
}
