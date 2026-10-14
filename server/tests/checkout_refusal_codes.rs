//! Checkout (`POST /loans`) refusals.
//!
//! Every 422 body includes server-computed `forceable`. Staff `force` skips a
//! gate only when that flag is true. A non-forceable refusal wins over an
//! earlier forceable one.

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
    checkout_with_reason(app, admin_token, user_id, item_id, force, None).await
}

async fn checkout_with_reason(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
    override_reason: Option<&str>,
) -> (StatusCode, Value) {
    let mut payload = json!({
        "userId": user_id.to_string(),
        "itemId": item_id.to_string(),
        "force": force
    });
    if let Some(reason) = override_reason {
        payload["overrideReason"] = json!(reason);
    }
    app.post_json("/api/v1/loans", &payload, Some(token)).await
}

/// Forceable codes for a caller with circulation override write (the admin token).
/// `subscription_expired` and `item_held_for_other` are forceable only for that caller.
fn expected_forceable(code: &str) -> bool {
    matches!(
        code,
        "subscription_expired"
            | "item_held_for_other"
            | "fines_over_threshold"
            | "max_loans"
            | "max_media_loans"
            | "item_not_loanable"
    )
}

async fn staff_token(
    app: &TestApp,
    admin_token: &str,
    name: &str,
    code_prefix: &str,
    settings_rights: &str,
    override_rights: &str,
) -> String {
    let code = format!("{code_prefix}{}", fixtures::unique_suffix() % 1_000_000_000);
    let (status, body) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": name,
                "loansRights": "w",
                "settingsRights": settings_rights,
                "circulationOverrideRights": override_rights
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create account type {name}: {body}"
    );
    assert_eq!(body["name"], name);
    assert_eq!(body["settingsRights"], settings_rights);
    assert_eq!(body["circulationOverrideRights"], override_rights);
    assert_ne!(body["code"], "admin");

    let login = format!("staff_{}", fixtures::unique_suffix());
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
    assert_eq!(status, StatusCode::CREATED, "create staff {name}: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "login {name}: {body}");
    body["token"].as_str().expect("token").to_string()
}

fn assert_refusal(status: StatusCode, body: &Value, code: &str, message_part: &str) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], code, "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert_eq!(body["forceable"], expected_forceable(code), "{body}");
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains(message_part),
        "expected {message_part:?} in {body}"
    );
    if code == "item_status_action_required" {
        assert!(body["itemStatus"].is_string(), "{body}");
    } else {
        assert!(body.get("itemStatus").is_none(), "{body}");
    }
    if expected_forceable(code) && matches!(code, "subscription_expired" | "item_held_for_other") {
        assert_eq!(body["reasonRequired"], true, "{body}");
    } else {
        assert!(body.get("reasonRequired").is_none(), "{body}");
    }
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
    let (me_status, me) = app
        .get_json_with_auth("/api/v1/auth/me", &admin_token)
        .await;
    assert_eq!(me_status, StatusCode::OK, "{me}");
    let actor_id = fixtures::json_id(&me["id"]);
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

    let (status, missing) = checkout(&app, &admin_token, reader_id, item_id, true).await;
    assert_eq!(missing["code"], "override_reason_required", "{missing}");
    assert_eq!(missing["error"], "Business Rule Violation", "{missing}");
    assert!(missing.get("forceable").is_none(), "{missing}");
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");

    let (status, created) = checkout_with_reason(
        &app,
        &admin_token,
        reader_id,
        item_id,
        true,
        Some("membership renewal is at the desk"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "force checkout: {created}");
    assert_eq!(active_loan_count(&app, reader_id).await, 1);
    let loan_id = fixtures::json_id(&created["id"]);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, audit) = app
        .get_json_with_auth(
            &format!(
                "/api/v1/audit?eventType=loan.subscription_expired_overridden&entityId={loan_id}"
            ),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "audit query: {audit}");
    let entries = audit["entries"].as_array().cloned().unwrap_or_default();
    let row = entries
        .iter()
        .find(|row| fixtures::json_id(&row["entityId"]) == loan_id)
        .expect("subscription override audit");
    assert_eq!(fixtures::json_id(&row["userId"]), actor_id);
    assert_eq!(row["entityType"], "loan");
    assert_eq!(row["payload"]["code"], "subscription_expired");
    assert_eq!(
        row["payload"]["overrideReason"], "membership renewal is at the desk",
        "{row}"
    );
    assert!(row["payload"].get("user_id").is_none(), "{row}");
    assert_eq!(fixtures::json_id(&row["payload"]["patron_id"]), reader_id);
    assert_eq!(fixtures::json_id(&row["payload"]["item_id"]), item_id);
    assert_eq!(fixtures::json_id(&row["payload"]["loan_id"]), loan_id);
}

