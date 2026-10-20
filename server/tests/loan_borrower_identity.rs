//! `GET /loans/:id/user` returns a staff profile only when the caller can read users.

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

fn assert_minimized(body: &Value, user_id: i64, card: Option<&str>, initials: &str) {
    assert_eq!(body["id"], user_id.to_string(), "{body}");
    assert_eq!(body["initials"], initials, "{body}");
    match card {
        Some(number) => assert_eq!(body["cardNumber"], number, "{body}"),
        None => assert!(body["cardNumber"].is_null(), "{body}"),
    }
    let mut keys: Vec<&str> = body.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["cardNumber", "id", "initials"], "restricted shape must be exactly id, cardNumber, initials: {body}");
}

fn assert_no_personal_text(body: &Value, forbidden: &[&str]) {
    let text = body.to_string();
    for needle in forbidden {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

#[tokio::test]
async fn loan_borrower_identity_follows_users_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "borrower_id").await;

    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Isolde',
            lastname = 'Moreau',
            email = 'isolde.moreau@secret.test',
            phone = '0612345678',
            addr_street = '12 Rue de la Paix',
            barcode = 'CARD-ISOLDE'
        WHERE id = $1
        "#,
    )
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set borrower identity");

    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");

    let suffix = fixtures::unique_suffix();
    let barcode = format!("BORROWER-{suffix}");
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Borrower identity {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio}");
    let item_id = fixtures::json_id(&biblio["biblio"]["items"][0]["id"]);

    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string(),
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);
    let path = format!("/api/v1/loans/{loan_id}/user");

    let personal = ["Isolde", "Moreau", "isolde.moreau@secret.test", "0612345678", "12 Rue de la Paix", login.as_str()];

    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let (status, body) = app.get_json_with_auth(&path, &loans_only).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_minimized(&body, reader_id, Some("CARD-ISOLDE"), "IM");
    assert_no_personal_text(&body, &personal);

    let users_own = retarget(&admin_token, &secret, Rights::Own, Rights::Write);
    let (status, body) = app.get_json_with_auth(&path, &users_own).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_minimized(&body, reader_id, Some("CARD-ISOLDE"), "IM");
    assert_no_personal_text(&body, &personal);

    for users_rights in [Rights::Read, Rights::Write] {
        let token = retarget(&admin_token, &secret, users_rights, Rights::Write);
        let (status, body) = app.get_json_with_auth(&path, &token).await;
        assert_eq!(status, StatusCode::OK, "{users_rights:?} {body}");
        assert_eq!(body["id"], reader_id.to_string(), "{body}");
        assert_eq!(body["firstname"], "Isolde", "{body}");
        assert_eq!(body["lastname"], "Moreau", "{body}");
        assert_eq!(body["login"], login, "{body}");
        assert_eq!(body["barcode"], "CARD-ISOLDE", "{body}");
        assert!(body.get("cardNumber").is_none(), "{body}");
        assert!(body.get("initials").is_none(), "{body}");
        assert!(body.get("email").is_none(), "{body}");
        assert!(body.get("phone").is_none(), "{body}");
    }

    let users_read_only = retarget(&admin_token, &secret, Rights::Read, Rights::None);
    let (status, body) = app.get_json_with_auth(&path, &users_read_only).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    sqlx::query("UPDATE users SET barcode = NULL WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("clear card");

    let (status, body) = app.get_json_with_auth(&path, &loans_only).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_minimized(&body, reader_id, None, "IM");
    assert_no_personal_text(&body, &personal);

    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Write);
    let (status, body) = app.get_json_with_auth(&path, &users_read).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["barcode"].is_null(), "cleared card stays null: {body}");
}

