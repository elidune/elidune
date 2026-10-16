//! Checkout unpaid-fine override (#173).
//!
//! `fines_over_threshold` uses the same rule as renewal: loans write
//! (`loans_rights` `w`) makes it forceable. The account-type name is not
//! consulted. A successful skip is audited after the loan commits as
//! `loan.fine_threshold_overridden` with `{ reason, loanId, patron_id }`.
//! The override is that loan only (and each created loan of a batch). Mode
//! `none` does not block, so the override is not offered.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn next_id() -> i64 {
    static N: AtomicI64 = AtomicI64::new(1);
    let tick = fixtures::unique_suffix() as i64;
    tick.wrapping_add(N.fetch_add(1, Ordering::Relaxed))
}

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, i64, String) {
    let barcode = format!("CFO-{}", fixtures::unique_suffix());
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
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (biblio_id, item_id, barcode)
}

async fn amount_audience(app: &TestApp, admin_token: &str, cents: i64) -> i64 {
    let name = format!("cfo_{}", fixtures::unique_suffix());
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

/// Custom account type. The code is never `admin`, `librarian`, or `reader`.
async fn user_with_rights(
    app: &TestApp,
    admin_token: &str,
    loans_rights: &str,
    settings_rights: &str,
) -> (i64, String, String) {
    let code = format!("desk{}", fixtures::unique_token());
    assert!(!matches!(code.as_str(), "admin" | "librarian" | "reader"));
    let (status, body) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": format!("Desk {code}"),
                "loansRights": loans_rights,
                "settingsRights": settings_rights
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "account type: {body}");
    assert_eq!(body["loansRights"], loans_rights, "{body}");
    assert_ne!(body["code"], "admin");

    let login = format!("desk_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Desk",
                "lastname": "Staff",
                "email": format!("{login}@test.local"),
                "accountType": code,
                "publicType": public_type_id.to_string(),
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "user: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (status, session) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "login: {session}");
    let token = session["token"].as_str().expect("token").to_string();
    (user_id, token, code)
}

fn checkout_body(user_id: i64, item_id: i64, force: bool) -> Value {
    json!({
        "userId": user_id.to_string(),
        "itemId": item_id.to_string(),
        "force": force
    })
}

fn assert_fine_refusal(status: StatusCode, body: &Value, forceable: bool) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let mut keys: Vec<_> = body
        .as_object()
        .unwrap_or_else(|| panic!("object body: {body}"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, ["code", "error", "forceable", "message"], "{body}");
    assert_eq!(body["code"], "fines_over_threshold", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert_eq!(body["forceable"], forceable, "{body}");
    let message = body["message"].as_str().unwrap_or("");
    assert!(message.contains("5"), "{body}");
    assert!(message.contains("1"), "{body}");
}

async fn insert_fine(app: &TestApp, loan_id: i64, reader_id: i64) {
    sqlx::query(
        r#"
        INSERT INTO fines (id, loan_id, user_id, amount, paid_amount, status, charge_type)
        VALUES ($1, $2, $3, 500, 0, 'pending', 'overdue')
        "#,
    )
    .bind(next_id())
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("insert fine");
}

async fn override_payload(app: &TestApp, loan_id: i64) -> (Option<i64>, Value) {
    let pool = app.state.services.repository.pool();
    for _ in 0..40 {
        let row: Option<(Option<i64>, String, Value)> = sqlx::query_as(
            r#"
            SELECT user_id, entity_type, payload
            FROM audit_log
            WHERE event_type = 'loan.fine_threshold_overridden' AND entity_id = $1
            "#,
        )
        .bind(loan_id)
        .fetch_optional(pool)
        .await
        .expect("audit query");
        if let Some((actor, entity_type, payload)) = row {
            assert_eq!(entity_type, "loan");
            return (actor, payload);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("loan.fine_threshold_overridden missing for loan {loan_id}");
}

fn assert_payload(payload: &Value, loan_id: i64, patron_id: i64) {
    let mut keys: Vec<_> = payload
        .as_object()
        .expect("payload")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, ["loanId", "patron_id", "reason"], "{payload}");
    assert_eq!(payload["reason"], "fines_over_threshold", "{payload}");
    assert_eq!(payload["loanId"], json!(loan_id), "{payload}");
    assert_eq!(payload["patron_id"], json!(patron_id), "{payload}");
}

async fn override_count_for_patron(app: &TestApp, patron_id: i64) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM audit_log
        WHERE event_type = 'loan.fine_threshold_overridden'
          AND payload->>'patron_id' = $1
        "#,
    )
    .bind(patron_id.to_string())
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("count overrides")
}

#[tokio::test]
async fn loans_write_forces_one_checkout_and_audits_the_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let cents = elidune_server::models::fine::money_to_cents("1".parse().expect("threshold"))
        .expect("cents");
    let public_type = amount_audience(&app, &admin_token, cents).await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "cfo", Some(public_type))
            .await;
    let (staff_id, desk_token, account_type) = user_with_rights(&app, &admin_token, "w", "n").await;
    assert_ne!(account_type, "admin");
    let (_, blocked_token, blocked_type) = user_with_rights(&app, &admin_token, "n", "w").await;
    assert_ne!(blocked_type, "admin");

    let (_, seed_item, _) = create_copy(&app, &admin_token, "Fine Seed").await;
    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, seed_item, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    insert_fine(&app, fixtures::json_id(&created["id"]), reader_id).await;

    let (_, blocked_item, _) = create_copy(&app, &admin_token, "Fine Blocked").await;
    let (status, refused) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, blocked_item, false),
            Some(&desk_token),
        )
        .await;
    assert_fine_refusal(status, &refused, true);

    let (status, denied) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, blocked_item, true),
            Some(&blocked_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "authorization_failed", "{denied}");
    assert!(denied.get("forceable").is_none(), "{denied}");

    let (status, forced) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, blocked_item, true),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{forced}");
    let loan_id = fixtures::json_id(&forced["id"]);
    let (actor, payload) = override_payload(&app, loan_id).await;
    assert_eq!(actor, Some(staff_id));
    assert_payload(&payload, loan_id, reader_id);

    let (_, later_item, _) = create_copy(&app, &admin_token, "Still Blocked").await;
    let (status, still) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, later_item, false),
            Some(&desk_token),
        )
        .await;
    assert_fine_refusal(status, &still, true);
    assert_eq!(override_count_for_patron(&app, reader_id).await, 1);
}

