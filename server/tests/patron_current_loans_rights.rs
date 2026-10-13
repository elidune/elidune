//! `GET /users/:id/loans` for a desk agent who can circulate but cannot read users.
//!
//! Current loans are visible with `{ id, cardNumber, initials }` only.
//! `archived=true` stays on the users-read denial.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::{json, Value};

fn retarget(token: &str, secret: &str, users: Rights, loans: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.users_rights = users;
    claims.rights.loans_rights = loans;
    claims.create_token(secret).expect("mint test token")
}

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

fn assert_error_body(body: &Value) {
    assert_eq!(
        sorted_keys(body),
        ["code", "error", "message"],
        "error body must stay {{code, error, message}}: {body}"
    );
}

fn assert_users_read_denial(status: StatusCode, body: &Value) {
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_error_body(body);
    assert_eq!(body["code"], "authorization_failed", "{body}");
    assert_eq!(body["error"], "Forbidden", "{body}");
    assert_eq!(
        body["message"], "Insufficient rights to read users",
        "{body}"
    );
}

fn assert_minimized_user(body: &Value, user_id: i64, card: &str) {
    let user = &body["user"];
    assert_eq!(user["id"], user_id.to_string(), "{user}");
    assert_eq!(user["cardNumber"], card, "{user}");
    assert_eq!(user["initials"], "IM", "{user}");
    assert_eq!(
        sorted_keys(user),
        ["cardNumber", "id", "initials"],
        "loan-rights-only identity is exactly id, cardNumber, initials: {user}"
    );
}

const CONTACT_KEYS: &[&str] = &[
    "firstname",
    "lastname",
    "email",
    "phone",
    "login",
    "addrStreet",
    "addrCity",
    "address",
    "birthdate",
];

