//! Lost copy found at the desk (#146).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::json;

fn decimal_str(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_f64().map(|n| format!("{n:.2}")))
        .unwrap_or_else(|| value.to_string().trim_matches('"').to_string())
}

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

async fn checkout(
    app: &TestApp,
    token: &str,
    reader_id: i64,
    price: &str,
) -> (i64, i64, i64, String) {
    let barcode = format!("LF-{}", fixtures::unique_suffix());
    let payload = json!({
        "title": format!("Lost Found {}", fixtures::unique_suffix()),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{ "barcode": barcode, "borrowable": true, "price": price }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({ "userId": reader_id.to_string(), "itemId": item_id.to_string() }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    (fixtures::json_id(&loan["id"]), item_id, biblio_id, barcode)
}

async fn mark_lost(app: &TestApp, token: &str, loan_id: i64, bill: bool) -> serde_json::Value {
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": bill, "notes": "declared lost" }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {body}");
    body
}

async fn item_flags(app: &TestApp, token: &str, item_id: i64) -> (i64, bool) {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "item: {body}");
    let item = &body["items"][0];
    (
        item["circulationStatus"].as_i64().unwrap_or(0),
        item["borrowable"].as_bool().unwrap_or(true),
    )
}

async fn resolve(
    app: &TestApp,
    token: &str,
    item_id: i64,
    disposition: &str,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        &format!("/api/v1/loans/items/{item_id}/lost-found"),
        &json!({ "disposition": disposition }),
        Some(token),
    )
    .await
}

fn audit_entry<'a>(body: &'a serde_json::Value, event_type: &str) -> Option<&'a serde_json::Value> {
    body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["eventType"] == event_type)
}

async fn audit_body(
    app: &TestApp,
    token: &str,
    event_type: &str,
    entity_id: i64,
) -> serde_json::Value {
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/audit?eventType={event_type}&entityId={entity_id}&perPage=20"),
            token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "audit: {body}");
    body
}

/// Item lost-found events are inserted after the response. Retry until the row is visible.
async fn wait_audit(
    app: &TestApp,
    token: &str,
    event_type: &str,
    entity_id: i64,
) -> serde_json::Value {
    for _ in 0..25 {
        let body = audit_body(app, token, event_type, entity_id).await;
        if audit_entry(&body, event_type).is_some() {
            return body;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    panic!("missing audit {event_type} for {entity_id}");
}

fn money_is_zero(value: &serde_json::Value) -> bool {
    decimal_str(value)
        .parse::<f64>()
        .is_ok_and(|n| n.abs() < 0.001)
}

#[tokio::test]
async fn normal_return_omits_was_lost() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfplain").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "10.00").await;
    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    assert!(body.get("wasLost").is_none(), "normal return: {body}");
    assert!(body.get("fineId").is_none(), "normal return fine: {body}");
    assert_eq!(body["status"], "returned");
    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!(circulation, 0);
    assert!(borrowable);
}

#[tokio::test]
async fn lost_return_keeps_status_and_circulate_cancels_unpaid_fee() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let actor_id = fixtures::json_id(&me["id"]);
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfunpaid").await;
    let (waiter_id, _) = fixtures::create_reader(&app, &token, "lfwait").await;
    let (loan_id, item_id, biblio_id, _) = checkout(&app, &token, reader_id, "15.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    assert_eq!(lost["charge"]["chargeType"], "replacement");
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({ "userId": waiter_id.to_string(), "biblioId": biblio_id.to_string() }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return lost: {body}");
    assert_eq!(body["status"], "lost_found");
    assert_eq!(body["wasLost"], true);
    assert_eq!(fixtures::json_id(&body["fineId"]), fine_id);
    assert_eq!(body["fineStatus"], "unpaid");
    assert_eq!(body["fineAmount"], 1500);
    assert_eq!(body["paidAmount"], 0);
    assert!(body.get("loan").is_none());
    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!(
        (circulation, borrowable),
        (1, false),
        "return must not clear lost"
    );

    let (status, holds) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "holds before resolve: {holds}");
    assert_eq!(holds[0]["status"], "pending");

    let (status, resolved) = resolve(&app, &token, item_id, "circulate").await;
    assert_eq!(status, StatusCode::OK, "circulate: {resolved}");
    assert_eq!(resolved["itemStatus"], "available");
    assert_eq!(resolved["borrowable"], true);
    assert_eq!(resolved["fineAction"], "cancelled");
    assert_eq!(resolved["refundedAmount"], 0);
    assert_eq!(resolved["cancelledAmount"], 1500);
    assert_eq!(resolved["hold"]["status"], "ready");

    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (0, true));

    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    assert!(money_is_zero(&fines["totalUnpaid"]), "unpaid: {fines}");
    assert_eq!(fines["fines"][0]["status"], "cancelled");

    let (status, again) = resolve(&app, &token, item_id, "circulate").await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "second resolve: {again}"
    );

    let seen = wait_audit(&app, &token, "item.lost_found_seen", item_id).await;
    let seen_row = audit_entry(&seen, "item.lost_found_seen").expect("seen audit");
    assert_eq!(fixtures::json_id(&seen_row["userId"]), actor_id);
    assert!(seen_row["createdAt"].is_string());

    let cancelled = audit_body(&app, &token, "fine.cancelled", fine_id).await;
    let row = audit_entry(&cancelled, "fine.cancelled").expect("cancel audit");
    assert_eq!(fixtures::json_id(&row["userId"]), actor_id);
    assert_eq!(row["payload"]["cancelledAmount"], 1500);
    assert!(row["createdAt"].is_string());
    let refunded = audit_body(&app, &token, "fine.refunded", fine_id).await;
    assert!(audit_entry(&refunded, "fine.refunded").is_none());
}

