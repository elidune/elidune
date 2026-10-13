//! #162: a known copy that is not on loan is an informational check-in.

mod common;

use std::time::Duration;

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

fn assert_notice(body: &Value) {
    assert_eq!(body["notice"]["code"], "item_not_on_loan", "{body}");
    assert_eq!(body["notice"]["level"], "informational", "{body}");
    assert!(
        body.get("code").is_none(),
        "notice is not an error body: {body}"
    );
    assert!(body.get("error").is_none(), "{body}");
}

fn assert_minimized(body: &Value, user_id: i64, card: &str) {
    assert_eq!(body["id"], user_id.to_string(), "{body}");
    assert_eq!(body["cardNumber"], card, "{body}");
    assert_eq!(body["initials"], "HH", "{body}");
    let mut keys: Vec<&str> = body
        .as_object()
        .expect("holdFor")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["cardNumber", "id", "initials"], "{body}");
}

fn assert_hidden(body: &Value, needles: &[&str]) {
    let text = body.to_string();
    for needle in needles {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

fn copy_by_barcode<'a>(body: &'a Value, barcode: &str) -> &'a Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["barcode"].as_str() == Some(barcode))
        .unwrap_or_else(|| panic!("copy {barcode} missing from {body}"))
}

async fn wait_audit(app: &TestApp, item_id: i64) -> Value {
    let pool = app.state.services.repository.pool();
    for _ in 0..40 {
        let row: Option<(String, Value)> = sqlx::query_as(
            r#"
            SELECT outcome, payload
            FROM audit_log
            WHERE event_type = 'item.not_on_loan' AND entity_type = 'item' AND entity_id = $1
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(item_id)
        .fetch_optional(pool)
        .await
        .expect("audit query");
        if let Some((outcome, payload)) = row {
            assert_eq!(outcome, "success");
            return payload;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("item.not_on_loan audit row missing for item {item_id}");
}

#[tokio::test]
async fn on_shelf_return_without_hold_is_informational() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let barcode = format!("SHELF-{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("On shelf {barcode}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "available", "{body}");
    assert_notice(&body);
    assert!(body["loan"].is_null(), "{body}");
    assert!(body["hold"].is_null(), "{body}");
    assert!(body.get("holdFor").is_none(), "{body}");

    let (status, biblio) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let copy = copy_by_barcode(&biblio, &barcode);
    assert_eq!(copy["availability"], "available", "{copy}");
    assert_eq!(copy["onHold"], false, "{copy}");

    let payload = wait_audit(&app, item_id).await;
    assert_eq!(payload["notice"], "item_not_on_loan", "{payload}");
    assert_eq!(payload["barcode"], barcode, "{payload}");
    assert!(payload["hold_id"].is_null(), "{payload}");
    assert_eq!(payload["transit_started"], false, "{payload}");
}

#[tokio::test]
async fn on_shelf_return_with_pending_hold_sets_copy_aside() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Write);

    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "shelf_bor").await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "shelf_hold").await;
    let card = format!("CARD-{holder_id}");
    let email = format!("helene.holder.{holder_id}@secret.test");
    let phone = format!("06{holder_id}");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Helene', lastname = 'Holder', email = $2, phone = $3, barcode = $4
        WHERE id = $1
        "#,
    )
    .bind(holder_id)
    .bind(&email)
    .bind(&phone)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("holder identity");
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(holder_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("holder login");
    let needles = [
        "Helene",
        "Holder",
        email.as_str(),
        phone.as_str(),
        login.as_str(),
    ];

    let loaned_barcode = format!("OUT-{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Waiting hold {loaned_barcode}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": loaned_barcode, "borrowable": true }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let loaned_item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": borrower_id.to_string(),
                "itemId": loaned_item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": holder_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&holder_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{hold}");
    assert_eq!(hold["status"], "pending", "{hold}");
    assert!(hold["itemId"].is_null(), "no shelf copy yet: {hold}");
    let hold_id = fixtures::json_id(&hold["id"]);

    let shelf_barcode = format!("IN-{}", fixtures::unique_suffix());
    let (status, shelf) = app
        .post_json(
            &format!("/api/v1/biblios/{biblio_id}/items"),
            &json!({ "barcode": shelf_barcode, "borrowable": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{shelf}");
    let shelf_item_id = fixtures::json_id(&shelf["id"]);

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{shelf_barcode}/return"),
            Some(&loans_only),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ready", "{body}");
    assert_notice(&body);
    assert!(body["loan"].is_null(), "{body}");
    assert_eq!(body["hold"]["status"], "ready", "{body}");
    assert_eq!(fixtures::json_id(&body["hold"]["id"]), hold_id, "{body}");
    assert_eq!(
        fixtures::json_id(&body["hold"]["itemId"]),
        shelf_item_id,
        "{body}"
    );
    assert!(body["hold"]["notifiedAt"].is_string(), "{body}");
    assert_minimized(&body["holdFor"], holder_id, &card);
    assert_hidden(&body, &needles);

    let still_out: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM loans WHERE item_id = $1 AND returned_at IS NULL",
    )
    .bind(loaned_item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("open loan");
    assert_eq!(still_out, 1, "the copy that is out stays on loan");

    let (status, biblio) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &loans_only)
        .await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let copy = copy_by_barcode(&biblio, &shelf_barcode);
    assert_eq!(copy["onHold"], true, "{copy}");
    assert_minimized(&copy["holdFor"], holder_id, &card);
    assert_hidden(&biblio, &needles);

    let (status, full) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &users_read)
        .await;
    assert_eq!(status, StatusCode::OK, "{full}");
    let full_copy = copy_by_barcode(&full, &shelf_barcode);
    assert_eq!(full_copy["holdFor"]["login"], login, "{full_copy}");
    assert_eq!(full_copy["holdFor"]["firstname"], "Helene", "{full_copy}");
    assert!(full_copy["holdFor"].get("email").is_none(), "{full_copy}");
    assert!(full_copy["holdFor"].get("phone").is_none(), "{full_copy}");

    let payload = wait_audit(&app, shelf_item_id).await;
    assert_eq!(payload["notice"], "item_not_on_loan", "{payload}");
    assert_eq!(payload["barcode"], shelf_barcode, "{payload}");
    assert_eq!(payload["hold_id"].as_i64(), Some(hold_id), "{payload}");
}