#[tokio::test]
async fn batch_force_audits_each_created_loan_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let cents = elidune_server::models::fine::money_to_cents("1".parse().expect("threshold"))
        .expect("cents");
    let public_type = amount_audience(&app, &admin_token, cents).await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "cfobatch", Some(public_type))
            .await;
    let (_, desk_token, _) = user_with_rights(&app, &admin_token, "w", "n").await;

    let (_, seed_item, _) = create_copy(&app, &admin_token, "Batch Seed").await;
    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, seed_item, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    insert_fine(&app, fixtures::json_id(&created["id"]), reader_id).await;

    let (_, _, first) = create_copy(&app, &admin_token, "Batch One").await;
    let (_, _, second) = create_copy(&app, &admin_token, "Batch Two").await;
    let (status, body) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_id.to_string(),
                "barcodes": [first, second],
                "force": true
            }),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["created"], 2, "{body}");
    assert_eq!(body["errors"], 0, "{body}");
    let first_loan = fixtures::json_id(&body["results"][0]["loanId"]);
    let second_loan = fixtures::json_id(&body["results"][1]["loanId"]);
    let (_, first_payload) = override_payload(&app, first_loan).await;
    let (_, second_payload) = override_payload(&app, second_loan).await;
    assert_payload(&first_payload, first_loan, reader_id);
    assert_payload(&second_payload, second_loan, reader_id);

    let (_, _, third) = create_copy(&app, &admin_token, "Batch Still Blocked").await;
    let (status, blocked) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_id.to_string(),
                "barcodes": [third],
                "force": false
            }),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{blocked}");
    assert_eq!(blocked["created"], 0, "{blocked}");
    assert_eq!(
        blocked["results"][0]["code"], "fines_over_threshold",
        "{blocked}"
    );
    assert_eq!(override_count_for_patron(&app, reader_id).await, 2);
}

#[tokio::test]
async fn non_forceable_copy_gate_writes_no_fine_override() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let cents = elidune_server::models::fine::money_to_cents("1".parse().expect("threshold"))
        .expect("cents");
    let public_type = amount_audience(&app, &admin_token, cents).await;
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "cfoheld", Some(public_type))
            .await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "cfoholder").await;
    let (_, desk_token, _) = user_with_rights(&app, &admin_token, "w", "n").await;

    let (_, seed_item, _) = create_copy(&app, &admin_token, "Held Seed").await;
    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, seed_item, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    insert_fine(&app, fixtures::json_id(&created["id"]), reader_id).await;

    let (biblio_id, held_item, _) = create_copy(&app, &admin_token, "Held Copy").await;
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status)
        VALUES ($1, $2, $3, $4, 1, 'ready')
        "#,
    )
    .bind(next_id())
    .bind(holder_id)
    .bind(held_item)
    .bind(biblio_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("ready hold");

    let (status, refused) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, held_item, true),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["code"], "item_held_for_other", "{refused}");
    assert_eq!(refused["forceable"], false, "{refused}");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(override_count_for_patron(&app, reader_id).await, 0);
}

#[tokio::test]
async fn none_mode_does_not_offer_the_override() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let name = format!("cfonone_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "none"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {body}");
    assert_eq!(body["unpaidThresholdMode"], "none", "{body}");
    let public_type = fixtures::json_id(&body["id"]);
    let (reader_id, _) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "cfonone", Some(public_type))
            .await;

    let (_, seed_item, _) = create_copy(&app, &admin_token, "None Seed").await;
    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, seed_item, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    insert_fine(&app, fixtures::json_id(&created["id"]), reader_id).await;

    let (_, next_item, _) = create_copy(&app, &admin_token, "None Checkout").await;
    let (status, checked_out) = app
        .post_json(
            "/api/v1/loans",
            &checkout_body(reader_id, next_item, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{checked_out}");
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(override_count_for_patron(&app, reader_id).await, 0);
}
