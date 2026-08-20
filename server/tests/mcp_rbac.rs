//! MCP RBAC: patrons cannot read other users' PII; secrets never exposed.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

async fn spawn_app() -> Option<(tokio::sync::MutexGuard<'static, ()>, TestApp)> {
    let guard = TEST_LOCK.lock().await;
    TestApp::spawn().await.map(|app| (guard, app))
}

async fn mcp_call(app: &TestApp, token: Option<&str>, method: &str, params: Value) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/mcp",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        }),
        token,
    )
    .await
}

async fn mcp_query(app: &TestApp, token: &str, sql: &str) -> (StatusCode, Value) {
    mcp_call(app, Some(token), "tools/call", json!({ "name": "query", "arguments": { "sql": sql } })).await
}

fn structured(body: &Value) -> &Value {
    &body["result"]["structuredContent"]
}

fn is_tool_error(body: &Value) -> bool {
    body["result"]["isError"].as_bool() == Some(true)
}

fn row_emails(body: &Value) -> Vec<String> {
    structured(body)["rows"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|r| r["email"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn unauthenticated_mcp_is_rejected() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let (status, _) = mcp_call(&app, None, "initialize", json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn reader_cannot_see_other_users_email() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_id_a, token_a) = fixtures::create_reader(&app, &admin_token, "mcpreader_a").await;
    let (_id_b, _token_b) = fixtures::create_reader(&app, &admin_token, "mcpreader_b").await;

    let (status, body) = mcp_query(&app, &token_a, "SELECT email FROM users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!is_tool_error(&body), "{body}");
    let emails = row_emails(&body);
    assert_eq!(emails, vec!["mcpreader_a@test.local".to_string()]);
    assert!(!emails.iter().any(|e| e.contains("mcpreader_b")));
}

#[tokio::test]
async fn librarian_can_see_other_users_email() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_id_r, _token_r) = fixtures::create_reader(&app, &admin_token, "mcplib_reader").await;
    let (_id_l, lib_token) = fixtures::create_user_with_type(&app, &admin_token, "mcplibrarian", "librarian").await;

    let (status, body) = mcp_query(&app, &lib_token, "SELECT email FROM users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!is_tool_error(&body), "{body}");
    let emails = row_emails(&body);
    assert!(emails.iter().any(|e| e.contains("mcplib_reader")), "librarian should see patron email, got {emails:?}");
}

#[tokio::test]
async fn nobody_can_select_password_column() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = mcp_query(&app, &admin_token, "SELECT password FROM users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(is_tool_error(&body), "password column must not be visible: {body}");
}

#[tokio::test]
async fn writes_and_set_role_are_rejected() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = mcp_query(&app, &admin_token, "INSERT INTO biblios (title) VALUES ('x')").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(is_tool_error(&body), "INSERT must be rejected: {body}");

    let (status, body) = mcp_query(&app, &admin_token, "SET ROLE elidune_mcp_admin").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(is_tool_error(&body), "SET ROLE must be rejected: {body}");
}

#[tokio::test]
async fn librarian_cannot_read_audit_log_admin_can() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_id, lib_token) = fixtures::create_user_with_type(&app, &admin_token, "mcplib_audit", "librarian").await;

    let (status, admin_body) = mcp_query(&app, &admin_token, "SELECT id FROM audit_log LIMIT 1").await;
    assert_eq!(status, StatusCode::OK, "{admin_body}");
    assert!(!is_tool_error(&admin_body), "admin should read audit_log: {admin_body}");

    let (status, lib_body) = mcp_query(&app, &lib_token, "SELECT id FROM audit_log LIMIT 1").await;
    assert_eq!(status, StatusCode::OK, "{lib_body}");
    assert!(is_tool_error(&lib_body), "librarian must not read audit_log: {lib_body}");
}

#[tokio::test]
async fn guest_cannot_read_users_but_can_read_catalog() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_id, guest_token) = fixtures::create_user_with_type(&app, &admin_token, "mcpguest", "guest").await;

    let (status, body) = mcp_query(&app, &guest_token, "SELECT email FROM users").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(is_tool_error(&body), "guest must not read users: {body}");

    let (status, body) = mcp_query(&app, &guest_token, "SELECT id FROM biblios").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!is_tool_error(&body), "guest should read catalog: {body}");
}

#[tokio::test]
async fn reader_join_loans_does_not_leak_other_users() {
    let Some((_guard, app)) = spawn_app().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (id_a, token_a) = fixtures::create_reader(&app, &admin_token, "mcpjoin_a").await;
    let (id_b, _token_b) = fixtures::create_reader(&app, &admin_token, "mcpjoin_b").await;

    let pool = app.state.services.repository.pool();
    sqlx::query("INSERT INTO loans (user_id, date) VALUES ($1, NOW()), ($2, NOW())")
        .bind(id_a)
        .bind(id_b)
        .execute(pool)
        .await
        .expect("seed loans");

    let (status, body) = mcp_query(&app, &token_a, "SELECT u.email FROM loans l JOIN users u ON u.id = l.user_id").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!is_tool_error(&body), "{body}");
    let emails = row_emails(&body);
    assert_eq!(emails, vec!["mcpjoin_a@test.local".to_string()]);
}
