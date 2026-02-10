//! Elidune Server - Library Management System
//!
//! A modern Rust REST API server for library management.

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use elidune_server::{
    api,
    config::AppConfig,
    repository::Repository,
    services::Services,
    AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Load configuration
    let config = AppConfig::load().expect("Failed to load configuration");

    // Initialize tracing
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| format!("elidune_server={},tower_http=debug", config.logging.level).into());

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Elidune Server v{}", env!("CARGO_PKG_VERSION"));

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .min_connections(config.database.min_connections)
        .connect(&config.database.url)
        .await
        .expect("Failed to connect to database");

    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database migrations completed");

    // Initialize Redis connection
    let redis_service = elidune_server::services::redis::RedisService::new(&config.redis.url)
        .await
        .expect("Failed to connect to Redis");
    
    tracing::info!("Connected to Redis");

    // Save server address before moving config
    let server_host = config.server.host.clone();
    let server_port = config.server.port;

    // Create repository and services
    let repository = Repository::new(pool);
    let services = Services::new(
        repository,
        config.users.clone(),
        config.email.clone(),
        config.redis.clone(),
        redis_service,
    )
    .await
    .expect("Failed to create services");

    // Create application state
    let state = AppState {
        config: Arc::new(config),
        services: Arc::new(services),
    };

    // Build router
    let app = create_router(state);

    // Start server
    let addr = SocketAddr::new(
        server_host.parse().expect("Invalid host address"),
        server_port,
    );
    
    tracing::info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Create the application router with all routes
fn create_router(state: AppState) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API v1 routes
    let api_v1 = Router::new()
        // Health check
        .route("/health", get(api::health::health_check))
        .route("/ready", get(api::health::readiness_check))
        // Authentication
        .route("/auth/login", post(api::auth::login))
        .route("/auth/me", get(api::auth::me))
        .route("/auth/profile", put(api::users::update_my_profile))
        .route("/auth/verify-2fa", post(api::auth::verify_2fa))
        .route("/auth/verify-recovery", post(api::auth::verify_recovery))
        .route("/auth/setup-2fa", post(api::auth::setup_2fa))
        .route("/auth/disable-2fa", post(api::auth::disable_2fa))
        // Items (catalog)
        .route("/items", get(api::items::list_items))
        .route("/items", post(api::items::create_item))
        .route("/items/:id", get(api::items::get_item))
        .route("/items/:id", put(api::items::update_item))
        .route("/items/:id", delete(api::items::delete_item))
        .route("/items/:id/specimens", get(api::items::list_specimens))
        .route("/items/:id/specimens", post(api::items::create_specimen))
        // Specimens
        .route("/specimens/:id", delete(api::items::delete_specimen))
        // Users
        .route("/users", get(api::users::list_users))
        .route("/users", post(api::users::create_user))
        .route("/users/:id", get(api::users::get_user))
        .route("/users/:id", put(api::users::update_user))
        .route("/users/:id", delete(api::users::delete_user))
        .route("/users/:id/account-type", put(api::users::update_account_type))
        .route("/users/:id/loans", get(api::loans::get_user_loans))
        // Loans
        .route("/loans", post(api::loans::create_loan))
        .route("/loans/:id/return", post(api::loans::return_loan))
        .route("/loans/:id/renew", post(api::loans::renew_loan))
        // Z39.50
        .route("/z3950/search", get(api::z3950::search))
        .route("/z3950/import", post(api::z3950::import_record))
        // Statistics
        .route("/stats", get(api::stats::get_stats))
        .route("/stats/loans", get(api::stats::get_loan_stats))
        // Settings
        .route("/settings", get(api::settings::get_settings))
        .route("/settings", put(api::settings::update_settings))
        .with_state(state.clone());

    // OpenAPI documentation
    let openapi = api::openapi::create_openapi_router();

    Router::new()
        .nest("/api/v1", api_v1)
        .merge(openapi)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
