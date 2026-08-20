//! Application-level SQL allowlist for MCP queries (defense in depth; RLS is the real boundary).

use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::error::AppError;

/// Prepared statement safe to run in a read-only MCP transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedSql {
    /// `SELECT` / `WITH` wrapped so each row is JSON and row count is capped.
    Select { sql: String },
    /// `EXPLAIN` of a `SELECT` (not wrapped).
    Explain { sql: String },
}

/// Parse `sql` and accept only a single read-only `SELECT`/`WITH` or `EXPLAIN SELECT`.
pub fn guard_sql(sql: &str, max_rows: u32) -> Result<GuardedSql, AppError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("SQL query is empty".into()));
    }
    if max_rows == 0 {
        return Err(AppError::Validation("max_rows must be greater than 0".into()));
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, trimmed).map_err(|e| AppError::Validation(format!("Invalid SQL: {e}")))?;

    if statements.len() != 1 {
        return Err(AppError::Validation("Only a single SQL statement is allowed".into()));
    }

    match &statements[0] {
        Statement::Query(_) => {
            let inner = trimmed.trim_end_matches(';').trim();
            Ok(GuardedSql::Select { sql: wrap_select(inner, max_rows) })
        }
        Statement::Explain { statement, .. } => {
            if !matches!(statement.as_ref(), Statement::Query(_)) {
                return Err(AppError::Validation("EXPLAIN is only allowed for SELECT queries".into()));
            }
            Ok(GuardedSql::Explain {
                sql: trimmed.trim_end_matches(';').trim().to_string(),
            })
        }
        other => Err(AppError::Validation(format!("Only SELECT / WITH / EXPLAIN SELECT are allowed, got {}", statement_kind(other)))),
    }
}

fn wrap_select(sql: &str, max_rows: u32) -> String {
    format!("SELECT to_jsonb(mcp_q) AS row FROM ({sql}) AS mcp_q LIMIT {max_rows}")
}

fn statement_kind(stmt: &Statement) -> &'static str {
    match stmt {
        Statement::Insert { .. } => "INSERT",
        Statement::Update { .. } => "UPDATE",
        Statement::Delete { .. } => "DELETE",
        Statement::CreateTable { .. } => "CREATE TABLE",
        Statement::AlterTable { .. } => "ALTER TABLE",
        Statement::Drop { .. } => "DROP",
        Statement::Copy { .. } => "COPY",
        Statement::SetRole { .. }
        | Statement::SetVariable { .. }
        | Statement::SetTimeZone { .. }
        | Statement::SetNames { .. }
        | Statement::SetNamesDefault { .. }
        | Statement::SetTransaction { .. } => "SET",
        Statement::Grant { .. } => "GRANT",
        Statement::Revoke { .. } => "REVOKE",
        Statement::Truncate { .. } => "TRUNCATE",
        Statement::Analyze { .. } => "ANALYZE",
        Statement::Execute { .. } => "EXECUTE",
        Statement::Prepare { .. } => "PREPARE",
        _ => "unsupported statement",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select_sql(sql: &str) -> String {
        match guard_sql(sql, 200).expect("guard") {
            GuardedSql::Select { sql } => sql,
            other => panic!("expected Select, got {other:?}"),
        }
    }

    #[test]
    fn accepts_select_and_wraps_limit() {
        let out = select_sql("SELECT id FROM biblios");
        assert!(out.starts_with("SELECT to_jsonb(mcp_q) AS row FROM (SELECT id FROM biblios)"));
        assert!(out.ends_with("LIMIT 200"));
    }

    #[test]
    fn accepts_with_cte() {
        let out = select_sql("WITH x AS (SELECT 1 AS n) SELECT n FROM x");
        assert!(out.contains("WITH x AS"));
        assert!(out.ends_with("LIMIT 50") || out.contains("LIMIT 200"));
    }

    #[test]
    fn accepts_explain_select() {
        match guard_sql("EXPLAIN SELECT 1", 200).unwrap() {
            GuardedSql::Explain { sql } => assert_eq!(sql, "EXPLAIN SELECT 1"),
            other => panic!("expected Explain, got {other:?}"),
        }
    }

    #[test]
    fn rejects_empty() {
        let err = guard_sql("   ", 200).unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    #[test]
    fn rejects_insert() {
        let err = guard_sql("INSERT INTO users (login) VALUES ('x')", 200).unwrap_err().to_string();
        assert!(err.contains("INSERT"));
    }

    #[test]
    fn rejects_multiple_statements() {
        let err = guard_sql("SELECT 1; SELECT 2", 200).unwrap_err().to_string();
        assert!(err.contains("single"));
    }

    #[test]
    fn rejects_set_role() {
        let err = guard_sql("SET ROLE elidune_mcp_admin", 200).unwrap_err().to_string();
        assert!(err.contains("SET") || err.contains("Only SELECT"));
    }

    #[test]
    fn rejects_drop() {
        let err = guard_sql("DROP TABLE users", 200).unwrap_err().to_string();
        assert!(err.contains("DROP") || err.contains("Only SELECT"));
    }

    #[test]
    fn rejects_explain_insert() {
        let err = guard_sql("EXPLAIN INSERT INTO users (login) VALUES ('x')", 200).unwrap_err().to_string();
        assert!(err.contains("EXPLAIN"));
    }
}
