//! HTTP application router assembly (shared by the binary and integration tests).

use std::sync::Arc;
use std::time::Duration;

use axum::extract::Extension;
use axum::response::IntoResponse;
use axum::{routing::get, Router};
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::{GovernorError, GovernorLayer};
use tower_http::trace::TraceLayer;

use crate::error::rate_limited_response;
use crate::rate_limit::{parse_trusted_proxies, replenish_period, AuthAttemptLimiter, PasswordResetAttemptLimiter, SessionKeyExtractor, TrustedProxyKeyExtractor};
use crate::{api, config::AppConfig, AppState};

/// Options for building the HTTP router (production defaults vs permissive test limits).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBuildOptions {
    /// Override strict auth IP quota (requests per minute). `None` uses `AppConfig`.
    pub auth_ip_per_minute: Option<u64>,
    /// Override strict auth IP burst. `None` uses `AppConfig`.
    pub auth_ip_burst: Option<u32>,
    /// Override per-identifier failure cap. `None` uses `AppConfig`.
    pub auth_identifier_max_failures: Option<u32>,
    /// Override per-identifier window, in seconds. `None` uses `AppConfig`.
    pub auth_identifier_window_secs: Option<u64>,
    /// Override password-reset request quota. `None` uses `AppConfig`.
    pub auth_password_reset_max: Option<u32>,
    /// Override password-reset window, in seconds. `None` uses `AppConfig`.
    pub auth_password_reset_window_secs: Option<u64>,
    /// Override authenticated auth-route quota (requests per minute, per user). `None` uses `AppConfig`.
    pub auth_session_per_minute: Option<u64>,
    /// Override authenticated auth-route burst. `None` uses `AppConfig`.
    pub auth_session_burst: Option<u32>,
    /// Override public API quota (requests per minute). `None` uses `AppConfig`.
    pub public_per_minute: Option<u64>,
    /// Override public burst size. `None` uses `AppConfig`.
    pub public_burst: Option<u32>,
    /// Spawn background thread to evict expired governor entries (disable in tests).
    pub spawn_rate_limit_cleanup: bool,
    /// Include OpenAPI / Swagger UI routes.
    pub include_openapi: bool,
    /// Include TraceLayer HTTP logging.
    pub include_trace_layer: bool,
}

impl Default for AppBuildOptions {
    fn default() -> Self {
        Self {
            auth_ip_per_minute: None,
            auth_ip_burst: None,
            auth_identifier_max_failures: None,
            auth_identifier_window_secs: None,
            auth_password_reset_max: None,
            auth_password_reset_window_secs: None,
            auth_session_per_minute: None,
            auth_session_burst: None,
            public_per_minute: None,
            public_burst: None,
            spawn_rate_limit_cleanup: true,
            include_openapi: true,
            include_trace_layer: true,
        }
    }
}

impl AppBuildOptions {
    /// High burst limits and no background cleanup — suitable for in-process integration tests.
    pub fn for_tests() -> Self {
        Self {
            auth_ip_per_minute: Some(10_000),
            auth_ip_burst: Some(10_000),
            auth_identifier_max_failures: Some(100_000),
            auth_identifier_window_secs: Some(60),
            auth_password_reset_max: Some(10_000),
            auth_password_reset_window_secs: Some(3600),
            auth_session_per_minute: Some(10_000),
            auth_session_burst: Some(10_000),
            public_per_minute: Some(10_000),
            public_burst: Some(10_000),
            spawn_rate_limit_cleanup: false,
            include_openapi: false,
            include_trace_layer: false,
        }
    }
}

/// Build the full Axum router with all API routes and middleware.
pub fn build_app(state: AppState) -> Router {
    build_app_with_options(state, AppBuildOptions::default())
}