fn assert_no_identity_keys(body: &Value) {
    const FORBIDDEN: &[&str] = &["firstname", "lastname", "email", "userEmail", "phone", "login"];
    fn walk(value: &Value, path: &str) {
        match value {
            Value::Object(map) => {
                for key in FORBIDDEN {
                    assert!(!map.contains_key(*key), "identity key {key} at {path}: {value}");
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

async fn name_reader(app: &TestApp, reader_id: i64, card: &str, email: &str) {
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Isolde',
            lastname = 'Moreau',
            email = $2,
            phone = '0612345678',
            barcode = $3
        WHERE id = $1
        "#,
    )
    .bind(reader_id)
    .bind(email)
    .bind(card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set borrower identity");
}

async fn checkout(app: &TestApp, admin_token: &str, reader_id: i64, tag: &str) -> (i64, String) {
    let suffix = fixtures::unique_suffix();
    let barcode = format!("{tag}-{suffix}");
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("{tag} {suffix}"),
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
                "itemId": item_id.to_string(),
                "force": false
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    (fixtures::json_id(&created["id"]), barcode)
}

fn assert_staff_profile(body: &Value, reader_id: i64, login: &str, card: &str) {
    assert_eq!(body["id"], reader_id.to_string(), "{body}");
    assert_eq!(body["firstname"], "Isolde", "{body}");
    assert_eq!(body["lastname"], "Moreau", "{body}");
    assert_eq!(body["login"], login, "{body}");
    assert_eq!(body["barcode"], card, "{body}");
    assert!(body.get("cardNumber").is_none(), "{body}");
    assert!(body.get("initials").is_none(), "{body}");
    assert!(body.get("email").is_none(), "{body}");
    assert!(body.get("phone").is_none(), "{body}");
}

#[tokio::test]
async fn return_routes_follow_users_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "return_id").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.return@secret.test").await;
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Write);
    let personal = ["Isolde", "Moreau", "isolde.return@secret.test", "0612345678", login.as_str()];

    let (loan_id, _) = checkout(&app, &admin_token, reader_id, "RETID").await;
    let (status, body) = app.post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&loans_only)).await;
    assert_eq!(status, StatusCode::OK, "return by id: {body}");
    assert_minimized(&body["loan"]["user"], reader_id, Some(&card), "IM");
    assert_no_personal_text(&body, &personal);

    let (loan_id, _) = checkout(&app, &admin_token, reader_id, "RETIDFULL").await;
    let (status, body) = app.post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&users_read)).await;
    assert_eq!(status, StatusCode::OK, "return by id full: {body}");
    assert_staff_profile(&body["loan"]["user"], reader_id, &login, &card);

    let (_, barcode) = checkout(&app, &admin_token, reader_id, "RETITEM").await;
    let (status, body) = app.post_empty(&format!("/api/v1/loans/items/{barcode}/return"), Some(&loans_only)).await;
    assert_eq!(status, StatusCode::OK, "return by item: {body}");
    assert_minimized(&body["loan"]["user"], reader_id, Some(&card), "IM");
    assert_no_personal_text(&body, &personal);

    let (_, barcode) = checkout(&app, &admin_token, reader_id, "RETITEMFULL").await;
    let (status, body) = app.post_empty(&format!("/api/v1/loans/items/{barcode}/return"), Some(&users_read)).await;
    assert_eq!(status, StatusCode::OK, "return by item full: {body}");
    assert_staff_profile(&body["loan"]["user"], reader_id, &login, &card);

    let (_, barcode) = checkout(&app, &admin_token, reader_id, "RETBATCH").await;
    let (status, body) = app.post_json("/api/v1/loans/batch-return", &json!({ "barcodes": [barcode] }), Some(&loans_only)).await;
    assert_eq!(status, StatusCode::OK, "batch return: {body}");
    assert_eq!(body["results"][0]["success"], true, "{body}");
    assert_minimized(&body["results"][0]["loan"]["user"], reader_id, Some(&card), "IM");
    assert_no_personal_text(&body, &personal);

    let (_, barcode) = checkout(&app, &admin_token, reader_id, "RETBATCHFULL").await;
    let (status, body) = app.post_json("/api/v1/loans/batch-return", &json!({ "barcodes": [barcode] }), Some(&users_read)).await;
    assert_eq!(status, StatusCode::OK, "batch return full: {body}");
    assert_staff_profile(&body["results"][0]["loan"]["user"], reader_id, &login, &card);
}

async fn overdue_row(app: &TestApp, token: &str, loan_id: i64) -> Value {
    app.find_overdue_loan(token, loan_id).await
}

#[tokio::test]
async fn overdue_without_users_read_is_minimal() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "overdue_min").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.overdue@secret.test").await;
    let (loan_id, _) = checkout(&app, &admin_token, reader_id, "OVERMIN").await;
    sqlx::query("UPDATE loans SET expiry_at = '1970-01-02T00:00:00Z' WHERE id = $1")
        .bind(loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backdate due date");

    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let row = overdue_row(&app, &loans_only, loan_id).await;
    assert_minimized(&row["user"], reader_id, Some(&card), "IM");
    assert_no_identity_keys(&row);
    assert_no_personal_text(&row, &["Isolde", "Moreau", "isolde.overdue@secret.test", "0612345678"]);
    assert!(row.get("userId").is_none(), "{row}");
    assert!(row.get("firstname").is_none(), "{row}");
    assert!(row.get("userEmail").is_none(), "{row}");
}

