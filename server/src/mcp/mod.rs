//! Read-only MCP server: Streamable HTTP JSON-RPC over JWT, SQL under PostgreSQL RLS.

mod executor;
mod protocol;
mod roles;
mod sql_guard;
mod tools;

use axum::routing::post;
use axum::Router;

use crate::AppState;

pub use sql_guard::{guard_sql, GuardedSql};

/// MCP routes mounted at `/mcp` inside `/api/v1`.
pub fn router() -> Router<AppState> {
    Router::new().route("/mcp", post(protocol::handle_post).get(protocol::handle_get).delete(protocol::handle_delete))
}

/// Build `postgres://elidune_mcp:elidune_mcp@...` from the application DATABASE URL.
pub fn derive_mcp_database_url(app_url: &str) -> String {
    const USERINFO: &str = "elidune_mcp:elidune_mcp";
    for prefix in ["postgres://", "postgresql://"] {
        if let Some(rest) = app_url.strip_prefix(prefix) {
            if let Some(at) = rest.find('@') {
                return format!("{prefix}{USERINFO}@{}", &rest[at + 1..]);
            }
        }
    }
    app_url.to_string()
}

#[cfg(test)]
mod tests {
    use super::derive_mcp_database_url;

    #[test]
    fn derives_mcp_user_from_app_url() {
        assert_eq!(
            derive_mcp_database_url("postgres://elidune:elidune@localhost:5432/elidune_test"),
            "postgres://elidune_mcp:elidune_mcp@localhost:5432/elidune_test"
        );
    }
}