/// Build the Axum router with explicit build options (used by integration tests).
pub fn build_app_with_options(state: AppState, options: AppBuildOptions) -> Router {
    let cors = build_cors(&state.config);

    let server_cfg = &state.config.server;
    let auth_ip_per_minute = options.auth_ip_per_minute.unwrap_or(server_cfg.auth_ip_per_minute);
    let auth_ip_burst = options.auth_ip_burst.unwrap_or(server_cfg.auth_ip_burst);
    let session_per_minute = options.auth_session_per_minute.unwrap_or(server_cfg.auth_session_per_minute);
    let session_burst = options.auth_session_burst.unwrap_or(server_cfg.auth_session_burst);
    let public_per_minute = options.public_per_minute.unwrap_or(server_cfg.public_per_minute);
    let public_burst = options.public_burst.unwrap_or(server_cfg.public_burst);
    let identifier_max = options.auth_identifier_max_failures.unwrap_or(server_cfg.auth_identifier_max_failures);
    let identifier_window = Duration::from_secs(options.auth_identifier_window_secs.unwrap_or(server_cfg.auth_identifier_window_secs).max(1));
    let reset_max = options.auth_password_reset_max.unwrap_or(server_cfg.auth_password_reset_max);
    let reset_window = Duration::from_secs(options.auth_password_reset_window_secs.unwrap_or(server_cfg.auth_password_reset_window_secs).max(1));

    let trusted = match parse_trusted_proxies(server_cfg.trusted_proxies.as_deref()) {
        Ok(nets) => Arc::new(nets),
        Err(err) => panic!("invalid server.trusted_proxies: {err}"),
    };
    let jwt_secret: Arc<str> = Arc::from(state.config.users.jwt_secret.as_str());
    let attempts = Arc::new(AuthAttemptLimiter::new(identifier_max, identifier_window));
    let resets = Arc::new(PasswordResetAttemptLimiter::new(reset_max, reset_window));

    let auth_ip_governor: &'static _ = Box::leak(Box::new(finish_ip_governor(auth_ip_per_minute, auth_ip_burst, Arc::clone(&trusted))));
    let session_governor: &'static _ = Box::leak(Box::new(finish_session_governor(session_per_minute, session_burst, Arc::clone(&trusted), Arc::clone(&jwt_secret))));
    let public_governor: &'static _ = Box::leak(Box::new(finish_ip_governor(public_per_minute, public_burst, trusted)));

    if options.spawn_rate_limit_cleanup {
        let auth_limiter = auth_ip_governor.limiter().clone();
        let session_limiter = session_governor.limiter().clone();
        let public_limiter = public_governor.limiter().clone();
        let attempts_cleanup = Arc::clone(&attempts);
        let resets_cleanup = Arc::clone(&resets);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            auth_limiter.retain_recent();
            session_limiter.retain_recent();
            public_limiter.retain_recent();
            attempts_cleanup.prune();
            resets_cleanup.prune();
        });
    }

    // Strict per-IP ceiling: login, 2FA, password reset, first setup.
    // `/auth/me` and `/auth/profile` use the session limiter below.
    let sensitive_router = api::auth::sensitive_router()
        .layer(GovernorLayer { config: auth_ip_governor })
        .layer(Extension(Arc::clone(&attempts)))
        .layer(Extension(Arc::clone(&resets)));

    let session_router = api::auth::session_router()
        .layer(GovernorLayer { config: session_governor })
        .layer(Extension(attempts))
        .layer(Extension(resets));

    use crate::features::{gated, Feature};

    let public_router = Router::new()
        .merge(api::opac::router())
        .merge(api::covers::router())
        .merge(api::library_info::router_public())
        .merge(gated(api::events::public_router(), state.clone(), Feature::Events))
        .layer(GovernorLayer { config: public_governor });

    let first_setup_router = api::first_setup::router().layer(GovernorLayer { config: auth_ip_governor });

    let api_v1 = Router::new()
        .merge(api::health::router())
        .merge(first_setup_router)
        .merge(sensitive_router)
        .merge(session_router)
        .merge(public_router)
        .merge(api::biblios::router())
        .merge(api::items::router())
        .merge(gated(api::withdrawals::router(), state.clone(), Feature::Weeding))
        .merge(api::users::router())
        .merge(api::loans::router())
        .merge(api::batch::router())
        .merge(api::holds::router())
        .merge(gated(api::transits::router(), state.clone(), Feature::Transits))
        .merge(api::fines::router())
        .merge(gated(api::inventory::router(), state.clone(), Feature::Inventory))
        .merge(api::sse::router())
        .merge(gated(api::z3950::router(), state.clone(), Feature::Z3950))
        .merge(gated(api::stats::router(), state.clone(), Feature::Stats))
        .merge(api::library_info::router_staff())
        .merge(api::email_templates::router())
        .merge(api::admin_config::router())
        .merge(api::audit::router())
        .merge(api::public_types::router())
        .merge(api::genres::router())
        .merge(gated(api::visitor_counts::router(), state.clone(), Feature::VisitorCounts))
        .merge(api::schedules::router())
        .merge(api::series::router())
        .merge(api::collections::router())
        .merge(api::sites::router())
        .merge(gated(api::deposits::router(), state.clone(), Feature::Deposits))
        .merge(api::equipment::router())
        .merge(gated(api::events::router(), state.clone(), Feature::Events))
        .merge(gated(api::acquisitions::router(), state.clone(), Feature::Acquisitions))
        .merge(gated(api::suggestions::router(), state.clone(), Feature::PurchaseSuggestions))
        .merge(api::account_types::router())
        .merge(api::maintenance::router())
        .merge(api::tasks::router())
        .with_state(state.clone());

    let mut app = Router::new()
        .route("/version", get(api::health::version))
        .merge(api::metrics::router().with_state(state.clone()))
        .nest("/api/v1", api_v1);

    if options.include_openapi {
        app = app.merge(api::openapi::create_openapi_router());
    }

    if options.include_trace_layer {
        app = app.layer(TraceLayer::new_for_http());
    }

    app.layer(cors)
}

