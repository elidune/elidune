//! Test data builders for integration tests.

#![allow(dead_code)]

use chrono::NaiveDate;
use serde_json::json;

/// Parse a snowflake ID from JSON (string or number).
pub fn json_id(value: &serde_json::Value) -> i64 {
    value
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| value.as_i64())
        .expect("expected numeric id in JSON")
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
///
/// Parallel integration tests share one database, so first-setup can race.
/// If another test already completed bootstrap, fall through to admin login.
pub async fn ensure_first_setup(app: &super::TestApp) -> String {
    let (_, health) = app.get_json("/api/v1/health").await;
    if health["setup"]["needFirstSetup"].as_bool() == Some(true) {
        let (status, body) = app
            .post_json("/api/v1/first_setup", &first_setup_payload(), None)
            .await;
        if status == axum::http::StatusCode::CREATED {
            return body["token"]
                .as_str()
                .expect("token in first_setup response")
                .to_string();
        }
    }

    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": "testadmin", "password": "testadmin1234" }),
            None,
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "login failed: {body}");
    body["token"]
        .as_str()
        .expect("token in login response")
        .to_string()
}

async fn first_public_type_id(app: &super::TestApp, admin_token: &str) -> i64 {
    let (status, body) = app
        .get_json_with_auth("/api/v1/public-types", admin_token)
        .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "list public types: {body}"
    );
    let types = body.as_array().expect("public types array");
    assert!(!types.is_empty(), "expected seeded public types");
    let chosen = types
        .iter()
        .find(|t| t["name"] == "adult")
        .unwrap_or(&types[0]);
    json_id(&chosen["id"])
}

/// Suffix so parallel / rerun tests do not collide on a shared database.
pub fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
}

/// Create a reader user via API; returns (user_id, token).
pub async fn create_reader(app: &super::TestApp, admin_token: &str, login: &str) -> (i64, String) {
    let login = format!("{login}_{}", unique_suffix());
    let public_type_id = first_public_type_id(app, admin_token).await;
    let payload = json!({
        "login": login,
        "password": "readerpass1234",
        "firstname": "Reader",
        "lastname": login,
        "email": format!("{login}@test.local"),
        "accountType": "reader",
        "publicType": public_type_id.to_string(),
        "sex": "f",
        "birthdate": NaiveDate::from_ymd_opt(1990, 6, 1).unwrap().to_string(),
        "addrCity": "Paris"
    });

    let (status, body) = app
        .post_json("/api/v1/users", &payload, Some(admin_token))
        .await;
    assert_eq!(
        status,
        axum::http::StatusCode::CREATED,
        "create user: {body}"
    );

    let user_id = json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password for tests");

    let (login_status, login_body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(
        login_status,
        axum::http::StatusCode::OK,
        "reader login: {login_body}"
    );

    let token = login_body["token"].as_str().unwrap().to_string();
    (user_id, token)
}
