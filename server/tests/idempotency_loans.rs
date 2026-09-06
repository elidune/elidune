//! HTTP-level Idempotency-Key tests for checkout and renew.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

async fn seed_checkout_item(app: &TestApp, admin_token: &str) -> (i64, i64, String) {
    let (reader_id, _) = fixtures::create_reader(app, admin_token, "idemreader").await;
    let barcode = format!("IDEM-{}", fixtures::unique_suffix());
    let payload = json!({
        "title": "Idempotency Desk Book",
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": barcode,
            "borrowable": true
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (reader_id, item_id, barcode)
}

fn idem_headers(key: &str) -> [(&'static str, &str); 1] {
    [("Idempotency-Key", key)]
}

#[tokio::test]
async fn checkout_without_key_still_creates_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, item_id, _) = seed_checkout_item(&app, &admin_token).await;

    let payload = json!({
        "userId": reader_id.to_string(),
        "itemId": item_id.to_string()
    });
    let (status, body) = app
        .post_json("/api/v1/loans", &payload, Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    assert!(fixtures::json_id(&body["id"]) > 0);
}

#[tokio::test]
async fn checkout_replays_same_key_and_payload() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, item_id, _) = seed_checkout_item(&app, &admin_token).await;

    let payload = json!({
        "userId": reader_id.to_string(),
        "itemId": item_id.to_string()
    });
    let key = format!("checkout-{}", fixtures::unique_suffix());

    let (status1, body1) = app
        .post_json_with_headers(
            "/api/v1/loans",
            &payload,
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(status1, StatusCode::CREATED, "first checkout: {body1}");
    let loan_id = fixtures::json_id(&body1["id"]);

    let (status2, body2) = app
        .post_json_with_headers(
            "/api/v1/loans",
            &payload,
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(status2, StatusCode::CREATED, "replay checkout: {body2}");
    assert_eq!(fixtures::json_id(&body2["id"]), loan_id);
    assert_eq!(body2["message"], body1["message"]);

    let (list_status, list_body) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &admin_token)
        .await;
    assert_eq!(list_status, StatusCode::OK, "list loans: {list_body}");
    let items = list_body["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "replay must not create a second loan");
}

#[tokio::test]
async fn checkout_same_key_different_payload_conflicts() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, item_a, _) = seed_checkout_item(&app, &admin_token).await;
    let (reader_b, item_b, _) = seed_checkout_item(&app, &admin_token).await;

    let key = format!("conflict-{}", fixtures::unique_suffix());
    let first = json!({
        "userId": reader_a.to_string(),
        "itemId": item_a.to_string()
    });
    let second = json!({
        "userId": reader_b.to_string(),
        "itemId": item_b.to_string()
    });

    let (status1, body1) = app
        .post_json_with_headers(
            "/api/v1/loans",
            &first,
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(status1, StatusCode::CREATED, "first checkout: {body1}");

    let (status2, body2) = app
        .post_json_with_headers(
            "/api/v1/loans",
            &second,
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(status2, StatusCode::CONFLICT, "payload mismatch: {body2}");
    assert_eq!(body2["code"], "conflict");
    assert!(
        body2["message"]
            .as_str()
            .unwrap_or("")
            .contains("Idempotency-Key"),
        "conflict message: {body2}"
    );

    let (list_status, list_body) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_b}/loans"), &admin_token)
        .await;
    assert_eq!(list_status, StatusCode::OK, "list B: {list_body}");
    let items = list_body["items"].as_array().expect("items");
    assert!(items.is_empty(), "conflict must not check out item B");
}

#[tokio::test]
async fn checkout_invalid_key_is_validation_error() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, item_id, _) = seed_checkout_item(&app, &admin_token).await;
    let payload = json!({
        "userId": reader_id.to_string(),
        "itemId": item_id.to_string()
    });

    let (status, body) = app
        .post_json_with_headers(
            "/api/v1/loans",
            &payload,
            Some(&admin_token),
            &[("Idempotency-Key", "not a valid key")],
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "invalid key: {body}");
    assert_eq!(body["code"], "validation_error");
}