#[tokio::test]
async fn paid_replacement_is_refunded_on_barcode_return_then_circulate() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfpaid").await;
    let (loan_id, item_id, _, barcode) = checkout(&app, &token, reader_id, "15.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, paid) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "15.00" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pay: {paid}");

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "scan: {body}");
    assert_eq!(
        body["notice"]["code"], "item_not_available_status",
        "{body}"
    );
    assert_eq!(body["notice"]["level"], "warning", "{body}");
    assert_eq!(body["circulationStatus"], "lost", "{body}");
    assert_eq!(body["status"], "unchanged", "{body}");
    assert!(body.get("wasLost").is_none(), "{body}");
    assert!(body.get("fineId").is_none(), "{body}");
    let (circulation, _) = item_flags(&app, &token, item_id).await;
    assert_eq!(circulation, 1);

    let (status, resolved) = resolve(&app, &token, item_id, "circulate").await;
    assert_eq!(status, StatusCode::OK, "resolve: {resolved}");
    assert_eq!(resolved["fineAction"], "refunded");
    assert_eq!(resolved["refundedAmount"], 1500);
    assert_eq!(resolved["cancelledAmount"], 0);

    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    assert_eq!(fines["fines"][0]["status"], "refunded");
    assert_eq!(decimal_str(&fines["fines"][0]["refundedAmount"]), "15.00");

    let refunded = audit_body(&app, &token, "fine.refunded", fine_id).await;
    let row = audit_entry(&refunded, "fine.refunded").expect("refund audit");
    assert_eq!(row["payload"]["refundedAmount"], 1500);
    assert!(row["userId"].is_number() || row["userId"].is_string());
}

async fn user_fines(app: &TestApp, token: &str, reader_id: i64) -> serde_json::Value {
    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    fines
}

async fn assert_no_fine_settlement_audit(app: &TestApp, token: &str, fine_id: i64) {
    for event in ["fine.cancelled", "fine.refunded", "fine.waived"] {
        let body = audit_body(app, token, event, fine_id).await;
        assert!(
            audit_entry(&body, event).is_none(),
            "unexpected {event} for fine {fine_id}: {body}"
        );
    }
}

