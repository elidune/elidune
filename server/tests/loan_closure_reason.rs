//! Closed loans record `closureReason` and `foundAt` (#157).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

async fn admin(app: &TestApp) -> String {
    let token = fixtures::ensure_first_setup(app).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    if status != StatusCode::OK {
        return token;
    }
    let admin_id = fixtures::json_id(&me["id"]);
    let _ = app
        .state
        .services
        .users
        .set_must_change_password(admin_id, false)
        .await;
    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": "testadmin", "password": "testadmin1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "admin login: {body}");
    body["token"].as_str().expect("token").to_string()
}

async fn checkout(app: &TestApp, token: &str, reader_id: i64) -> (i64, String) {
    let barcode = format!("CR-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Closure {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true, "price": "12.00" }]
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({ "userId": reader_id.to_string(), "itemId": item_id.to_string() }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    (fixtures::json_id(&loan["id"]), barcode)
}

fn row_by_barcode<'a>(items: &'a [Value], barcode: &str) -> &'a Value {
    items
        .iter()
        .find(|row| {
            row["barcode"] == barcode
                || row["itemIdentification"] == barcode
                || row["biblio"]["items"]
                    .as_array()
                    .is_some_and(|copies| copies.iter().any(|copy| copy["barcode"] == barcode))
        })
        .unwrap_or_else(|| panic!("barcode {barcode} missing from {items:?}"))
}

async fn archived_rows(app: &TestApp, token: &str, path: &str) -> Vec<Value> {
    let (status, body) = app.get_json_with_auth(path, token).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body}");
    body["items"]
        .as_array()
        .unwrap_or_else(|| panic!("items: {body}"))
        .clone()
}

fn assert_closure(row: &Value, reason: &str, found: bool) {
    assert_eq!(row["closureReason"], reason, "{row}");
    if found {
        assert!(
            row["foundAt"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "foundAt must be a timestamp: {row}"
        );
    } else {
        assert!(row["foundAt"].is_null(), "foundAt must be null: {row}");
    }
}

async fn assert_both_endpoints(
    app: &TestApp,
    staff_token: &str,
    reader_id: i64,
    reader_token: &str,
    barcode: &str,
    reason: &str,
    found: bool,
) {
    let staff = archived_rows(
        app,
        staff_token,
        &format!("/api/v1/users/{reader_id}/loans?archived=true"),
    )
    .await;
    assert_closure(row_by_barcode(&staff, barcode), reason, found);

    let mine = archived_rows(app, reader_token, "/api/v1/me/loans?archived=true").await;
    assert_closure(row_by_barcode(&mine, barcode), reason, found);

    let open = archived_rows(app, reader_token, "/api/v1/me/loans").await;
    assert!(
        open.iter()
            .all(|row| row.get("closureReason").is_none() && row.get("foundAt").is_none()),
        "open /me/loans must omit closure fields: {open:?}"
    );
    assert!(
        open.iter().all(|row| row["barcode"] != barcode),
        "open /me/loans must not include the closed loan: {open:?}"
    );
}

#[tokio::test]
async fn return_archives_closure_reason_returned() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &token, "crret").await;
    let (other_id, other_token) = fixtures::create_reader(&app, &token, "croth").await;
    let (loan_id, barcode) = checkout(&app, &token, reader_id).await;
    let _ = other_id;

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");

    assert_both_endpoints(
        &app,
        &token,
        reader_id,
        &reader_token,
        &barcode,
        "returned",
        false,
    )
    .await;

    let other = archived_rows(&app, &other_token, "/api/v1/me/loans?archived=true").await;
    assert!(
        other.iter().all(|row| row["barcode"] != barcode),
        "another patron must not see this closed loan: {other:?}"
    );
}

#[tokio::test]
async fn declared_lost_archives_closure_reason_lost() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &token, "crlost").await;
    let (loan_id, barcode) = checkout(&app, &token, reader_id).await;

    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": false, "notes": "declared lost" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {body}");
    assert_eq!(body["loanClosed"], true, "{body}");

    assert_both_endpoints(
        &app,
        &token,
        reader_id,
        &reader_token,
        &barcode,
        "lost",
        false,
    )
    .await;
}

#[tokio::test]
async fn lost_then_found_keeps_lost_and_sets_found_at() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &token, "crfound").await;
    let (loan_id, barcode) = checkout(&app, &token, reader_id).await;

    let (status, lost) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true, "notes": "declared lost" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {lost}");
    let item_id = fixtures::json_id(&lost["itemId"]);

    let (status, left) = app
        .post_json(
            &format!("/api/v1/loans/items/{item_id}/lost-found"),
            &json!({ "disposition": "leave" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "leave lost: {left}");
    assert_both_endpoints(
        &app,
        &token,
        reader_id,
        &reader_token,
        &barcode,
        "lost",
        false,
    )
    .await;

    let (status, found) = app
        .post_json(
            &format!("/api/v1/loans/items/{item_id}/lost-found"),
            &json!({ "disposition": "circulate" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "found: {found}");
    assert_both_endpoints(
        &app,
        &token,
        reader_id,
        &reader_token,
        &barcode,
        "lost",
        true,
    )
    .await;
}

#[tokio::test]
async fn me_loans_archived_lists_only_the_callers_closed_loans() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (owner_id, owner_token) = fixtures::create_reader(&app, &token, "crown").await;
    let (_other_id, other_token) = fixtures::create_reader(&app, &token, "crpeer").await;
    let (loan_id, barcode) = checkout(&app, &token, owner_id).await;
    let (open_loan_id, open_barcode) = checkout(&app, &token, owner_id).await;
    let _ = open_loan_id;

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");

    let mine = archived_rows(&app, &owner_token, "/api/v1/me/loans?archived=true").await;
    assert_closure(row_by_barcode(&mine, &barcode), "returned", false);
    assert!(
        mine.iter().all(|row| row["barcode"] != open_barcode),
        "archived /me/loans must not include the open loan: {mine:?}"
    );

    let open = archived_rows(&app, &owner_token, "/api/v1/me/loans").await;
    assert!(row_by_barcode(&open, &open_barcode)
        .get("closureReason")
        .is_none());
    assert!(open.iter().all(|row| row["barcode"] != barcode));

    let other = archived_rows(&app, &other_token, "/api/v1/me/loans?archived=true").await;
    assert!(
        other
            .iter()
            .all(|row| row["barcode"] != barcode && row["barcode"] != open_barcode),
        "peer archived loans: {other:?}"
    );

    let (status, denied) = app
        .get_json_with_auth(
            &format!("/api/v1/users/{owner_id}/loans?archived=true"),
            &other_token,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "other patron staff history: {denied}"
    );
}
