//! Shared harness for in-process HTTP integration tests.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Notify;
use tower::ServiceExt;

use elidune_server::{
    build_app_with_options, repository::Repository, services::Services, AppBuildOptions, AppConfig,
    AppState, DynamicConfig, EmailService,
};

pub mod fixtures;

/// In-process test application with database, Redis, and HTTP router.
pub struct TestApp {
    pub router: Router,
    pub state: AppState,
}

impl TestApp {
    /// Build a test app with permissive rate limits.
    /// Skips the test when `DATABASE_URL` is unset and `CI` is not set.
    pub async fn spawn() -> Option<Self> {
        Self::spawn_with_options(AppBuildOptions::for_tests()).await
    }

    /// Build a test app with explicit router options (rate-limit tests).
    pub async fn spawn_with_options(options: AppBuildOptions) -> Option<Self> {
        Self::spawn_with_config(AppConfig::for_test(), options).await
    }

    /// Build a test app from a full config (password-reset link tests).
    pub async fn spawn_with_config(config: AppConfig, options: AppBuildOptions) -> Option<Self> {
        if std::env::var("DATABASE_URL").is_err() && std::env::var("CI").is_err() {
            eprintln!("Skipping integration test: DATABASE_URL not set");
            return None;
        }

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database.url)
            .await
            .expect("connect test database");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");

        let dynamic_config = DynamicConfig::new(config.clone());

        let redis_service = elidune_server::services::redis::RedisService::new(&config.redis.url)
            .await
            .expect("connect redis");

        let email_service = Arc::new(EmailService::new(dynamic_config.clone(), pool.clone()));

        let repository = Repository::new(pool, Some(dynamic_config.clone()));

        let (event_bus_tx, _) = tokio::sync::broadcast::channel(256);
        let event_bus = elidune_server::services::event_bus::EventBus::new(event_bus_tx.clone());

        let services = Arc::new(
            Services::new(
                repository,
                config.users.clone(),
                dynamic_config.clone(),
                config.redis.clone(),
                redis_service,
                None,
                email_service,
                event_bus,
                config.public_base_url(),
            )
            .await
            .expect("create services"),
        );

        let state = AppState {
            config: Arc::new(config),
            dynamic_config,
            services,
            scheduler_notify: Arc::new(Notify::new()),
            event_bus: event_bus_tx,
        };

        let router = build_app_with_options(state.clone(), options);

        Some(Self { router, state })
    }

    /// Send an HTTP request and return the full response.
    pub async fn request(&self, mut req: Request<Body>) -> Response<Body> {
        // `GovernorLayer` keys on peer IP; oneshot requests have no socket unless we set it.
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 3000))));
        self.router
            .clone()
            .oneshot(req)
            .await
            .expect("router oneshot")
    }

    /// GET helper returning status and JSON body.
    pub async fn get_json(&self, uri: &str) -> (StatusCode, serde_json::Value) {
        let response = self
            .request(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// POST JSON helper.
    pub async fn post_json(
        &self,
        uri: &str,
        body: &serde_json::Value,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        self.post_json_with_headers(uri, body, auth_token, &[])
            .await
    }

    /// POST JSON with extra headers (e.g. `Idempotency-Key`).
    pub async fn post_json_with_headers(
        &self,
        uri: &str,
        body: &serde_json::Value,
        auth_token: Option<&str>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method("POST").uri(uri);
        if let Some(token) = auth_token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let response = self
            .request(
                builder
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// GET helper with Bearer auth.
    pub async fn get_json_with_auth(
        &self,
        uri: &str,
        token: &str,
    ) -> (StatusCode, serde_json::Value) {
        let response = self
            .request(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// PUT JSON helper.
    pub async fn put_json(
        &self,
        uri: &str,
        body: &serde_json::Value,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method("PUT").uri(uri);
        if let Some(token) = auth_token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        let response = self
            .request(
                builder
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// DELETE helper with Bearer auth.
    pub async fn delete_with_auth(
        &self,
        uri: &str,
        token: &str,
    ) -> (StatusCode, serde_json::Value) {
        let response = self
            .request(
                Request::builder()
                    .method("DELETE")
                    .uri(uri)
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// POST with empty body.
    pub async fn post_empty(
        &self,
        uri: &str,
        auth_token: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        self.post_empty_with_headers(uri, auth_token, &[]).await
    }

    /// POST empty body with extra headers (e.g. `Idempotency-Key`).
    pub async fn post_empty_with_headers(
        &self,
        uri: &str,
        auth_token: Option<&str>,
        headers: &[(&str, &str)],
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method("POST").uri(uri);
        if let Some(token) = auth_token {
            builder = builder.header("Authorization", format!("Bearer {token}"));
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let response = self.request(builder.body(Body::empty()).unwrap()).await;
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    /// Walk every page of a paginated GET until `predicate` matches one row.
    ///
    /// `path` may already include filters. `page` and `perPage` are appended.
    /// The route has no row filter, so a shared database can push a seeded row
    /// off the first page.
    pub async fn find_paged_row<F>(
        &self,
        token: &str,
        path: &str,
        items_key: &str,
        what: &str,
        predicate: F,
    ) -> serde_json::Value
    where
        F: Fn(&serde_json::Value) -> bool,
    {
        const PER_PAGE: i64 = 50;
        let mut page = 1_i64;
        loop {
            let sep = if path.contains('?') { '&' } else { '?' };
            let uri = format!("{path}{sep}page={page}&perPage={PER_PAGE}");
            let (status, body) = self.get_json_with_auth(&uri, token).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            let items = body
                .get(items_key)
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("{uri}: expected `{items_key}` array, got {body}"));
            if let Some(row) = items.iter().find(|row| predicate(row)) {
                return row.clone();
            }
            let total = body["total"].as_i64().unwrap_or(0);
            if items.is_empty() || page.saturating_mul(PER_PAGE) >= total {
                panic!("{what} not found after {page} page(s) of {uri} (total={total})");
            }
            page += 1;
        }
    }

    /// `GET /loans/overdue` has only `page` and `perPage`. Find the seeded loan on any page.
    pub async fn find_overdue_loan(&self, token: &str, loan_id: i64) -> serde_json::Value {
        self.find_paged_row(
            token,
            "/api/v1/loans/overdue",
            "loans",
            &format!("overdue loan {loan_id}"),
            |row| fixtures::json_id(&row["loanId"]) == loan_id,
        )
        .await
    }
}
