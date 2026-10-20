//! Fine money movements write their audit row in the same transaction (#160).
//!
//! Each test reads the audit log as soon as the API returns. No sleep, no retry.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

async fn admin(app: &TestApp) -> String {
    let token = fixtures::ensure_first_setup(app).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    if status == StatusCode::OK {
        let admin_id = fixtures::json_id(&me["id"]);
        let _ = app.state.services.users.set_must_change_password(admin_id, false).await;
    }
    let (status, body) = app.post_json("/api/v1/auth/login", &json!({ "username": "testadmin", "password": "testadmin1234" }), None).await;
    assert_eq!(status, StatusCode::OK, "admin login: {body}");
    body["token"].as_str().expect("token").to_string()
}

async fn lost_fine(app: &TestApp, token: &str, login: &str) -> (i64, i64, i64) {
    let (reader_id, _) = fixtures::create_reader(app, token, login).await;
    let barcode = format!("FA-{}", fixtures::unique_suffix());
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Fine audit {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true, "price": "15.00" }]
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {biblio}");
    let item_id = fixtures::json_id(&biblio["biblio"]["items"][0]["id"]);
    let (status, loan) = app
        .post_json("/api/v1/loans", &json!({ "userId": reader_id.to_string(), "itemId": item_id.to_string() }), Some(token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    let (status, lost) = app
        .post_json(&format!("/api/v1/loans/{loan_id}/lost"), &json!({ "bill": true, "notes": "declared lost" }), Some(token))
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {lost}");
    let fine_id = fixtures::json_id(&lost["charge"]["id"]);
    (reader_id, item_id, fine_id)
}

fn audit_entry<'a>(body: &'a serde_json::Value, event_type: &str) -> Option<&'a serde_json::Value> {
    body["entries"].as_array().into_iter().flatten().find(|e| e["eventType"] == event_type)
}

async fn audit_now(app: &TestApp, token: &str, event_type: &str, entity_id: i64) -> serde_json::Value {
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/audit?eventType={event_type}&entityId={entity_id}&perPage=20"), token).await;
    assert_eq!(status, StatusCode::OK, "audit: {body}");
    body
}

fn assert_audit_now(body: &serde_json::Value, event_type: &str) {
    let row = audit_entry(body, event_type).unwrap_or_else(|| panic!("missing {event_type}: {body}"));
    assert!(row["createdAt"].is_string(), "createdAt: {row}");
}

async fn fine_status(app: &TestApp, token: &str, reader_id: i64) -> String {
    let (status, fines) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), token).await;
    assert_eq!(status, StatusCode::OK, "fines: {fines}");
    fines["fines"][0]["status"].as_str().unwrap_or_default().to_string()
}

async fn item_lost(app: &TestApp, token: &str, item_id: i64) -> bool {
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}"), token).await;
    assert_eq!(status, StatusCode::OK, "item: {body}");
    let item = &body["items"][0];
    item["circulationStatus"].as_i64() == Some(1) && item["borrowable"].as_bool() == Some(false)
}

async fn resolve(app: &TestApp, token: &str, item_id: i64) -> (StatusCode, serde_json::Value) {
    app.post_json(&format!("/api/v1/loans/items/{item_id}/lost-found"), &json!({ "disposition": "circulate" }), Some(token))
        .await
}

#[tokio::test]
async fn payment_cannot_exceed_remaining_balance() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fablance").await;

    let (status, over) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "15.01" }), Some(&token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "overpay: {over}");
    assert_eq!(over["code"], "payment_exceeds_balance");
    assert_eq!(over["error"], "Business Rule Violation");
    assert!(over["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert_eq!(over["remainingCents"], 1500);
    assert_eq!(fine_status(&app, &token, reader_id).await, "pending");

    let (status, partial) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "4.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "partial: {partial}");
    assert_eq!(partial["status"], "partial");
    assert_eq!(partial["amount"], "15.00");
    assert_eq!(partial["paidAmount"], "4.00");

    let (status, still_over) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "11.01" }), Some(&token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "over remaining: {still_over}");
    assert_eq!(still_over["code"], "payment_exceeds_balance");
    assert_eq!(still_over["error"], "Business Rule Violation");
    assert!(still_over["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert_eq!(still_over["remainingCents"], 1100);
    assert_eq!(fine_status(&app, &token, reader_id).await, "partial");

    let (status, exact) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "11.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "exact: {exact}");
    assert_eq!(exact["status"], "paid");
    assert_eq!(exact["amount"], "15.00");
    assert_eq!(exact["paidAmount"], "15.00");
}

#[tokio::test]
async fn full_payment_audit_exists_when_pay_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (_reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fafull").await;
    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "15.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "pay: {paid}");
    assert_eq!(paid["status"], "paid");
    let body = audit_now(&app, &token, "fine.paid", fine_id).await;
    assert_audit_now(&body, "fine.paid");
}

#[tokio::test]
async fn partial_payment_audit_exists_when_pay_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fapart").await;
    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "4.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "partial pay: {paid}");
    assert_eq!(paid["status"], "partial");
    let body = audit_now(&app, &token, "fine.paid", fine_id).await;
    assert_audit_now(&body, "fine.paid");
    assert_eq!(fine_status(&app, &token, reader_id).await, "partial");
}