fn finish_ip_governor(per_minute: u64, burst: u32, trusted: Arc<Vec<ipnet::IpNet>>) -> tower_governor::governor::GovernorConfig<TrustedProxyKeyExtractor, governor::middleware::NoOpMiddleware> {
    let mut builder = GovernorConfigBuilder::default();
    builder.period(replenish_period(per_minute)).burst_size(burst.max(1));
    let mut builder = builder.key_extractor(TrustedProxyKeyExtractor { trusted });
    builder.error_handler(governor_error_response);
    builder.finish().expect("rate limiter burst and period are non-zero")
}

fn finish_session_governor(
    per_minute: u64,
    burst: u32,
    trusted: Arc<Vec<ipnet::IpNet>>,
    jwt_secret: Arc<str>,
) -> tower_governor::governor::GovernorConfig<SessionKeyExtractor, governor::middleware::NoOpMiddleware> {
    let mut builder = GovernorConfigBuilder::default();
    builder.period(replenish_period(per_minute)).burst_size(burst.max(1));
    let mut builder = builder.key_extractor(SessionKeyExtractor { trusted, jwt_secret });
    builder.error_handler(governor_error_response);
    builder.finish().expect("rate limiter burst and period are non-zero")
}

fn governor_error_response(error: GovernorError) -> axum::response::Response {
    match error {
        GovernorError::TooManyRequests { wait_time, .. } => rate_limited_response(wait_time),
        GovernorError::UnableToExtractKey => {
            let body = axum::Json(serde_json::json!({
                "code": "internal_error",
                "error": "Internal Server Error",
                "message": "Unable to determine client address",
            }));
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
        }
        GovernorError::Other { code, msg, .. } => {
            let message = msg.unwrap_or_else(|| "Request could not be rate limited".to_string());
            let body = axum::Json(serde_json::json!({
                "code": "internal_error",
                "error": "Internal Server Error",
                "message": message,
            }));
            (code, body).into_response()
        }
    }
}

/// Build the CORS layer from configuration.
///
/// When `server.cors_origins` is set, only listed origins are allowed.
/// When empty or absent, CORS falls back to `Any` (development mode).
pub fn build_cors(config: &AppConfig) -> tower_http::cors::CorsLayer {
    use axum::http::HeaderValue;
    use tower_http::cors::{Any, CorsLayer};

    if let Some(ref origins) = config.server.cors_origins {
        if !origins.is_empty() {
            let parsed: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
            if !parsed.is_empty() {
                return CorsLayer::new()
                    .allow_origin(parsed)
                    .allow_methods(Any)
                    .allow_headers(Any)
                    .expose_headers([axum::http::header::CONTENT_DISPOSITION, axum::http::header::CONTENT_TYPE]);
            }
        }
    }

    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers([axum::http::header::CONTENT_DISPOSITION, axum::http::header::CONTENT_TYPE])
}