#[tokio::test]
async fn desk_agent_cannot_force_expired_subscription() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let desk = staff_token(&app, &admin_token, "Agent guichet", "desk", "n", "n").await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkdesk").await;
    let (_, item_id, _) = create_copy(&app, &admin_token, "Desk Expired", true).await;
    sqlx::query("UPDATE users SET expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire subscription");

    for force in [false, true] {
        let (status, body) = checkout(&app, &desk, reader_id, item_id, force).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "force={force}: {body}"
        );
        assert_eq!(body["code"], "subscription_expired", "{body}");
        assert_eq!(body["error"], "Business Rule Violation", "{body}");
        assert_eq!(body["forceable"], false, "force={force}: {body}");
    }
    assert_eq!(active_loan_count(&app, reader_id).await, 0);
}

#[tokio::test]
async fn subscription_expired_force_follows_override_right_not_the_account_type_name() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let named_admin = staff_token(&app, &admin_token, "Admin", "admlike", "w", "n").await;
    let coordinator = staff_token(&app, &admin_token, "Floor coordinator", "lead", "n", "w").await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkrights").await;
    let (_, item_id, _) = create_copy(&app, &admin_token, "Rights Not Name", true).await;
    sqlx::query("UPDATE users SET expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire subscription");

    let (status, body) = checkout(&app, &named_admin, reader_id, item_id, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "subscription_expired", "{body}");
    assert_eq!(body["forceable"], false, "{body}");
    assert_eq!(active_loan_count(&app, reader_id).await, 0);

    let (status, created) = checkout_with_reason(
        &app,
        &coordinator,
        reader_id,
        item_id,
        true,
        Some("coordinator override"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "circulation override on a custom type: {created}"
    );
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
}

#[tokio::test]
async fn damaged_copy_is_item_not_loanable_and_force_still_checks_out() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkdmg").await;
    let (_, item_id, _) = create_copy(&app, &admin_token, "Damaged Copy", true).await;
    sqlx::query("UPDATE items SET circulation_status = 2 WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark damaged");

    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_refusal(
        status,
        &body,
        "item_not_loanable",
        "Item is marked damaged and is not borrowable",
    );
    let (status, created) = checkout(&app, &admin_token, reader_id, item_id, true).await;
    assert_eq!(status, StatusCode::CREATED, "force damaged: {created}");
}

#[tokio::test]
async fn status_action_required_is_refused_with_force() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "chkstat").await;
    for (db_status, api_status) in [
        (1_i16, "lost"),
        (3, "claimedReturned"),
        (4, "toSearch"),
        (5, "missing"),
    ] {
        let (_, item_id, _) =
            create_copy(&app, &admin_token, &format!("Status {api_status}"), true).await;
        sqlx::query("UPDATE items SET circulation_status = $2 WHERE id = $1")
            .bind(item_id)
            .bind(db_status)
            .execute(app.state.services.repository.pool())
            .await
            .expect("set circulation status");
        for force in [false, true] {
            let (status, body) = checkout(&app, &admin_token, reader_id, item_id, force).await;
            assert_refusal(
                status,
                &body,
                "item_status_action_required",
                &format!("Item circulation status is {api_status}."),
            );
            assert_eq!(body["itemStatus"], api_status, "{body}");
            assert_eq!(body["forceable"], false, "{body}");
        }
        assert_eq!(active_loan_count(&app, reader_id).await, 0);
    }
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
        !body["message"]
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

    let (status, blank) =
        checkout_with_reason(&app, &admin_token, other_id, item_id, true, Some("   ")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{blank}");
    assert_eq!(blank["code"], "override_reason_required", "{blank}");
    assert_eq!(blank["error"], "Business Rule Violation", "{blank}");
    assert!(blank.get("forceable").is_none(), "{blank}");
    assert_eq!(active_loan_count(&app, other_id).await, 0);

    let desk = staff_token(&app, &admin_token, "Desk only", "holddesk", "n", "n").await;
    let (status, denied) = checkout_with_reason(
        &app,
        &desk,
        other_id,
        item_id,
        true,
        Some("desk cannot force a hold"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{denied}");
    assert_eq!(denied["code"], "item_held_for_other", "{denied}");
    assert_eq!(denied["forceable"], false, "{denied}");
    assert!(denied.get("reasonRequired").is_none(), "{denied}");
}

#[tokio::test]
async fn expired_subscription_loses_to_a_copy_held_for_another_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ordhold").await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "ordsub").await;
    let (biblio_id, item_id, _) = create_copy(&app, &admin_token, "Held And Expired", true).await;
    sqlx::query("UPDATE users SET expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire subscription");
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

    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_refusal(status, &body, "subscription_expired", "2020-01-15");
    assert_eq!(body["reasonRequired"], true, "{body}");

    let (status, missing) = checkout(&app, &admin_token, reader_id, item_id, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");
    assert_eq!(missing["code"], "override_reason_required", "{missing}");
    assert_eq!(active_loan_count(&app, reader_id).await, 0);
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