fn assert_no_contact_fields(body: &Value) {
    fn walk(value: &Value, path: &str) {
        match value {
            Value::Object(map) => {
                for key in CONTACT_KEYS {
                    assert!(
                        !map.contains_key(*key),
                        "contact field {key} at {path}: {value}"
                    );
                }
                for (key, child) in map {
                    walk(child, &format!("{path}.{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    walk(body, "$");
}

fn assert_no_personal_text(body: &Value, forbidden: &[&str]) {
    let text = body.to_string();
    for needle in forbidden {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

struct PatronLoans {
    reader_id: i64,
    card: String,
    login: String,
    email: String,
    open_loan_id: i64,
    open_barcode: String,
    archived_barcode: String,
}

async fn seed(app: &TestApp, admin_token: &str) -> PatronLoans {
    let (reader_id, _) = fixtures::create_reader(app, admin_token, "deskloan").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    let email = format!("isolde.desk.{card}@secret.test");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Isolde',
            lastname = 'Moreau',
            email = $2,
            phone = '0612345678',
            addr_street = '12 Rue de la Paix',
            addr_city = 'Lyon',
            barcode = $3
        WHERE id = $1
        "#,
    )
    .bind(reader_id)
    .bind(&email)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set patron contact");

    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");

    let (archived_loan_id, archived_barcode) = checkout(app, admin_token, reader_id, "ARCH").await;
    let (status, returned) = app
        .post_empty(
            &format!("/api/v1/loans/{archived_loan_id}/return"),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {returned}");

    let (open_loan_id, open_barcode) = checkout(app, admin_token, reader_id, "OPEN").await;
    PatronLoans {
        reader_id,
        card,
        login,
        email,
        open_loan_id,
        open_barcode,
        archived_barcode,
    }
}

async fn checkout(app: &TestApp, admin_token: &str, reader_id: i64, tag: &str) -> (i64, String) {
    let suffix = fixtures::unique_suffix();
    let barcode = format!("{tag}-{suffix}");
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Desk copy {tag} {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio}");
    let item_id = fixtures::json_id(&biblio["biblio"]["items"][0]["id"]);
    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    (fixtures::json_id(&created["id"]), barcode)
}

fn personal(patron: &PatronLoans) -> Vec<&str> {
    vec![
        "Isolde",
        "Moreau",
        patron.email.as_str(),
        "0612345678",
        "12 Rue de la Paix",
        "Lyon",
        patron.login.as_str(),
    ]
}

fn row_ids(body: &Value) -> Vec<i64> {
    body["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items: {body}"))
        .iter()
        .map(|row| fixtures::json_id(&row["id"]))
        .collect()
}

#[tokio::test]
async fn loan_rights_only_sees_current_loans_with_minimal_identity() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let patron = seed(&app, &admin_token).await;
    let path = format!("/api/v1/users/{}/loans", patron.reader_id);
    let leaked = personal(&patron);

    for loans in [Rights::Read, Rights::Write] {
        for users in [Rights::None, Rights::Own] {
            let token = retarget(&admin_token, &secret, users, loans);
            let (status, body) = app.get_json_with_auth(&path, &token).await;
            assert_eq!(status, StatusCode::OK, "{users:?}/{loans:?} {body}");
            let ids = row_ids(&body);
            assert!(
                ids.contains(&patron.open_loan_id),
                "current loan missing: {body}"
            );
            assert!(
                body["items"].as_array().is_some_and(|rows| rows
                    .iter()
                    .all(|row| row["itemIdentification"] != patron.archived_barcode)),
                "archived loan leaked into the current list: {body}"
            );
            for row in body["items"].as_array().expect("items") {
                assert_minimized_user(row, patron.reader_id, &patron.card);
            }
            assert_no_contact_fields(&body);
            assert_no_personal_text(&body, &leaked);
        }
    }
}

#[tokio::test]
async fn archived_loans_stay_forbidden_for_loan_rights_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let patron = seed(&app, &admin_token).await;
    let archived = format!("/api/v1/users/{}/loans?archived=true", patron.reader_id);
    let current = format!("/api/v1/users/{}/loans", patron.reader_id);

    let no_rights = retarget(&admin_token, &secret, Rights::None, Rights::None);
    let (baseline_status, baseline) = app.get_json_with_auth(&archived, &no_rights).await;
    assert_users_read_denial(baseline_status, &baseline);

    let (current_status, current_body) = app.get_json_with_auth(&current, &no_rights).await;
    assert_users_read_denial(current_status, &current_body);
    assert_eq!(
        current_body, baseline,
        "same denial for current and archived"
    );

    for loans in [Rights::Read, Rights::Write] {
        let token = retarget(&admin_token, &secret, Rights::None, loans);
        let (status, body) = app.get_json_with_auth(&archived, &token).await;
        assert_users_read_denial(status, &body);
        assert_eq!(
            body, baseline,
            "loan-rights-only archived denial matches the users-read denial"
        );
    }
}

#[tokio::test]
async fn users_read_loan_list_is_unchanged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let patron = seed(&app, &admin_token).await;
    let current = format!("/api/v1/users/{}/loans", patron.reader_id);
    let archived = format!("/api/v1/users/{}/loans?archived=true", patron.reader_id);

    let users_only = retarget(&admin_token, &secret, Rights::Read, Rights::None);
    let (status, body) = app.get_json_with_auth(&current, &users_only).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_error_body(&body);
    assert_eq!(body["code"], "authorization_failed", "{body}");
    assert_eq!(body["error"], "Forbidden", "{body}");
    assert_eq!(
        body["message"], "Insufficient rights to read loans for another user",
        "{body}"
    );

    for users in [Rights::Read, Rights::Write] {
        let token = retarget(&admin_token, &secret, users, Rights::Read);
        let (status, body) = app.get_json_with_auth(&current, &token).await;
        assert_eq!(status, StatusCode::OK, "{users:?} current: {body}");
        let row = body["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|row| fixtures::json_id(&row["id"]) == patron.open_loan_id)
            .unwrap_or_else(|| panic!("open loan missing: {body}"));
        let user = &row["user"];
        assert_eq!(user["id"], patron.reader_id.to_string(), "{user}");
        assert_eq!(user["firstname"], "Isolde", "{user}");
        assert_eq!(user["lastname"], "Moreau", "{user}");
        assert_eq!(user["login"], patron.login, "{user}");
        assert_eq!(user["barcode"], patron.card, "{user}");
        assert!(user.get("cardNumber").is_none(), "{user}");
        assert!(user.get("initials").is_none(), "{user}");
        assert!(user.get("email").is_none(), "{user}");
        assert!(user.get("phone").is_none(), "{user}");

        let (status, history) = app.get_json_with_auth(&archived, &token).await;
        assert_eq!(status, StatusCode::OK, "{users:?} archived: {history}");
        assert!(
            history["items"].as_array().is_some_and(|rows| {
                rows.iter()
                    .any(|row| row["itemIdentification"] == patron.archived_barcode)
            }),
            "users read still lists archived loans: {history}"
        );
        assert!(
            history["items"].as_array().is_some_and(|rows| {
                rows.iter()
                    .all(|row| row["itemIdentification"] != patron.open_barcode)
            }),
            "open loan is not history: {history}"
        );
    }
}

#[tokio::test]
async fn no_loan_or_users_rights_is_refused() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let patron = seed(&app, &admin_token).await;
    let current = format!("/api/v1/users/{}/loans", patron.reader_id);

    for (users, loans) in [
        (Rights::None, Rights::None),
        (Rights::None, Rights::Own),
        (Rights::Own, Rights::None),
        (Rights::Own, Rights::Own),
    ] {
        let token = retarget(&admin_token, &secret, users, loans);
        let (status, body) = app.get_json_with_auth(&current, &token).await;
        assert_users_read_denial(status, &body);
    }
}