#[tokio::test]
async fn waiver_audit_exists_when_waive_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fawaive").await;
    let (status, waived) = app.post_json(&format!("/api/v1/fines/{fine_id}/waive"), &json!({ "notes": "remise gracieuse" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "waive: {waived}");
    assert_eq!(waived["status"], "waived");
    let body = audit_now(&app, &token, "fine.waived", fine_id).await;
    assert_audit_now(&body, "fine.waived");
    assert_eq!(fine_status(&app, &token, reader_id).await, "waived");
}

#[tokio::test]
async fn cancellation_audit_exists_when_lost_found_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (_reader_id, item_id, fine_id) = lost_fine(&app, &token, "facancel").await;
    let (status, resolved) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::OK, "resolve: {resolved}");
    assert_eq!(resolved["fineAction"], "cancelled");
    let cancelled = audit_now(&app, &token, "fine.cancelled", fine_id).await;
    assert_audit_now(&cancelled, "fine.cancelled");
    assert_eq!(cancelled["entries"][0]["payload"]["cancelledAmount"], 1500);
    let refunded = audit_now(&app, &token, "fine.refunded", fine_id).await;
    assert!(audit_entry(&refunded, "fine.refunded").is_none());
}

#[tokio::test]
async fn refund_audit_exists_when_lost_found_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (_reader_id, item_id, fine_id) = lost_fine(&app, &token, "farefund").await;
    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "15.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "pay: {paid}");
    let (status, resolved) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::OK, "resolve: {resolved}");
    assert_eq!(resolved["fineAction"], "refunded");
    let refunded = audit_now(&app, &token, "fine.refunded", fine_id).await;
    assert_audit_now(&refunded, "fine.refunded");
    assert_eq!(refunded["entries"][0]["payload"]["refundedAmount"], 1500);
}

#[tokio::test]
async fn partial_settlement_audits_exist_when_lost_found_returns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (_reader_id, item_id, fine_id) = lost_fine(&app, &token, "fapartial").await;
    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "4.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "partial pay: {paid}");
    let (status, resolved) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::OK, "resolve: {resolved}");
    assert_eq!(resolved["fineAction"], "partial");
    let refunded = audit_now(&app, &token, "fine.refunded", fine_id).await;
    assert_audit_now(&refunded, "fine.refunded");
    assert_eq!(refunded["entries"][0]["payload"]["refundedAmount"], 400);
    let cancelled = audit_now(&app, &token, "fine.cancelled", fine_id).await;
    assert_audit_now(&cancelled, "fine.cancelled");
    assert_eq!(cancelled["entries"][0]["payload"]["cancelledAmount"], 1100);
}

#[tokio::test]
async fn failed_payment_audit_rolls_back_the_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fapayrb").await;
    app.state.services.audit.fail_next_transactional_insert();
    let (status, body) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "4.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "pay: {body}");
    assert_eq!(fine_status(&app, &token, reader_id).await, "pending");
    let paid = audit_now(&app, &token, "fine.paid", fine_id).await;
    assert!(audit_entry(&paid, "fine.paid").is_none(), "audit: {paid}");

    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "4.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "retry pay: {paid}");
    let body = audit_now(&app, &token, "fine.paid", fine_id).await;
    assert_audit_now(&body, "fine.paid");
}

#[tokio::test]
async fn failed_waiver_audit_rolls_back_the_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, _item_id, fine_id) = lost_fine(&app, &token, "fawaiverb").await;
    app.state.services.audit.fail_next_transactional_insert();
    let (status, body) = app.post_json(&format!("/api/v1/fines/{fine_id}/waive"), &json!({ "notes": "remise" }), Some(&token)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "waive: {body}");
    assert_eq!(fine_status(&app, &token, reader_id).await, "pending");
    let waived = audit_now(&app, &token, "fine.waived", fine_id).await;
    assert!(audit_entry(&waived, "fine.waived").is_none(), "audit: {waived}");
}

#[tokio::test]
async fn failed_cancel_audit_rolls_back_the_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, item_id, fine_id) = lost_fine(&app, &token, "facancelrb").await;
    app.state.services.audit.fail_next_transactional_insert();
    let (status, body) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "resolve: {body}");
    assert!(item_lost(&app, &token, item_id).await, "item must stay lost");
    assert_eq!(fine_status(&app, &token, reader_id).await, "pending");
    let cancelled = audit_now(&app, &token, "fine.cancelled", fine_id).await;
    assert!(audit_entry(&cancelled, "fine.cancelled").is_none(), "audit: {cancelled}");

    let (status, resolved) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::OK, "retry resolve: {resolved}");
    let cancelled = audit_now(&app, &token, "fine.cancelled", fine_id).await;
    assert_audit_now(&cancelled, "fine.cancelled");
}

#[tokio::test]
async fn failed_refund_audit_rolls_back_the_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = admin(&app).await;
    let (reader_id, item_id, fine_id) = lost_fine(&app, &token, "farefundrb").await;
    let (status, paid) = app.post_json(&format!("/api/v1/fines/{fine_id}/pay"), &json!({ "amount": "15.00" }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "pay: {paid}");
    app.state.services.audit.fail_next_transactional_insert();
    let (status, body) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "resolve: {body}");
    assert!(item_lost(&app, &token, item_id).await, "item must stay lost");
    assert_eq!(fine_status(&app, &token, reader_id).await, "paid");
    let refunded = audit_now(&app, &token, "fine.refunded", fine_id).await;
    assert!(audit_entry(&refunded, "fine.refunded").is_none(), "audit: {refunded}");

    let (status, resolved) = resolve(&app, &token, item_id).await;
    assert_eq!(status, StatusCode::OK, "retry resolve: {resolved}");
    let refunded = audit_now(&app, &token, "fine.refunded", fine_id).await;
    assert_audit_now(&refunded, "fine.refunded");
}
