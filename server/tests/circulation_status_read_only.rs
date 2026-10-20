//! `PUT /items/:id` must not write `circulationStatus` (#200).
//!
//! A body that includes the key, with any JSON value, is rejected with 422
//! `circulation_status_read_only` and no other column is updated. Omitting the
//! field still updates the copy.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

type ItemSnapshot = (Option<String>, Option<String>, Option<i16>, bool);

async fn create_copy(app: &TestApp, admin_token: &str) -> i64 {
    let suffix = fixtures::unique_suffix();
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Read-only circulation status {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("RO-{suffix}"),
                    "callNumber": "SHELF-A",
                    "internalNote": "original",
                    "borrowable": true
                }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn mark_to_search(app: &TestApp, item_id: i64) {
    sqlx::query("UPDATE items SET circulation_status = 4, borrowable = FALSE WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark copy to-search");
}

async fn snapshot(app: &TestApp, item_id: i64) -> ItemSnapshot {
    sqlx::query_as("SELECT call_number, internal_note, circulation_status, borrowable FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("read item")
}

fn assert_read_only_body(body: &Value) {
    let mut keys: Vec<&str> = body.as_object().unwrap_or_else(|| panic!("error object: {body}")).keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "error", "message"], "{body}");
    assert_eq!(body["code"], "circulation_status_read_only", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    let message = body["message"].as_str().unwrap_or_else(|| panic!("{body}"));
    let lower = message.to_ascii_lowercase();
    assert!(lower.contains("lost"), "{message}");
    assert!(lower.contains("missing"), "{message}");
    assert!(lower.contains("to-search"), "{message}");
}

#[tokio::test]
async fn put_with_circulation_status_is_422_and_writes_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let item_id = create_copy(&app, &admin_token).await;
    mark_to_search(&app, item_id).await;
    let before = snapshot(&app, item_id).await;
    assert_eq!(before.2, Some(4));

    let cases = [("integer", json!(1)), ("string", json!("lost")), ("null", json!(null)), ("object", json!({"status": "lost"}))];
    for (label, circulation_status) in cases {
        let (status, body) = app
            .put_json(
                &format!("/api/v1/items/{item_id}"),
                &json!({
                    "callNumber": "SHELF-B",
                    "internalNote": "changed",
                    "borrowable": true,
                    "circulationStatus": circulation_status,
                }),
                Some(&admin_token),
            )
            .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{label}: {body}");
        assert_read_only_body(&body);
        assert_eq!(snapshot(&app, item_id).await, before, "{label}: circulationStatus on PUT must not apply any other field");
    }
}

#[tokio::test]
async fn put_without_circulation_status_still_updates_other_fields() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let item_id = create_copy(&app, &admin_token).await;
    mark_to_search(&app, item_id).await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/items/{item_id}"),
            &json!({
                "callNumber": "SHELF-B",
                "internalNote": "changed"
            }),
            Some(&admin_token),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.get("circulationStatus").is_none() || body["circulationStatus"].is_null(),
        "omitted circulationStatus stays absent from the write contract: {body}"
    );
    let (call_number, notes, circulation_status, _) = snapshot(&app, item_id).await;
    assert_eq!(call_number.as_deref(), Some("SHELF-B"));
    assert_eq!(notes.as_deref(), Some("changed"));
    assert_eq!(circulation_status, Some(4));
}