/// Found + damaged with an open 2000-cent replacement charge: the fine row stays
/// as it is, and no cancel or refund audit is written.
#[tokio::test]
async fn open_replacement_stays_when_found_damaged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfdmgopen").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "20.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    assert_eq!(decimal_str(&lost["charge"]["amount"]), "20.00");

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    assert_eq!(body["wasLost"], true);
    assert_eq!(body["fineStatus"], "unpaid");
    assert_eq!(body["fineAmount"], 2000);
    assert_eq!(body["paidAmount"], 0);

    let before = user_fines(&app, &token, reader_id).await;
    assert_eq!(before["fines"][0]["status"], "pending");
    assert_eq!(decimal_str(&before["fines"][0]["amount"]), "20.00");

    let (status, resolved) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(status, StatusCode::OK, "damaged: {resolved}");
    assert_eq!(resolved["itemStatus"], "damaged");
    assert_eq!(resolved["borrowable"], false);
    assert_eq!(resolved["disposition"], "damaged");
    assert_eq!(resolved["fineAction"], "unchanged");
    assert_eq!(fixtures::json_id(&resolved["fineId"]), fine_id);
    assert_eq!(resolved["refundedAmount"], 0);
    assert_eq!(resolved["cancelledAmount"], 0);

    let after = user_fines(&app, &token, reader_id).await;
    assert_eq!(
        before["fines"][0], after["fines"][0],
        "fine row changed: {after}"
    );
    assert_eq!(before["totalUnpaid"], after["totalUnpaid"]);
    assert_no_fine_settlement_audit(&app, &token, fine_id).await;

    let seen = wait_audit(&app, &token, "item.lost_found_resolved", item_id).await;
    let row = audit_entry(&seen, "item.lost_found_resolved").expect("resolved audit");
    assert_eq!(row["payload"]["disposition"], "damaged");
    assert_eq!(row["payload"]["fineAction"], "unchanged");
    assert_eq!(row["payload"]["refundedAmount"], 0);
    assert_eq!(row["payload"]["cancelledAmount"], 0);

    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (2, false));
}

/// A replacement charge that was already paid is not refunded when the copy is damaged.
#[tokio::test]
async fn paid_replacement_is_not_refunded_when_found_damaged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfdmgpaid").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "20.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, paid) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "20.00" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pay: {paid}");

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    assert_eq!(body["fineStatus"], "paid");
    assert_eq!(body["fineAmount"], 2000);
    assert_eq!(body["paidAmount"], 2000);

    let before = user_fines(&app, &token, reader_id).await;
    assert_eq!(before["fines"][0]["status"], "paid");
    assert!(before["fines"][0].get("refundedAmount").is_none());

    let (status, resolved) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(status, StatusCode::OK, "damaged: {resolved}");
    assert_eq!(resolved["itemStatus"], "damaged");
    assert_eq!(resolved["fineAction"], "unchanged");
    assert_eq!(fixtures::json_id(&resolved["fineId"]), fine_id);
    assert_eq!(resolved["refundedAmount"], 0);
    assert_eq!(resolved["cancelledAmount"], 0);

    let after = user_fines(&app, &token, reader_id).await;
    assert_eq!(
        before["fines"][0], after["fines"][0],
        "paid fine changed: {after}"
    );
    assert_eq!(after["fines"][0]["status"], "paid");
    assert!(after["fines"][0].get("refundedAmount").is_none());
    assert_no_fine_settlement_audit(&app, &token, fine_id).await;

    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (2, false));
}

