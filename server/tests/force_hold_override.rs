//! Force a loan past a hold for another patron (#167).
//!
//! The right is `circulation_override_rights` write, not the account-type name
//! and not settings write. `overrideReason` is mandatory. The hold stays first
//! and is still ready after the copy comes back. A lost card and an open loan
//! stay non-forceable. Patron identity on the refusal is card and initials only.

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

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, i64) {
    let barcode = format!("HLD-{}", fixtures::unique_suffix());
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
    (fixtures::json_id(&body["biblio"]["id"]), fixtures::json_id(&body["biblio"]["items"][0]["id"]))
}

async fn ready_hold(app: &TestApp, holder_id: i64, item_id: i64, biblio_id: i64, position: i32) -> i64 {
    let hold_id = next_id();
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status)
        VALUES ($1, $2, $3, $4, $5, 'ready')
        "#,
    )
    .bind(hold_id)
    .bind(holder_id)
    .bind(item_id)
    .bind(biblio_id)
    .bind(position)
    .execute(app.state.services.repository.pool())
    .await
    .expect("ready hold");
    hold_id
}

async fn hold_row(app: &TestApp, hold_id: i64) -> (String, i32, i64, Option<i64>) {
    sqlx::query_as("SELECT status, position, user_id, item_id FROM holds WHERE id = $1")
        .bind(hold_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("hold row")
}

fn checkout_body(user_id: i64, item_id: i64, force: bool, reason: Option<&str>) -> Value {
    let mut payload = json!({
        "userId": user_id.to_string(),
        "itemId": item_id.to_string(),
        "force": force
    });
    if let Some(reason) = reason {
        payload["overrideReason"] = json!(reason);
    }
    payload
}

#[tokio::test]
async fn built_in_types_keep_today_force_capability() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _admin = fixtures::ensure_first_setup(&app).await;
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT code, settings_rights, circulation_override_rights FROM account_types WHERE is_system ORDER BY code")
        .fetch_all(app.state.services.repository.pool())
        .await
        .expect("account types");
    for (code, settings, override_rights) in rows {
        let expected = if settings == "w" { "w" } else { "n" };
        assert_eq!(override_rights, expected, "{code} settings={settings}");
    }
}

