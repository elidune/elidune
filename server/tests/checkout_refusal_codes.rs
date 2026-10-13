//! Checkout (`POST /loans`) refusals that used to share `business_rule_violation`.
//!
//! Each case keeps its previous HTTP 422 and desk message, and staff `force`
//! still skips the same gates it skipped before. A lost card and a withdrawn
//! copy are unchanged.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn next_id() -> i64 {
    static N: AtomicI64 = AtomicI64::new(1);
    let tick = fixtures::unique_suffix() as i64;
    tick.wrapping_add(N.fetch_add(1, Ordering::Relaxed))
}

async fn create_copy(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    borrowable: bool,
) -> (i64, i64, String) {
    let barcode = format!("CHK-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": borrowable }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (biblio_id, item_id, barcode)
}

async fn audience(app: &TestApp, admin_token: &str, settings: Value, threshold_cents: i64) -> i64 {
    let name = format!("chk_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "amount",
                "unpaidThresholdCents": threshold_cents
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {body}");
    let id = fixtures::json_id(&body["id"]);
    let (status, saved) = app
        .put_json(
            &format!("/api/v1/public-types/{id}/loan-settings"),
            &json!({ "settings": settings }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "loan settings: {saved}");
    id
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, Value) {
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

fn assert_refusal(status: StatusCode, body: &Value, code: &str, message_part: &str) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], code, "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains(message_part),
        "expected {message_part:?} in {body}"
    );
    assert!(body.get("code").is_some() && body.get("error").is_some());
}

async fn active_loan_count(app: &TestApp, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM loans WHERE user_id = $1 AND returned_at IS NULL")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("active loans")
}

#[tokio::test]
async fn expired_subscription_is_subscription_expired_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chksub").await;
    let (_, item_id, _) = create_copy(&app, &admin_token, "Expired At Checkout", true).await;

    sqlx::query("UPDATE users SET expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire subscription");

    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_refusal(status, &body, "subscription_expired", "2020-01-15");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .contains("force=true"),
        "{body}"
    );
    assert_eq!(active_loan_count(&app, reader_id).await, 0);

    let (status, created) = checkout(&app, &admin_token, reader_id, item_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force checkout: {created}");
    assert_eq!(active_loan_count(&app, reader_id).await, 1);
}

#[tokio::test]
async fn non_loanable_copy_is_item_not_loanable_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkloanable").await;

    let (_, reference_id, _) = create_copy(&app, &admin_token, "Not Borrowable", false).await;
    let (status, body) = checkout(&app, &admin_token, reader_id, reference_id, false).await;
    assert_refusal(status, &body, "item_not_loanable", "Item is not borrowable");
    assert_eq!(active_loan_count(&app, reader_id).await, 0);
    let (status, created) = checkout(&app, &admin_token, reader_id, reference_id, true).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "force not-borrowable: {created}"
    );

    let (_, lost_id, _) = create_copy(&app, &admin_token, "Lost Copy", true).await;
    sqlx::query("UPDATE items SET circulation_status = 1 WHERE id = $1")
        .bind(lost_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark lost");
    let (status, body) = checkout(&app, &admin_token, reader_id, lost_id, false).await;
    assert_refusal(
        status,
        &body,
        "item_not_loanable",
        "Item is marked lost and is not borrowable",
    );
    let (status, created) = checkout(&app, &admin_token, reader_id, lost_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force lost: {created}");
}

#[tokio::test]
async fn withdrawn_copy_stays_generic_and_force_does_not_override() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkwdn").await;
    let (_, item_id, _) = create_copy(&app, &admin_token, "Withdrawn Copy", true).await;
    sqlx::query("UPDATE items SET weeding_status = 'withdrawn' WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("withdraw");

    for force in [false, true] {
        let (status, body) = checkout(&app, &admin_token, reader_id, item_id, force).await;
        assert_refusal(
            status,
            &body,
            "business_rule_violation",
            "Item is withdrawn and cannot be checked out",
        );
    }
    assert_eq!(active_loan_count(&app, reader_id).await, 0);
}

#[tokio::test]
async fn total_quota_is_max_loans_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type = audience(
        &app,
        &admin_token,
        json!([{ "nbMax": 1, "duration": 21, "nbRenews": 1 }]),
        0,
    )
    .await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "chkquota", Some(public_type))
            .await;
    let (_, first_id, _) = create_copy(&app, &admin_token, "Quota First", true).await;
    let (_, second_id, _) = create_copy(&app, &admin_token, "Quota Second", true).await;

    let (status, created) = checkout(&app, &admin_token, reader_id, first_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "first loan: {created}");

    let (status, body) = checkout(&app, &admin_token, reader_id, second_id, false).await;
    assert_refusal(
        status,
        &body,
        "max_loans",
        "Maximum total loans reached (1/1)",
    );
    assert_eq!(active_loan_count(&app, reader_id).await, 1);

    let (status, forced) = checkout(&app, &admin_token, reader_id, second_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force over quota: {forced}");
    assert_eq!(active_loan_count(&app, reader_id).await, 2);
}

#[tokio::test]
async fn both_caps_still_report_max_loans() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type = audience(
        &app,
        &admin_token,
        json!([
            { "nbMax": 1, "duration": 21, "nbRenews": 1 },
            { "mediaType": "printedText", "nbMax": 1, "duration": 21, "nbRenews": 1 }
        ]),
        0,
    )
    .await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "chkboth", Some(public_type))
            .await;
    let (_, first_id, _) = create_copy(&app, &admin_token, "Both Caps First", true).await;
    let (_, second_id, _) = create_copy(&app, &admin_token, "Both Caps Second", true).await;
    let (status, created) = checkout(&app, &admin_token, reader_id, first_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, body) = checkout(&app, &admin_token, reader_id, second_id, false).await;
    assert_refusal(
        status,
        &body,
        "max_loans",
        "Maximum loans reached: total (1/1), this media type (1/1)",
    );
}

