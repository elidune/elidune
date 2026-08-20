//! Test data builders for integration tests.

use chrono::NaiveDate;
use serde_json::json;

/// Parse a snowflake ID from JSON (string or number).
pub fn json_id(value: &serde_json::Value) -> i64 {
    value.as_str().and_then(|s| s.parse().ok()).or_else(|| value.as_i64()).expect("expected numeric id in JSON")
}

/// `POST /api/v1/first_setup` payload for a fresh database.
pub fn first_setup_payload() -> serde_json::Value {
    json!({
        "admin": {
            "login": "testadmin",
            "password": "testadmin1234",
            "firstname": "Test",
            "lastname": "Admin",
            "email": "admin@test.local",
            "sex": "m",
            "birthdate": "1980-01-15",
            "language": "french"
        },
        "library": {
            "name": "Test Library",
            "addrCity": "Paris",
            "addrCountry": "FR"
        }
    })
}

/// Run first setup when the database is empty; returns JWT token.
pub async fn ensure_first_setup(app: &super::TestApp) -> String {
    let (_, health) = app.get_json("/api/v1/health").await;
    if health["setup"]["needFirstSetup"].as_bool() == Some(true) {
        let (status, body) = app.post_json("/api/v1/first_setup", &first_setup_payload(), None).await;
        assert_eq!(status, axum::http::StatusCode::CREATED, "first_setup failed: {body}");
        return body["token"].as_str().expect("token in first_setup response").to_string();
    }

    let (status, body) = app.post_json("/api/v1/auth/login", &json!({ "username": "testadmin", "password": "testadmin1234" }), None).await;
    assert_eq!(status, axum::http::StatusCode::OK, "login failed: {body}");
    body["token"].as_str().expect("token in login response").to_string()
}

/// Create a user with the given account type; returns (user_id, full JWT).
///
/// Admin-created accounts must change their password before other endpoints accept the token.
pub async fn create_user_with_type(app: &super::TestApp, admin_token: &str, login: &str, account_type: &str) -> (i64, String) {
    let (pt_status, pt_body) = app.get_json_with_auth("/api/v1/public-types", admin_token).await;
    assert_eq!(pt_status, axum::http::StatusCode::OK, "list public types: {pt_body}");
    let public_type_id = pt_body
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|row| row.get("id"))
        .map(json_id)
        .expect("at least one public type");

    // Unique login so the shared test database can be reused across runs.
    let unique_login = format!("{login}_{}", &uuid::Uuid::new_v4().to_string()[..8]);

    let payload = json!({
        "login": unique_login,
        "password": "readerpass1234",
        "firstname": "Test",
        "lastname": login,
        "email": format!("{login}@test.local"),
        "accountType": account_type,
        "sex": "f",
        "birthdate": NaiveDate::from_ymd_opt(1990, 6, 1).unwrap().to_string(),
        "addrCity": "Paris",
        "publicType": public_type_id.to_string()
    });

    let (status, body) = app.post_json("/api/v1/users", &payload, Some(admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "create user: {body}");

    let user_id = json_id(&body["id"]);

    let (login_status, login_body) = app.post_json("/api/v1/auth/login", &json!({ "username": unique_login, "password": "readerpass1234" }), None).await;
    assert_eq!(login_status, axum::http::StatusCode::OK, "login: {login_body}");

    let scoped_token = login_body["token"].as_str().expect("token in login response");
    let (cp_status, cp_body) = app.post_json("/api/v1/auth/change-password", &json!({ "newPassword": "readerpass5678" }), Some(scoped_token)).await;
    assert_eq!(cp_status, axum::http::StatusCode::OK, "change password: {cp_body}");

    let token = cp_body["token"].as_str().expect("full token after password change").to_string();
    (user_id, token)
}

/// Create a reader user via API; returns (user_id, token).
pub async fn create_reader(app: &super::TestApp, admin_token: &str, login: &str) -> (i64, String) {
    create_user_with_type(app, admin_token, login, "reader").await
}
