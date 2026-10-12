//! QA #150 (#146): lost copy found again, extra cases.

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

fn money_is_zero(value: &serde_json::Value) -> bool {
    decimal_str(value)
        .parse::<f64>()
        .is_ok_and(|n| n.abs() < 0.001)
}

async fn fines_of(app: &TestApp, token: &str, reader_id: i64) -> serde_json::Value {
    let (status, fines) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    fines
}

fn count_entries(body: &serde_json::Value, event_type: &str) -> usize {
    body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["eventType"] == event_type)
        .count()
}

/// Fine no longer open (waived) -> return reports wasLost without fine fields.
#[tokio::test]
async fn qa_return_omits_fine_fields_when_fine_closed() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150waive").await;
    let (loan_id, item_id, _, barcode) = checkout(&app, &token, reader_id, "12.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, w) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/waive"),
            &json!({ "notes": "qa" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "waive: {w}");
    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "scan: {body}");
    assert_eq!(body["wasLost"], true, "{body}");
    for k in ["fineId", "fineStatus", "fineAmount", "paidAmount"] {
        assert!(
            body.get(k).is_none(),
            "{k} must be omitted when fine closed: {body}"
        );
    }
    let (c, b) = item_flags(&app, &token, item_id).await;
    assert_eq!((c, b), (1, false));
}

/// 403 on a real lost copy, and the copy is untouched.
#[tokio::test]
async fn qa_resolve_forbidden_leaves_item_untouched() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150rbac").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "10.00").await;
    mark_lost(&app, &token, loan_id, true).await;
    let secret = app.state.config.users.jwt_secret.as_str();
    let mut claims = UserClaims::from_token(&token, secret).expect("token");
    claims.rights.loans_rights = Rights::Read;
    let weak = claims.create_token(secret).expect("weak");
    let (status, body) = resolve(&app, &weak, item_id, "circulate").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "loans_rights=read: {body}");
    let (c, b) = item_flags(&app, &token, item_id).await;
    assert_eq!((c, b), (1, false));
    let fines = fines_of(&app, &token, reader_id).await;
    assert_eq!(fines["fines"][0]["status"], "pending", "{fines}");
}

#[tokio::test]
async fn qa_resolve_unknown_item_is_404() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (status, body) = resolve(&app, &token, 999_999_999_999, "circulate").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test]
async fn qa_resolve_not_lost_is_422() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150notlost").await;
    let (_, item_id, _, _) = checkout(&app, &token, reader_id, "10.00").await;
    for d in ["circulate", "damaged", "leave"] {
        let (status, body) = resolve(&app, &token, item_id, d).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{d}: {body}");
    }
}

#[tokio::test]
async fn qa_resolve_invalid_disposition_is_rejected() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150bad").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "10.00").await;
    mark_lost(&app, &token, loan_id, true).await;
    for payload in [
        json!({ "disposition": "burn" }),
        json!({}),
        json!({ "disposition": "" }),
    ] {
        let (status, body) = app
            .post_json(
                &format!("/api/v1/loans/items/{item_id}/lost-found"),
                &payload,
                Some(&token),
            )
            .await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
            "{payload}: {status} {body}"
        );
    }
    let (c, _) = item_flags(&app, &token, item_id).await;
    assert_eq!(c, 1, "invalid call must not change status");
    let fines = fines_of(&app, &token, reader_id).await;
    assert_eq!(fines["fines"][0]["status"], "pending");
}

/// leave with an open unpaid fee: fee untouched, copy stays lost, later circulate still works.
#[tokio::test]
async fn qa_leave_keeps_unpaid_fee_then_circulate_cancels() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150leave").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "15.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let before = fines_of(&app, &token, reader_id).await;

    let (status, r) = resolve(&app, &token, item_id, "leave").await;
    assert_eq!(status, StatusCode::OK, "leave: {r}");
    assert_eq!(r["itemStatus"], "lost");
    assert_eq!(r["fineAction"], "none");
    assert_eq!(r["refundedAmount"], 0);
    assert_eq!(r["cancelledAmount"], 0);
    let after = fines_of(&app, &token, reader_id).await;
    assert_eq!(before["fines"][0]["status"], after["fines"][0]["status"]);
    assert_eq!(
        decimal_str(&before["fines"][0]["amount"]),
        decimal_str(&after["fines"][0]["amount"])
    );
    assert_eq!(
        decimal_str(&before["totalUnpaid"]),
        decimal_str(&after["totalUnpaid"])
    );
    assert!(audit_entry(
        &audit_body(&app, &token, "fine.cancelled", fine_id).await,
        "fine.cancelled"
    )
    .is_none());

    let (status, r) = resolve(&app, &token, item_id, "circulate").await;
    assert_eq!(status, StatusCode::OK, "circulate after leave: {r}");
    assert_eq!(r["fineAction"], "cancelled");
    assert_eq!(r["cancelledAmount"], 1500);
}

/// paid fee + circulate: refunded, hold served, traced once; second call rejected, no double refund.
#[tokio::test]
async fn qa_paid_circulate_serves_hold_and_is_not_repeatable() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150paid").await;
    let (waiter_id, _) = fixtures::create_reader(&app, &token, "qa150paidw").await;
    let (loan_id, item_id, biblio_id, _) = checkout(&app, &token, reader_id, "12.50").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, p) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "12.50" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pay: {p}");
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({ "userId": waiter_id.to_string(), "biblioId": biblio_id.to_string() }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");

    let (status, r) = resolve(&app, &token, item_id, "circulate").await;
    assert_eq!(status, StatusCode::OK, "circulate: {r}");
    assert_eq!(r["fineAction"], "refunded");
    assert_eq!(r["refundedAmount"], 1250);
    assert_eq!(r["cancelledAmount"], 0);
    assert_eq!(r["hold"]["status"], "ready", "{r}");

    let (status, again) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "second call: {again}"
    );
    let audit = audit_body(&app, &token, "fine.refunded", fine_id).await;
    assert_eq!(count_entries(&audit, "fine.refunded"), 1, "{audit}");
    let fines = fines_of(&app, &token, reader_id).await;
    assert_eq!(fines["fines"][0]["status"], "refunded");
    assert_eq!(decimal_str(&fines["fines"][0]["refundedAmount"]), "12.50");
}

/// damaged twice: second call rejected; partial fee traced exactly once each way.
#[tokio::test]
async fn qa_damaged_partial_twice_is_rejected_cleanly() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "qa150dmg").await;
    let (loan_id, item_id, _, _) = checkout(&app, &token, reader_id, "20.00").await;
    let lost = mark_lost(&app, &token, loan_id, true).await;
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    let (status, _) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "5.00" }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, r) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["itemStatus"], "damaged");
    assert_eq!(r["fineAction"], "partial");
    assert_eq!(r["refundedAmount"], 500);
    assert_eq!(r["cancelledAmount"], 1500);
    let (status, again) = resolve(&app, &token, item_id, "damaged").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{again}");
    let (c, b) = item_flags(&app, &token, item_id).await;
    assert_eq!((c, b), (2, false));
    assert_eq!(
        count_entries(
            &audit_body(&app, &token, "fine.refunded", fine_id).await,
            "fine.refunded"
        ),
        1
    );
    assert_eq!(
        count_entries(
            &audit_body(&app, &token, "fine.cancelled", fine_id).await,
            "fine.cancelled"
        ),
        1
    );
    let fines = fines_of(&app, &token, reader_id).await;
    assert!(money_is_zero(&fines["totalUnpaid"]), "{fines}");
}
