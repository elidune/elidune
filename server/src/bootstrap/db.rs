//! Database pool creation and migrations.

use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::config::{AppConfig, DatabaseConfig};

/// Create a PostgreSQL pool and run pending migrations.
pub async fn connect_pool(db: &DatabaseConfig) -> Result<Pool<Postgres>, sqlx::Error> {
    let pool = PgPoolOptions::new().max_connections(db.max_connections).min_connections(db.min_connections).connect(&db.url).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Connect as the MCP login role (`elidune_mcp`). Does not run migrations.
pub async fn connect_mcp_pool(url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    PgPoolOptions::new().max_connections(5).min_connections(0).connect(url).await
}

/// Connect the MCP pool when enabled. Logs and returns `None` if the role is unreachable.
pub async fn try_mcp_pool(config: &AppConfig) -> Option<Pool<Postgres>> {
    if !config.mcp.enabled {
        tracing::info!("MCP server disabled");
        return None;
    }
    let url = config.mcp.resolved_database_url(&config.database.url);
    match connect_mcp_pool(&url).await {
        Ok(pool) => {
            tracing::info!("Connected MCP database role");
            Some(pool)
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                "MCP database connection failed; MCP endpoint will return 503"
            );
            None
        }
    }
}