#[tokio::test]
async fn overdue_with_users_read_includes_phone() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "overdue_full").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.phone@secret.test").await;
    let (loan_id, _) = checkout(&app, &admin_token, reader_id, "OVERFULL").await;
    sqlx::query("UPDATE loans SET expiry_at = '1970-01-02T00:00:00Z' WHERE id = $1")
        .bind(loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backdate due date");

    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Read);
    let row = overdue_row(&app, &users_read, loan_id).await;
    let user = &row["user"];
    assert_eq!(user["id"], reader_id.to_string(), "{user}");
    assert_eq!(user["firstname"], "Isolde", "{user}");
    assert_eq!(user["lastname"], "Moreau", "{user}");
    assert_eq!(user["email"], "isolde.phone@secret.test", "{user}");
    assert_eq!(user["phone"], "0612345678", "{user}");
    assert!(user.get("cardNumber").is_none(), "{user}");
    assert!(user.get("initials").is_none(), "{user}");
    assert!(user.get("login").is_none(), "{user}");
}

#[tokio::test]
async fn user_loans_follow_users_rights_not_account_type() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "uloans").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "uloans_other").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.loans@secret.test").await;
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");
    checkout(&app, &admin_token, reader_id, "ULOAN").await;
    checkout(&app, &admin_token, other_id, "ULOANOTHER").await;

    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &loans_only).await;
    assert_eq!(status, StatusCode::OK, "loans read without users read sees current loans: {body}");
    assert_minimized(&body["items"][0]["user"], reader_id, Some(&card), "IM");
    assert_no_personal_text(&body, &["Isolde", "Moreau", "isolde.loans@secret.test", login.as_str()]);

    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Read);
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &users_read).await;
    assert_eq!(status, StatusCode::OK, "users read: {body}");
    let row = &body["items"][0];
    assert_staff_profile(&row["user"], reader_id, &login, &card);

    let (status, body) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "patron own loans: {body}");
    assert_minimized(&body["items"][0]["user"], reader_id, Some(&card), "IM");
    assert_no_personal_text(&body, &["Isolde", "Moreau", "isolde.loans@secret.test", login.as_str()]);

    let (status, denied) = app.get_json_with_auth(&format!("/api/v1/users/{other_id}/loans"), &reader_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "patron other loans: {denied}");
}

#[tokio::test]
async fn patron_reads_own_loan_borrower_and_not_another() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ownuser").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "ownother").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.own@secret.test").await;
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");
    let (own_loan, _) = checkout(&app, &admin_token, reader_id, "OWNLOAN").await;
    let (other_loan, _) = checkout(&app, &admin_token, other_id, "OTHERLOAN").await;

    let (status, body) = app.get_json_with_auth(&format!("/api/v1/loans/{own_loan}/user"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "own borrower: {body}");
    assert_staff_profile(&body, reader_id, &login, &card);

    let (status, denied) = app.get_json_with_auth(&format!("/api/v1/loans/{other_loan}/user"), &reader_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "other borrower: {denied}");
}

#[tokio::test]
async fn stats_users_omits_named_borrowers() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "statsname").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    name_reader(&app, reader_id, &card, "isolde.stats@secret.test").await;
    checkout(&app, &admin_token, reader_id, "STATS").await;

    for path in ["/api/v1/stats/users", "/api/v1/stats/users?mode=leaderboard&sortBy=totalLoans&limit=50"] {
        for token in [
            admin_token.clone(),
            retarget(&admin_token, &secret, Rights::None, Rights::Read),
            retarget(&admin_token, &secret, Rights::Read, Rights::Read),
        ] {
            let (status, body) = app.get_json_with_auth(path, &token).await;
            assert_eq!(status, StatusCode::OK, "{path} {body}");
            assert!(body.get("mode").is_none(), "{path} {body}");
            assert!(body.get("users").is_none(), "{path} {body}");
            assert!(body["usersTotal"].as_i64().unwrap_or(0) >= 1, "{body}");
            assert!(body["usersByPublicType"].is_array(), "{body}");
            assert!(body["usersByAgeBand"].is_array(), "{body}");
            assert!(body["usersByAccountType"].is_array(), "{body}");
            assert!(body.get("usersBySite").is_none(), "{body}");
            assert_no_identity_keys(&body);
            assert_no_personal_text(&body, &["Isolde", "Moreau", "isolde.stats@secret.test", "0612345678"]);
        }
    }
}