#[tokio::test]
async fn partial_payment_stays_when_found_damaged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfpart").await;
    let (loan_id, item_id, biblio_id, _) = checkout(&app, &token, reader_id, "15.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, paid) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "4.00" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "partial pay: {paid}");

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    assert_eq!(body["fineStatus"], "partial");
    assert_eq!(body["fineAmount"], 1500);
    assert_eq!(body["paidAmount"], 400);

    let (waiter_id, _) = fixtures::create_reader(&app, &token, "lfparthold").await;
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({ "userId": waiter_id.to_string(), "biblioId": biblio_id.to_string() }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");

    let before = user_fines(&app, &token, reader_id).await;
    assert_eq!(before["fines"][0]["status"], "partial");

    let (status, resolved) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(status, StatusCode::OK, "damaged: {resolved}");
    assert_eq!(resolved["itemStatus"], "damaged");
    assert_eq!(resolved["borrowable"], false);
    assert_eq!(resolved["fineAction"], "unchanged");
    assert_eq!(resolved["refundedAmount"], 0);
    assert_eq!(resolved["cancelledAmount"], 0);
    assert!(resolved.get("hold").is_none());

    let after = user_fines(&app, &token, reader_id).await;
    assert_eq!(
        before["fines"][0], after["fines"][0],
        "partial fine changed: {after}"
    );

    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (2, false));

    let (status, holds) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "holds: {holds}");
    assert_eq!(
        holds[0]["status"], "pending",
        "damaged copy must not satisfy the hold"
    );

    assert_no_fine_settlement_audit(&app, &token, fine_id).await;
}

#[tokio::test]
async fn leave_keeps_lost_status_and_the_fee() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfleave").await;
    let (loan_id, item_id, _, barcode) = checkout(&app, &token, reader_id, "9.00").await;
    mark_lost(&app, &token, loan_id, false).await;

    let (status, by_id) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "no charge to locate the closed loan: {by_id}"
    );

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "scan: {body}");
    assert_eq!(
        body["notice"]["code"], "item_not_available_status",
        "{body}"
    );
    assert_eq!(body["notice"]["level"], "warning", "{body}");
    assert_eq!(body["circulationStatus"], "lost", "{body}");
    assert_eq!(body["status"], "unchanged", "{body}");
    assert!(body.get("wasLost").is_none(), "{body}");
    assert!(body.get("fineId").is_none(), "no linked fine: {body}");

    let (status, resolved) = resolve(&app, &token, item_id, "leave").await;
    assert_eq!(status, StatusCode::OK, "leave: {resolved}");
    assert_eq!(resolved["itemStatus"], "lost");
    assert_eq!(resolved["fineAction"], "none");
    assert_eq!(resolved["refundedAmount"], 0);
    assert_eq!(resolved["cancelledAmount"], 0);
    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (1, false));

    let left = wait_audit(&app, &token, "item.lost_found_left", item_id).await;
    assert!(audit_entry(&left, "item.lost_found_left").is_some());
}

#[tokio::test]
async fn active_lost_loan_return_does_not_change_status() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "lfactive").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "8.00").await;
    sqlx::query("UPDATE items SET circulation_status = 1, borrowable = FALSE WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set lost");
    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    assert_eq!(body["status"], "returned");
    assert_eq!(body["wasLost"], true);
    assert!(body.get("fineId").is_none());
    let (circulation, borrowable) = item_flags(&app, &token, item_id).await;
    assert_eq!((circulation, borrowable), (1, false));
}

#[tokio::test]
async fn resolve_requires_loans_write() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let secret = app.state.config.users.jwt_secret.as_str();
    let mut claims = UserClaims::from_token(&token, secret).expect("admin token");
    claims.rights.loans_rights = Rights::Own;
    claims.rights.holds_rights = Rights::Write;
    let weak = claims.create_token(secret).expect("weak token");
    let (status, body) = app
        .post_json(
            "/api/v1/loans/items/1/lost-found",
            &json!({ "disposition": "circulate" }),
            Some(&weak),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "loans_rights=own: {body}");
}
