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
    let mut keys: Vec<&str> = body
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["cardNumber", "id", "initials"],
        "restricted shape must be exactly id, cardNumber, initials: {body}"
    );
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

    let personal = [
        "Isolde",
        "Moreau",
        "isolde.moreau@secret.test",
        "0612345678",
        "12 Rue de la Paix",
        login.as_str(),
    ];

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
