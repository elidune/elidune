//! Checkout and renew are gated on the unpaid-fine threshold.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use rust_decimal::Decimal;
use serde_json::json;

async fn create_borrowable_item(app: &TestApp, admin_token: &str, prefix: &str) -> i64 {
    let payload = json!({
        "title": format!("Fine threshold {prefix}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": format!("{prefix}-{}", fixtures::unique_suffix()),
            "borrowable": true
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(admin_token),
    )
    .await
}

#[tokio::test]
async fn unpaid_under_threshold_allows_checkout() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "fineunder").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidFineThreshold": "10.00" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let item_id = create_borrowable_item(&app, &admin_token, "UNDER").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "checkout under threshold: {body}"
    );
}

#[tokio::test]
async fn unpaid_over_threshold_blocks_checkout_and_force_is_audited() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "fineover").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidFineThreshold": "5.00" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let first_item = create_borrowable_item(&app, &admin_token, "OVER1").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, first_item, false).await;
    assert_eq!(status, StatusCode::CREATED, "seed loan: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    app.state
        .services
        .repository
        .fines_create(loan_id, reader_id, Decimal::new(1500, 2), Some("overdue"))
        .await
        .expect("create unpaid fine");

    let blocked_item = create_borrowable_item(&app, &admin_token, "OVER2").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, blocked_item, false).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blocked: {body}");
    let message = body["message"].as_str().unwrap_or_default();
    assert!(message.contains("force=true"), "{message}");

    let (status, body) = checkout(&app, &admin_token, reader_id, blocked_item, true).await;
    assert_eq!(status, StatusCode::CREATED, "force checkout: {body}");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, audit) = app
        .get_json_with_auth(
            "/api/v1/audit?eventType=loan.fine_threshold_overridden",
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "audit query: {audit}");
    let entries = audit["entries"].as_array().cloned().unwrap_or_default();
    assert!(
        entries
            .iter()
            .any(|row| row["payload"]["operation"] == "checkout"),
        "expected override audit, got {audit}"
    );
}

#[tokio::test]
async fn unpaid_over_threshold_blocks_renew_unless_forced() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "finerenew").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidFineThreshold": "0" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let item_id = create_borrowable_item(&app, &admin_token, "RENEW").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    app.state
        .services
        .repository
        .fines_create(loan_id, reader_id, Decimal::new(250, 2), Some("late"))
        .await
        .expect("create unpaid fine");

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "renew blocked: {body}"
    );

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "force renew: {body}");
}
