//! `reminders.accrue_fines = false` ignores every unpaid-fine blocking threshold
//! and still sends overdue reminders. Stored thresholds are left as written.
//! Lost and damaged bills stay on the account.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};

use axum::http::StatusCode;
use common::{fixtures, TestApp};
use serde_json::{json, Value};

fn next_fine_id() -> i64 {
    static N: AtomicI64 = AtomicI64::new(1);
    (fixtures::unique_suffix() as i64).wrapping_add(N.fetch_add(1, Ordering::Relaxed))
}

fn set_accrue_fines(app: &TestApp, accrue: bool) {
    let mut value = serde_json::to_value(app.state.dynamic_config.read_reminders())
        .expect("serialize reminders");
    value["accrue_fines"] = json!(accrue);
    value["enabled"] = json!(true);
    app.state
        .dynamic_config
        .update_section("reminders", value)
        .expect("update reminders");
    assert_eq!(
        app.state.dynamic_config.read_reminders().accrue_fines,
        accrue
    );
}

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, String) {
    let barcode = format!("AFD-{}", fixtures::unique_suffix());
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

async fn amount_audience(app: &TestApp, admin_token: &str, cents: i64) -> i64 {
    let name = format!("afd_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "amount",
                "unpaidThresholdCents": cents
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {body}");
    fixtures::json_id(&body["id"])
}

async fn public_type_threshold(app: &TestApp, admin_token: &str, id: i64) -> Value {
    let (status, body) = app
        .get_json_with_auth("/api/v1/public-types", admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "public types: {body}");
    let row = body
        .as_array()
        .expect("public types")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == id)
        .unwrap_or_else(|| panic!("public type {id} missing: {body}"));
    json!({
        "unpaidThresholdMode": row["unpaidThresholdMode"],
        "unpaidThresholdCents": row["unpaidThresholdCents"]
    })
}

async fn global_threshold(app: &TestApp, admin_token: &str) -> Value {
    let (status, body) = app
        .get_json_with_auth("/api/v1/fines/policy", admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "fine policy: {body}");
    body
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": false
        }),
        Some(admin_token),
    )
    .await
}

async fn insert_replacement_fine(app: &TestApp, loan_id: i64, reader_id: i64) {
    sqlx::query(
        r#"
        INSERT INTO fines (id, loan_id, user_id, amount, paid_amount, status, charge_type, notes)
        VALUES ($1, $2, $3, 5.00, 0, 'pending', 'replacement', 'lost copy')
        "#,
    )
    .bind(next_fine_id())
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("insert replacement fine");
}

fn loan_row(items: &[Value], loan_id: i64) -> &Value {
    items
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == loan_id)
        .unwrap_or_else(|| panic!("loan {loan_id} missing from {items:?}"))
}

