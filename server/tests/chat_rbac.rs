//! Chat API RBAC and conversation isolation.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

#[tokio::test]
async fn chat_requires_auth_and_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, providers) = app.get_json_with_auth("/api/v1/chat/providers", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{providers:?}");

    let (_id, guest_token) = fixtures::create_user_with_type(&app, &admin_token, "chatguest", "guest").await;
    let (status, _) = app.get_json_with_auth("/api/v1/chat/providers", &guest_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn chat_conversations_are_isolated_per_user() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let provider_id = seed_test_provider(&app, &admin_token).await;

    let (_id_a, token_a) = fixtures::create_user_with_type(&app, &admin_token, "chatuser_a", "librarian").await;
    let (_id_b, token_b) = fixtures::create_user_with_type(&app, &admin_token, "chatuser_b", "librarian").await;

    let (status, conv) = app
        .post_json("/api/v1/chat/conversations", &json!({ "providerId": provider_id.to_string(), "title": "Secret A" }), Some(&token_a))
        .await;
    assert_eq!(status, StatusCode::OK, "{conv:?}");
    let conv_id = conv["id"].as_str().unwrap();

    let (status, _) = app.get_json_with_auth(&format!("/api/v1/chat/conversations/{conv_id}"), &token_b).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn seed_test_provider(app: &TestApp, admin_token: &str) -> i64 {
    let body = json!({
        "slug": "test-local",
        "label": "Test Local",
        "kind": "openaiCompat",
        "baseUrl": "http://127.0.0.1:11434/v1",
        "models": ["test-model"],
        "defaultModel": "test-model",
        "enabled": true
    });
    let (status, resp) = app.post_json("/api/v1/admin/llm/providers", &body, Some(admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{resp:?}");
    fixtures::json_id(&resp["id"])
}

#[tokio::test]
async fn admin_llm_providers_hide_api_key() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let body = json!({
        "slug": "secret-test",
        "label": "Secret",
        "kind": "openaiCompat",
        "baseUrl": "http://example.invalid/v1",
        "apiKey": "super-secret-key",
        "models": ["m"],
        "defaultModel": "m"
    });
    let (status, created) = app.post_json("/api/v1/admin/llm/providers", &body, Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{created:?}");
    assert_eq!(created["apiKeySet"], true);
    assert!(created.get("apiKey").is_none());

    let (status, list) = app.get_json_with_auth("/api/v1/admin/llm/providers", &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    let arr = list.as_array().expect("array");
    let row = arr.iter().find(|r| r["slug"] == "secret-test").expect("row");
    assert_eq!(row["apiKeySet"], true);
    assert!(row.get("apiKey").is_none());
}