#[tokio::test]
async fn media_quota_is_max_media_loans_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type = audience(
        &app,
        &admin_token,
        json!([
            { "nbMax": 5, "duration": 21, "nbRenews": 1 },
            { "mediaType": "printedText", "nbMax": 1, "duration": 21, "nbRenews": 1 }
        ]),
        0,
    )
    .await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "chkmedia", Some(public_type))
            .await;
    let (_, first_id, _) = create_copy(&app, &admin_token, "Media First", true).await;
    let (_, second_id, _) = create_copy(&app, &admin_token, "Media Second", true).await;
    let (status, created) = checkout(&app, &admin_token, reader_id, first_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, body) = checkout(&app, &admin_token, reader_id, second_id, false).await;
    assert_refusal(
        status,
        &body,
        "max_media_loans",
        "Maximum loans for this document type reached (1/1)",
    );
    let (status, forced) = checkout(&app, &admin_token, reader_id, second_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force media quota: {forced}");
}

#[tokio::test]
async fn fines_over_threshold_matches_renewal_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let cents = elidune_server::models::fine::money_to_cents("10".parse().expect("threshold"))
        .expect("cents");
    let public_type = audience(
        &app,
        &admin_token,
        json!([{ "nbMax": 5, "duration": 21, "nbRenews": 1 }]),
        cents,
    )
    .await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "chkfines", Some(public_type))
            .await;
    let (_, first_id, _) = create_copy(&app, &admin_token, "Fine Anchor", true).await;
    let (_, second_id, _) = create_copy(&app, &admin_token, "Fine Blocked", true).await;
    let (status, created) = checkout(&app, &admin_token, reader_id, first_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);

    sqlx::query(
        r#"
        INSERT INTO fines (id, loan_id, user_id, amount, paid_amount, status, charge_type)
        VALUES ($1, $2, $3, 15.00, 0, 'pending', 'overdue')
        "#,
    )
    .bind(next_id())
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("insert fine");

    let (status, body) = checkout(&app, &admin_token, reader_id, second_id, false).await;
    assert_refusal(status, &body, "fines_over_threshold", "force=true");
    let message = body["message"].as_str().unwrap_or("");
    assert!(message.contains("15"), "{body}");
    assert!(message.contains("10"), "{body}");
    assert_eq!(active_loan_count(&app, reader_id).await, 1);

    let (status, forced) = checkout(&app, &admin_token, reader_id, second_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force fines: {forced}");
    assert_eq!(active_loan_count(&app, reader_id).await, 2);
}

#[tokio::test]
async fn hold_for_another_patron_is_item_held_for_other() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "chkhold_a").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "chkhold_b").await;

    let (biblio_id, item_id, _) = create_copy(&app, &admin_token, "Held Copy", true).await;
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status)
        VALUES ($1, $2, $3, $4, 1, 'ready')
        "#,
    )
    .bind(next_id())
    .bind(holder_id)
    .bind(item_id)
    .bind(biblio_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("ready hold");

    let (status, body) = checkout(&app, &admin_token, other_id, item_id, false).await;
    assert_refusal(
        status,
        &body,
        "item_held_for_other",
        "active hold for another patron",
    );
    assert!(
        body["message"]
            .as_str()
            .unwrap_or("")
            .contains("force=true"),
        "{body}"
    );
    assert_eq!(active_loan_count(&app, other_id).await, 0);

    let (biblio_id, owned_id, _) = create_copy(&app, &admin_token, "Holder Copy", true).await;
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status)
        VALUES ($1, $2, $3, $4, 1, 'ready')
        "#,
    )
    .bind(next_id())
    .bind(holder_id)
    .bind(owned_id)
    .bind(biblio_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("holder ready hold");
    let (status, created) = checkout(&app, &admin_token, holder_id, owned_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "queued patron: {created}");

    let (status, forced) = checkout(&app, &admin_token, other_id, item_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force held copy: {forced}");
}

#[tokio::test]
async fn to_pick_for_another_patron_keeps_its_code() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "chkpick_a").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "chkpick_b").await;
    let (biblio_id, item_id, _) = create_copy(&app, &admin_token, "To Pick Copy", true).await;
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status)
        VALUES ($1, $2, $3, $4, 1, 'to_pick')
        "#,
    )
    .bind(next_id())
    .bind(holder_id)
    .bind(item_id)
    .bind(biblio_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("to_pick hold");

    let (status, body) = checkout(&app, &admin_token, other_id, item_id, false).await;
    assert_refusal(status, &body, "item_to_pick_for_other", "pick list");
}

#[tokio::test]
async fn batch_checkout_reports_the_same_code() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkbatch").await;
    let (_, _, barcode) = create_copy(&app, &admin_token, "Batch Not Borrowable", false).await;

    let (status, body) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_id.to_string(),
                "barcodes": [barcode],
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["created"], 0, "{body}");
    assert_eq!(body["errors"], 1, "{body}");
    assert_eq!(body["results"][0]["success"], false, "{body}");
    assert_eq!(body["results"][0]["code"], "item_not_loanable", "{body}");
    assert!(
        body["results"][0]["error"]
            .as_str()
            .unwrap_or("")
            .contains("not borrowable"),
        "{body}"
    );
}
