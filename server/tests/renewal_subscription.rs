//! Renewal refuses an expired patron subscription (`users.expiry_at` before now).
//!
//! `force` skips `subscription_expired` only for a caller with settings write,
//! the same rule as checkout. Loans write lifts `account_blocked` and
//! `fines_over_threshold`. A lost card stays non-forceable.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, String) {
    let barcode = format!("SUB-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (item_id, barcode)
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": user_id.to_string(),
                "itemId": item_id.to_string(),
                "force": force
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    fixtures::json_id(&body["id"])
}

fn loan_row(items: &[Value], loan_id: i64) -> &Value {
    items
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == loan_id)
        .unwrap_or_else(|| panic!("loan {loan_id} missing from {items:?}"))
}

async fn renewal_count(app: &TestApp, loan_id: i64) -> i16 {
    sqlx::query_scalar("SELECT COALESCE(nb_renews, 0)::int2 FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("renewal count")
}

async fn assert_refused(app: &TestApp, token: &str, path: &str) -> Value {
    let (status, body) = app.post_empty(path, Some(token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{path}: {body}");
    assert_eq!(body["code"], "subscription_expired", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    let message = body["message"].as_str().unwrap_or("");
    assert!(message.contains("2020-01-15"), "{body}");
    assert!(message.contains("force=true"), "{body}");
    body
}

#[tokio::test]
async fn expired_subscription_refuses_patron_and_staff_renewal_unless_forced() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "subexpired").await;
    let (item_id, barcode) = create_copy(&app, &admin_token, "Expired Subscription").await;
    let loan_id = checkout(&app, &admin_token, reader_id, item_id, false).await;

    sqlx::query("UPDATE users SET expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire subscription");

    let (status, list) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "my loans: {list}");
    let row = loan_row(list["items"].as_array().expect("items"), loan_id);
    assert_eq!(row["canRenew"], false, "{row}");
    assert_eq!(row["renewalRefusalCode"], "subscription_expired", "{row}");
    assert!(row["renewalOpensAt"].is_null(), "{row}");

    assert_refused(
        &app,
        &reader_token,
        &format!("/api/v1/loans/{loan_id}/renew"),
    )
    .await;
    assert_refused(
        &app,
        &admin_token,
        &format!("/api/v1/loans/{loan_id}/renew"),
    )
    .await;
    assert_refused(
        &app,
        &admin_token,
        &format!("/api/v1/loans/items/{barcode}/renew"),
    )
    .await;
    assert_eq!(renewal_count(&app, loan_id).await, 0);

    fixtures::pull_loan_into_renewal_window(&app, loan_id).await;
    let (status, list) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "inside window: {list}");
    let row = loan_row(list["items"].as_array().expect("items"), loan_id);
    assert_eq!(row["canRenew"], false, "{row}");
    assert_eq!(row["renewalRefusalCode"], "subscription_expired", "{row}");
    assert_refused(
        &app,
        &admin_token,
        &format!("/api/v1/loans/{loan_id}/renew"),
    )
    .await;
    assert_eq!(renewal_count(&app, loan_id).await, 0);

    let (status, renewed) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "forced renew: {renewed}");
    assert_eq!(renewal_count(&app, loan_id).await, 1);

    let (other_item, other_barcode) =
        create_copy(&app, &admin_token, "Expired Subscription By Copy").await;
    let other_loan = checkout(&app, &admin_token, reader_id, other_item, true).await;
    fixtures::pull_loan_into_renewal_window(&app, other_loan).await;
    assert_refused(
        &app,
        &admin_token,
        &format!("/api/v1/loans/items/{other_barcode}/renew"),
    )
    .await;
    let (status, forced_copy) = app
        .post_empty(
            &format!("/api/v1/loans/items/{other_barcode}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "forced renew by copy: {forced_copy}"
    );
    assert_eq!(renewal_count(&app, other_loan).await, 1);
}

#[tokio::test]
async fn open_or_future_subscription_still_renews() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    for (label, expiry_sql) in [
        (
            "future",
            "UPDATE users SET expiry_at = NOW() + interval '365 days' WHERE id = $1",
        ),
        ("open", "UPDATE users SET expiry_at = NULL WHERE id = $1"),
    ] {
        let (reader_id, reader_token) =
            fixtures::create_reader(&app, &admin_token, &format!("sub{label}")).await;
        let (item_id, _) = create_copy(&app, &admin_token, &format!("Subscription {label}")).await;
        let loan_id = checkout(&app, &admin_token, reader_id, item_id, false).await;
        sqlx::query(expiry_sql)
            .bind(reader_id)
            .execute(app.state.services.repository.pool())
            .await
            .unwrap_or_else(|err| panic!("set {label} expiry: {err}"));
        fixtures::pull_loan_into_renewal_window(&app, loan_id).await;

        let (status, list) = app
            .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
            .await;
        assert_eq!(status, StatusCode::OK, "{label}: {list}");
        let row = loan_row(list["items"].as_array().expect("items"), loan_id);
        assert_eq!(row["canRenew"], true, "{label}: {row}");
        assert!(row["renewalRefusalCode"].is_null(), "{label}: {row}");

        let (status, renewed) = app
            .post_empty(
                &format!("/api/v1/loans/{loan_id}/renew"),
                Some(&reader_token),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{label} renew: {renewed}");
        assert_eq!(renewal_count(&app, loan_id).await, 1, "{label}");
    }
}
