//! Custom account types (#201): delete rules, last admin, audit, session revocation.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};
use sqlx::Row;

fn type_code(prefix: &str) -> String {
    format!("{prefix}{}", fixtures::unique_suffix() % 1_000_000_000)
}

async fn admin(app: &TestApp) -> (String, i64) {
    let token = fixtures::ensure_first_setup(app).await;
    let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "admin profile: {body}");
    (token, fixtures::json_id(&body["id"]))
}

async fn create_type(app: &TestApp, token: &str, code: &str, loans: &str) -> Value {
    let (status, body) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": format!("Type {code}"),
                "loansRights": loans,
                "settingsRights": "n"
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create type: {body}");
    assert_eq!(body["code"], code);
    assert_eq!(body["isSystem"], false);
    body
}

async fn create_user_of_type(app: &TestApp, admin_token: &str, code: &str) -> (i64, String) {
    let login = format!("acct_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Custom",
                "lastname": "Type",
                "email": format!("{login}@test.local"),
                "accountType": code,
                "publicType": public_type_id.to_string(),
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create user: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (login_status, login_body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(login_status, StatusCode::OK, "login: {login_body}");
    let token = login_body["token"].as_str().expect("token").to_string();
    (user_id, token)
}

fn assert_conflict(status: StatusCode, body: &Value, code: &str) {
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], code);
    assert!(body["error"].is_string() && !body["error"].as_str().unwrap_or("").is_empty());
    assert!(body["message"].is_string() && !body["message"].as_str().unwrap_or("").is_empty());
}

#[tokio::test]
async fn delete_assigned_account_type_returns_409() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (token, _) = admin(&app).await;
    let code = type_code("use");
    create_type(&app, &token, &code, "n").await;
    let (user_id, _) = create_user_of_type(&app, &token, &code).await;

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/account-types/{code}"), &token)
        .await;
    assert_conflict(status, &body, "account_type_in_use");

    let (moved, move_body) = app
        .put_json(
            &format!("/api/v1/users/{user_id}/account-type"),
            &json!({ "accountType": "reader" }),
            Some(&token),
        )
        .await;
    assert_eq!(moved, StatusCode::OK, "reassign: {move_body}");

    let (deleted, deleted_body) = app
        .delete_with_auth(&format!("/api/v1/account-types/{code}"), &token)
        .await;
    assert_eq!(deleted, StatusCode::NO_CONTENT, "{deleted_body}");
}

#[tokio::test]
async fn system_account_types_cannot_be_deleted() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (token, _) = admin(&app).await;
    for code in ["admin", "librarian", "reader"] {
        let (status, body) = app
            .get_json_with_auth(&format!("/api/v1/account-types/{code}"), &token)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["isSystem"], true, "{code}");

        let (status, body) = app
            .delete_with_auth(&format!("/api/v1/account-types/{code}"), &token)
            .await;
        assert_conflict(status, &body, "account_type_system");
    }
}

#[tokio::test]
async fn last_active_admin_is_protected() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (token, admin_id) = admin(&app).await;

    let (status, body) = app
        .put_json(
            "/api/v1/account-types/admin",
            &json!({ "settingsRights": "r" }),
            Some(&token),
        )
        .await;
    assert_conflict(status, &body, "last_active_admin");
    let (status, body) = app
        .get_json_with_auth("/api/v1/account-types/admin", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["settingsRights"], "w");

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{admin_id}/account-type"),
            &json!({ "accountType": "reader" }),
            Some(&token),
        )
        .await;
    assert_conflict(status, &body, "last_active_admin");

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{admin_id}"),
            &json!({ "status": "blocked" }),
            Some(&token),
        )
        .await;
    assert_conflict(status, &body, "last_active_admin");

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{admin_id}"), &token)
        .await;
    assert_conflict(status, &body, "last_active_admin");

    let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "admin session still valid: {body}");
    assert_eq!(body["accountType"], "admin");
}

#[tokio::test]
async fn rights_change_writes_an_audit_row() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (token, admin_id) = admin(&app).await;
    let code = type_code("aud");
    create_type(&app, &token, &code, "n").await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/account-types/{code}"),
            &json!({ "loansRights": "r" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loansRights"], "r");

    let pool = app.state.services.repository.pool();
    let mut found = None;
    for _ in 0..50 {
        let row = sqlx::query(
            "SELECT user_id, payload FROM audit_log \
             WHERE event_type = 'account_type.updated' AND payload->>'code' = $1 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&code)
        .fetch_optional(pool)
        .await
        .expect("audit query");
        if let Some(row) = row {
            let user_id: Option<i64> = row.get("user_id");
            let payload: Value = row.get("payload");
            found = Some((user_id, payload));
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (user_id, payload) = found.expect("audit row for the rights change");
    assert_eq!(user_id, Some(admin_id));
    assert_eq!(payload["code"], code);
    assert_eq!(payload["before"]["loansRights"], "n");
    assert_eq!(payload["after"]["loansRights"], "r");
}

#[tokio::test]
async fn removing_a_right_invalidates_open_sessions() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (admin_token, _) = admin(&app).await;
    let code = type_code("ses");
    create_type(&app, &admin_token, &code, "n").await;
    let (_, user_token) = create_user_of_type(&app, &admin_token, &code).await;

    let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &user_token).await;
    assert_eq!(status, StatusCode::OK, "session before change: {body}");

    let (status, body) = app
        .put_json(
            &format!("/api/v1/account-types/{code}"),
            &json!({ "loansRights": "r" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "add right: {body}");
    let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &user_token).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "added right keeps the session: {body}"
    );

    let (status, body) = app
        .put_json(
            &format!("/api/v1/account-types/{code}"),
            &json!({ "loansRights": "n" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "remove right: {body}");
    let (status, body) = app.get_json_with_auth("/api/v1/auth/me", &user_token).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "removed right revokes the session: {body}"
    );
    assert_eq!(body["code"], "authentication_failed");
}
