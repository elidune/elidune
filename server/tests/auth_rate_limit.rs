//! Auth rate limits: `/auth/me` stays available, per-identifier login blocks are temporary.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::fixtures;
use common::TestApp;
use elidune_server::AppBuildOptions;
use http_body_util::BodyExt;
use serde_json::{json, Value};

fn strict_ip_options() -> AppBuildOptions {
    let mut options = AppBuildOptions::for_tests();
    // One token per minute after a burst of 2. `/auth/me` must not share this bucket.
    options.auth_ip_per_minute = Some(1);
    options.auth_ip_burst = Some(2);
    options.auth_session_per_minute = Some(600);
    options.auth_session_burst = Some(60);
    options.auth_identifier_max_failures = Some(100);
    options
}

fn identifier_options() -> AppBuildOptions {
    let mut options = AppBuildOptions::for_tests();
    options.auth_ip_per_minute = Some(600);
    options.auth_ip_burst = Some(100);
    options.auth_identifier_max_failures = Some(3);
    options.auth_identifier_window_secs = Some(900);
    options.auth_session_burst = Some(60);
    options
}

async fn post_login(app: &TestApp, username: &str, extra_headers: &[(&str, &str)]) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut builder = Request::builder().method("POST").uri("/api/v1/auth/login").header("content-type", "application/json");
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .request(builder.body(Body::from(json!({ "username": username, "password": "wrong-password-1" }).to_string())).unwrap())
        .await;
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

fn assert_rate_limited(status: StatusCode, headers: &axum::http::HeaderMap, body: &Value) {
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["code"], "rate_limited");
    let retry_after = body["retryAfter"].as_u64().expect("retryAfter");
    assert!(retry_after >= 1, "retryAfter={retry_after}");
    let header = headers.get("retry-after").and_then(|v| v.to_str().ok()).expect("Retry-After");
    assert_eq!(header, retry_after.to_string());
    assert!(body["error"].is_string());
    assert!(body["message"].is_string());
}

#[tokio::test]
async fn auth_me_stays_available_when_strict_ip_limit_is_exhausted() {
    let Some(app) = TestApp::spawn_with_options(strict_ip_options()).await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;

    for i in 0..10 {
        let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
        assert_eq!(status, StatusCode::OK, "call {i}: {body}");
        assert!(body["login"].is_string(), "{body}");
    }

    let mut saw_limit = false;
    for _ in 0..3 {
        let (status, headers, body) = post_login(&app, "nobody", &[]).await;
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert_rate_limited(status, &headers, &body);
            saw_limit = true;
            break;
        }
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    }
    assert!(saw_limit, "strict IP limiter did not return 429");

    // Unset trusted proxies: a forged X-Forwarded-For must stay on the peer bucket.
    let (status, headers, body) = post_login(&app, "nobody", &[("x-forwarded-for", "203.0.113.9")]).await;
    assert_rate_limited(status, &headers, &body);
}

#[tokio::test]
async fn login_identifier_block_is_per_account_and_staff_can_clear_it() {
    let Some(app) = TestApp::spawn_with_options(identifier_options()).await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let victim = format!("Victim{suffix}");
    let other = format!("other{suffix}");

    for attempt in 1..=3 {
        let (status, _, body) = post_login(&app, &victim, &[]).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "attempt {attempt} should still be checked: {body}");
        assert_ne!(body["code"], "rate_limited");
    }

    let (status, headers, body) = post_login(&app, &victim, &[]).await;
    assert_rate_limited(status, &headers, &body);
    let retry_after = body["retryAfter"].as_u64().unwrap();
    assert!(retry_after <= 900, "retryAfter={retry_after}");

    let (status, _, body) = post_login(&app, &other, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "other identifier: {body}");
    assert_eq!(body["code"], "authentication_failed");

    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "unblocker").await;
    let (status, body) = app.delete_with_auth(&format!("/api/v1/auth/rate-limit/{victim}"), &reader_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    let (status, body) = app.delete_with_auth(&format!("/api/v1/auth/rate-limit/{}", victim.to_lowercase()), &admin_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (status, _, body) = post_login(&app, &victim, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "cleared identifier should be checked again: {body}");
    assert_ne!(body["code"], "rate_limited");
}