#[tokio::test]
async fn disabled_accrual_ignores_public_type_threshold_until_reenabled() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    set_accrue_fines(&app, false);

    let cents = 100;
    let public_type_id = amount_audience(&app, &admin_token, cents).await;
    let stored_type = public_type_threshold(&app, &admin_token, public_type_id).await;
    assert_eq!(stored_type["unpaidThresholdMode"], "amount");
    assert_eq!(stored_type["unpaidThresholdCents"], cents);

    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "afdreader",
        Some(public_type_id),
    )
    .await;

    let (seed_item, _) = create_copy(&app, &admin_token, "Accrue Fines Seed").await;
    let (status, created) = checkout(&app, &admin_token, reader_id, seed_item).await;
    assert_eq!(status, StatusCode::CREATED, "seed checkout: {created}");
    let seed_loan_id = fixtures::json_id(&created["id"]);
    insert_replacement_fine(&app, seed_loan_id, reader_id).await;

    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    let listed = fines["fines"]
        .as_array()
        .expect("fines")
        .iter()
        .any(|fine| fine["chargeType"] == "replacement" && fine["status"] == "pending");
    assert!(listed, "lost-item fine must stay listed: {fines}");

    let (second_item, _) = create_copy(&app, &admin_token, "Accrue Fines Checkout").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, second_item).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "checkout while fines off: {body}"
    );
    assert!(body.get("code").is_none(), "{body}");
    let second_loan_id = fixtures::json_id(&body["id"]);

    let (_, batch_barcode) = create_copy(&app, &admin_token, "Accrue Fines Batch").await;
    let (status, batch) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_id.to_string(),
                "barcodes": [batch_barcode],
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "batch: {batch}");
    assert_eq!(batch["created"], 1, "{batch}");
    assert_eq!(batch["errors"], 0, "{batch}");
    assert_eq!(batch["results"][0]["success"], true, "{batch}");
    assert!(batch["results"][0]["code"].is_null(), "{batch}");
    let batch_loan_id = fixtures::json_id(&batch["results"][0]["loanId"]);

    fixtures::pull_loan_into_renewal_window(&app, seed_loan_id).await;
    fixtures::pull_loan_into_renewal_window(&app, second_loan_id).await;

    let (status, mine) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "me loans: {mine}");
    let row = loan_row(mine["items"].as_array().expect("items"), second_loan_id);
    assert_eq!(row["canRenew"], true, "{row}");
    assert!(row["renewalRefusalCode"].is_null(), "{row}");

    let (status, renewed) = app
        .post_empty(
            &format!("/api/v1/loans/{seed_loan_id}/renew"),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "renew while fines off: {renewed}");
    assert_ne!(renewed["code"], "fines_over_threshold", "{renewed}");

    let (status, returned) = app
        .post_empty(
            &format!("/api/v1/loans/{batch_loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return batch loan: {returned}");

    assert_eq!(
        public_type_threshold(&app, &admin_token, public_type_id).await,
        stored_type,
        "disabling fines must not rewrite the public type threshold"
    );
    let global_while_off = global_threshold(&app, &admin_token).await;

    set_accrue_fines(&app, true);

    assert_eq!(
        public_type_threshold(&app, &admin_token, public_type_id).await,
        stored_type
    );
    assert_eq!(
        global_threshold(&app, &admin_token).await,
        global_while_off,
        "re-enabling fines must not rewrite the global threshold"
    );

    let (blocked_item, _) = create_copy(&app, &admin_token, "Accrue Fines Blocked").await;
    let (status, refused) = checkout(&app, &admin_token, reader_id, blocked_item).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["code"], "fines_over_threshold", "{refused}");
    assert_eq!(refused["forceable"], true, "{refused}");

    let (status, renew_refused) = app
        .post_empty(
            &format!("/api/v1/loans/{second_loan_id}/renew"),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{renew_refused}");
    assert_eq!(
        renew_refused["code"], "fines_over_threshold",
        "{renew_refused}"
    );

    let (status, mine) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "me loans after re-enable: {mine}");
    let row = loan_row(mine["items"].as_array().expect("items"), second_loan_id);
    assert_eq!(row["canRenew"], false, "{row}");
    assert_eq!(row["renewalRefusalCode"], "fines_over_threshold", "{row}");

    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines after re-enable: {fines}");
    let still_listed = fines["fines"]
        .as_array()
        .expect("fines")
        .iter()
        .any(|fine| fine["chargeType"] == "replacement" && fine["status"] == "pending");
    assert!(still_listed, "lost-item fine must remain listed: {fines}");
}

#[tokio::test]
async fn disabled_accrual_still_queues_overdue_reminder_without_a_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    set_accrue_fines(&app, false);

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "afdremind").await;
    let (item_id, _) = create_copy(&app, &admin_token, "Accrue Fines Reminder").await;
    let (status, created) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {created}");
    let loan_id = fixtures::json_id(&created["id"]);

    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader email");
    sqlx::query("UPDATE loans SET expiry_at = NOW() - interval '40 days' WHERE id = $1")
        .bind(loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backdate loan");

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "send reminders: {body}");
    let detail = body["details"]
        .as_array()
        .expect("details")
        .iter()
        .find(|row| row["email"] == email)
        .unwrap_or_else(|| panic!("reminder for {email} missing: {body}"));
    assert_eq!(detail["tier"], "first", "{detail}");
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM email_outbox WHERE to_addr = $1 AND status = 'pending'",
    )
    .bind(&email)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("outbox count");
    assert!(queued >= 1, "reminder must be queued, pending={queued}");

    let fines: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM fines WHERE user_id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("fine count");
    assert_eq!(fines, 0, "accrue_fines=false must not create a fine");
}