#[tokio::test]
async fn forced_loan_keeps_the_hold_first_and_audits_the_reason() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (me_status, me) = app.get_json_with_auth("/api/v1/auth/me", &admin_token).await;
    assert_eq!(me_status, StatusCode::OK, "{me}");
    let staff_id = fixtures::json_id(&me["id"]);
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "holdkeep").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "holdtake").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "holdback").await;
    let (biblio_id, item_id) = create_copy(&app, &admin_token, "Held For Force").await;
    let hold_id = ready_hold(&app, holder_id, item_id, biblio_id, 1).await;
    let behind_id = next_id();
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, biblio_id, position, status)
        VALUES ($1, $2, $3, 2, 'pending')
        "#,
    )
    .bind(behind_id)
    .bind(other_id)
    .bind(biblio_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("second hold");

    let (status, refused) = app.post_json("/api/v1/loans", &checkout_body(borrower_id, item_id, false, None), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["code"], "item_held_for_other", "{refused}");
    assert_eq!(refused["error"], "Business Rule Violation", "{refused}");
    assert_eq!(refused["forceable"], true, "{refused}");
    assert_eq!(refused["reasonRequired"], true, "{refused}");
    let detail_keys = {
        let mut keys: Vec<_> = refused["details"].as_object().expect("details").keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(detail_keys, ["holdId"], "{refused}");
    assert_eq!(fixtures::json_id(&refused["details"]["holdId"]), hold_id);
    assert!(refused["details"].get("userId").is_none(), "{refused}");
    assert!(refused["details"].get("cardNumber").is_none(), "{refused}");
    assert!(refused["details"].get("initials").is_none(), "{refused}");
    assert!(refused["details"].get("firstname").is_none(), "{refused}");
    assert!(refused["details"].get("lastname").is_none(), "{refused}");
    assert!(!refused.to_string().contains("Reader"), "{refused}");

    let (status, created) = app
        .post_json("/api/v1/loans", &checkout_body(borrower_id, item_id, true, Some("reader is in the room")), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);
    let (status_name, position, user_id, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(status_name, "ready");
    assert_eq!(position, 1);
    assert_eq!(user_id, holder_id);
    assert_eq!(assigned, Some(item_id));
    let (behind_status, behind_position, behind_user, _): (String, i32, i64, Option<i64>) = hold_row(&app, behind_id).await;
    assert_eq!(behind_status, "pending");
    assert_eq!(behind_position, 2);
    assert_eq!(behind_user, other_id);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, audit) = app
        .get_json_with_auth(&format!("/api/v1/audit?eventType=loan.held_for_other_overridden&entityId={loan_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    let entries = audit["entries"].as_array().cloned().unwrap_or_default();
    let row = entries.iter().find(|row| fixtures::json_id(&row["entityId"]) == loan_id).expect("hold override audit");
    assert_eq!(fixtures::json_id(&row["userId"]), staff_id);
    assert_eq!(row["entityType"], "loan");
    assert_eq!(row["payload"]["reason"], "reader is in the room");
    assert_eq!(row["payload"]["overrideReason"], "reader is in the room");
    assert_eq!(fixtures::json_id(&row["payload"]["patron_id"]), borrower_id);
    assert_eq!(fixtures::json_id(&row["payload"]["holdId"]), hold_id);
    assert_eq!(fixtures::json_id(&row["payload"]["loanId"]), loan_id);
    assert!(row["payload"].get("staffId").is_none(), "{row}");

    let (status, returned) = app.post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    let (status_name, position, user_id, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(status_name, "ready", "return serves the same hold");
    assert_eq!(position, 1);
    assert_eq!(user_id, holder_id);
    assert_eq!(assigned, Some(item_id));
    let (behind_status, behind_position, _, _): (String, i32, i64, Option<i64>) = hold_row(&app, behind_id).await;
    assert_eq!(behind_status, "pending");
    assert_eq!(behind_position, 2);

    let (status, holder_loan) = app.post_json("/api/v1/loans", &checkout_body(holder_id, item_id, false, None), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "queued patron: {holder_loan}");
}

#[tokio::test]
async fn lost_card_and_open_loan_stay_non_forceable() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "losthold").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "lostborr").await;
    let (biblio_id, item_id) = create_copy(&app, &admin_token, "Lost And Held").await;
    let hold_id = ready_hold(&app, holder_id, item_id, biblio_id, 1).await;
    sqlx::query("UPDATE users SET card_lost = TRUE WHERE id = $1")
        .bind(borrower_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("lose card");

    let (status, lost) = app
        .post_json("/api/v1/loans", &checkout_body(borrower_id, item_id, true, Some("card was found")), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{lost}");
    assert_eq!(lost["code"], "card_lost", "{lost}");
    assert_eq!(lost["forceable"], false, "{lost}");
    assert!(lost.get("reasonRequired").is_none(), "{lost}");
    let (status_name, _, user_id, _) = hold_row(&app, hold_id).await;
    assert_eq!(status_name, "ready");
    assert_eq!(user_id, holder_id);

    let (owner_id, _) = fixtures::create_reader(&app, &admin_token, "openown").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "openoth").await;
    let (_, loaned_id) = create_copy(&app, &admin_token, "Already Out").await;
    let (status, first) = app.post_json("/api/v1/loans", &checkout_body(owner_id, loaned_id, false, None), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let (status, conflict) = app.post_json("/api/v1/loans", &checkout_body(other_id, loaned_id, true, Some("they asked")), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["code"], "item_on_loan_to_other_borrower", "{conflict}");
    assert_eq!(conflict["error"], "Conflict", "{conflict}");
    assert!(conflict.get("message").is_some(), "{conflict}");
    assert!(conflict.get("forceable").is_none(), "{conflict}");
}