#[tokio::test]
async fn renew_replays_same_key_and_does_not_increment_twice() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, item_id, _) = seed_checkout_item(&app, &admin_token).await;
    let checkout = json!({
        "userId": reader_id.to_string(),
        "itemId": item_id.to_string()
    });
    let (status, body) = app
        .post_json("/api/v1/loans", &checkout, Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    let key = format!("renew-{}", fixtures::unique_suffix());
    let uri = format!("/api/v1/loans/{loan_id}/renew");

    let (status1, body1) = app
        .post_empty_with_headers(&uri, Some(&admin_token), &idem_headers(&key))
        .await;
    assert_eq!(status1, StatusCode::OK, "first renew: {body1}");
    let expiry1 = body1["expiryAt"].as_str().expect("expiryAt").to_string();
    let message1 = body1["message"].as_str().expect("message").to_string();

    let (status2, body2) = app
        .post_empty_with_headers(&uri, Some(&admin_token), &idem_headers(&key))
        .await;
    assert_eq!(status2, StatusCode::OK, "replay renew: {body2}");
    assert_eq!(body2["expiryAt"], expiry1);
    assert_eq!(body2["message"], message1);
    assert_eq!(fixtures::json_id(&body2["id"]), loan_id);

    let (list_status, list_body) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &admin_token)
        .await;
    assert_eq!(list_status, StatusCode::OK, "list: {list_body}");
    let nb_renews = list_body["items"][0]["nbRenews"].as_i64();
    assert_eq!(nb_renews, Some(1), "replay must not increment nb_renews");
}

#[tokio::test]
async fn renew_same_key_different_loan_conflicts() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, item_a, _) = seed_checkout_item(&app, &admin_token).await;
    let (reader_b, item_b, _) = seed_checkout_item(&app, &admin_token).await;

    let (s1, b1) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_a.to_string(),
                "itemId": item_a.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(s1, StatusCode::CREATED, "checkout A: {b1}");
    let loan_a = fixtures::json_id(&b1["id"]);

    let (s2, b2) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_b.to_string(),
                "itemId": item_b.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(s2, StatusCode::CREATED, "checkout B: {b2}");
    let loan_b = fixtures::json_id(&b2["id"]);

    let key = format!("renew-conflict-{}", fixtures::unique_suffix());
    let (r1, rb1) = app
        .post_empty_with_headers(
            &format!("/api/v1/loans/{loan_a}/renew"),
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(r1, StatusCode::OK, "renew A: {rb1}");

    let (r2, rb2) = app
        .post_empty_with_headers(
            &format!("/api/v1/loans/{loan_b}/renew"),
            Some(&admin_token),
            &idem_headers(&key),
        )
        .await;
    assert_eq!(r2, StatusCode::CONFLICT, "renew B conflict: {rb2}");
}

#[tokio::test]
async fn renew_by_item_replays_same_key() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _item_id, barcode) = seed_checkout_item(&app, &admin_token).await;
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemIdentification": barcode
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    let key = format!("renew-item-{}", fixtures::unique_suffix());
    let uri = format!("/api/v1/loans/items/{barcode}/renew");

    let (status1, body1) = app
        .post_empty_with_headers(&uri, Some(&admin_token), &idem_headers(&key))
        .await;
    assert_eq!(status1, StatusCode::OK, "first by-item renew: {body1}");
    assert_eq!(fixtures::json_id(&body1["id"]), loan_id);

    let (status2, body2) = app
        .post_empty_with_headers(&uri, Some(&admin_token), &idem_headers(&key))
        .await;
    assert_eq!(status2, StatusCode::OK, "replay by-item renew: {body2}");
    assert_eq!(body2["expiryAt"], body1["expiryAt"]);
    assert_eq!(fixtures::json_id(&body2["id"]), loan_id);
}
