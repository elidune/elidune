//! Lost / damaged / claimed-returned desk workflows (#14).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

fn decimal_str(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_f64().map(|n| format!("{n:.2}")))
        .unwrap_or_else(|| value.to_string().trim_matches('"').to_string())
}

async fn unlock_admin(app: &TestApp, admin_token: &str) {
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", admin_token).await;
    if status != StatusCode::OK {
        return;
    }
    let admin_id = fixtures::json_id(&me["id"]);
    let _ = app
        .state
        .services
        .users
        .set_must_change_password(admin_id, false)
        .await;
}

async fn create_loan_with_priced_item(
    app: &TestApp,
    admin_token: &str,
    reader_id: i64,
    price: &str,
) -> (i64, i64) {
    let biblio_payload = json!({
        "title": format!("Exception Book {}", fixtures::unique_suffix()),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": format!("EX-{}", fixtures::unique_suffix()),
            "borrowable": true,
            "price": price
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &biblio_payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);

    let (loan_status, loan_body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(loan_status, StatusCode::CREATED, "checkout: {loan_body}");
    (fixtures::json_id(&loan_body["id"]), item_id)
}

async fn item_copy(app: &TestApp, admin_token: &str, item_id: i64) -> serde_json::Value {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "get item: {body}");
    body["items"][0].clone()
}

async fn audit_has(app: &TestApp, admin_token: &str, event_type: &str) -> bool {
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/audit?eventType={event_type}&perPage=50"),
            admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "audit: {body}");
    body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|e| e["eventType"] == event_type)
}

#[tokio::test]
async fn mark_lost_closes_loan_bills_item_price_and_blocks_checkout() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "lostreader").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "lostother").await;
    let (loan_id, item_id) =
        create_loan_with_priced_item(&app, &admin_token, reader_id, "15.00").await;

    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true, "notes": "declared lost at desk" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {body}");
    assert_eq!(body["outcome"], "lost");
    assert_eq!(body["itemStatus"], "lost");
    assert_eq!(body["borrowable"], false);
    assert_eq!(body["loanClosed"], true);
    assert_eq!(body["charge"]["chargeType"], "replacement");
    assert_eq!(decimal_str(&body["charge"]["amount"]), "15.00");

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["borrowable"], false);
    assert_eq!(item["circulationStatus"], 1);
    assert_eq!(item["borrowed"], false);

    let (loans_status, loans_body) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &admin_token)
        .await;
    assert_eq!(loans_status, StatusCode::OK);
    assert_eq!(loans_body["items"].as_array().map(Vec::len), Some(0));

    let (checkout_status, checkout_body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": other_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(checkout_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        checkout_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("lost"),
        "checkout block: {checkout_body}"
    );

    assert!(
        audit_has(&app, &admin_token, "item.marked_lost").await,
        "lost audit event"
    );
    assert!(
        audit_has(&app, &admin_token, "fine.created").await,
        "replacement charge audit"
    );
}

#[tokio::test]
async fn mark_damaged_return_vs_keep_and_damage_charge() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "dmgreader").await;
    let (keep_reader_id, _) = fixtures::create_reader(&app, &admin_token, "dmgkeep").await;

    let (return_loan_id, return_item_id) =
        create_loan_with_priced_item(&app, &admin_token, reader_id, "20.00").await;
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{return_loan_id}/damaged"),
            &json!({
                "disposition": "return",
                "bill": true,
                "amount": "5.00",
                "notes": "spine broken"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "damaged return: {body}");
    assert_eq!(body["outcome"], "damaged");
    assert_eq!(body["loanClosed"], true);
    assert_eq!(body["borrowable"], false);
    assert_eq!(body["charge"]["chargeType"], "damage");
    assert_eq!(decimal_str(&body["charge"]["amount"]), "5.00");
    let returned = item_copy(&app, &admin_token, return_item_id).await;
    assert_eq!(returned["circulationStatus"], 2);
    assert_eq!(returned["borrowable"], false);

    let (keep_loan_id, keep_item_id) =
        create_loan_with_priced_item(&app, &admin_token, keep_reader_id, "20.00").await;
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{keep_loan_id}/damaged"),
            &json!({ "disposition": "keep", "notes": "keep charged" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "damaged keep: {body}");
    assert_eq!(body["loanClosed"], false);
    assert!(body["charge"].is_null());
    let kept = item_copy(&app, &admin_token, keep_item_id).await;
    assert_eq!(kept["circulationStatus"], 2);
    assert_eq!(kept["borrowed"], true);

    let (loans_status, loans_body) = app
        .get_json_with_auth(
            &format!("/api/v1/users/{keep_reader_id}/loans"),
            &admin_token,
        )
        .await;
    assert_eq!(loans_status, StatusCode::OK);
    let active: Vec<_> = loans_body["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| fixtures::json_id(&l["id"]) == keep_loan_id)
        .collect();
    assert_eq!(active.len(), 1, "keep disposition leaves the loan open");

    assert!(audit_has(&app, &admin_token, "item.marked_damaged").await);
}

#[tokio::test]
async fn claimed_returned_stays_open_until_inventory_check() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "claimreader").await;
    let (loan_id, item_id) =
        create_loan_with_priced_item(&app, &admin_token, reader_id, "9.99").await;

    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claimed-returned"),
            &json!({ "notes": "patron says already returned" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "claimed: {body}");
    assert_eq!(body["outcome"], "claimedReturned");
    assert_eq!(body["itemStatus"], "claimedReturned");
    assert_eq!(body["loanClosed"], false);
    assert_eq!(body["borrowable"], false);
    assert!(body["charge"].is_null());

    let (bill_status, bill_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claimed-returned"),
            &json!({ "bill": true, "notes": "must not charge" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(bill_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        bill_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("investigation"),
        "claimed-returned must refuse billing: {bill_body}"
    );

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["circulationStatus"], 3);
    assert_eq!(item["borrowed"], true);

    let (queue_status, queue_body) = app
        .get_json_with_auth("/api/v1/loans/claims-returned", &admin_token)
        .await;
    assert_eq!(queue_status, StatusCode::OK, "queue: {queue_body}");
    let queued = queue_body["items"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|row| fixtures::json_id(&row["loanId"]) == loan_id);
    assert!(queued, "loan must appear on the claims-returned queue");

    let (silent_status, silent_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claims-returned/resolve"),
            &json!({ "outcome": "found", "inventoryChecked": false }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(silent_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        silent_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("inventory"),
        "silent clear blocked: {silent_body}"
    );

    let (renew_status, _) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(renew_status, StatusCode::UNPROCESSABLE_ENTITY);

    let (found_bill_status, found_bill_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claims-returned/resolve"),
            &json!({
                "outcome": "found",
                "inventoryChecked": true,
                "bill": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(found_bill_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        found_bill_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("investigation"),
        "found must not bill: {found_bill_body}"
    );

    let (found_status, found_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claims-returned/resolve"),
            &json!({ "outcome": "found", "inventoryChecked": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(found_status, StatusCode::OK, "resolve found: {found_body}");
    assert_eq!(found_body["outcome"], "claimsResolvedFound");
    assert_eq!(found_body["itemStatus"], "available");
    assert_eq!(found_body["loanClosed"], true);
    assert_eq!(found_body["borrowable"], true);

    let restored = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(restored["circulationStatus"], 0);
    assert_eq!(restored["borrowable"], true);
    assert_eq!(restored["borrowed"], false);

    assert!(audit_has(&app, &admin_token, "item.claimed_returned").await);
    assert!(audit_has(&app, &admin_token, "item.claims_returned_resolved").await);
}

#[tokio::test]
async fn claimed_returned_not_found_marks_lost_and_can_bill() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "notfoundr").await;
    let (loan_id, item_id) =
        create_loan_with_priced_item(&app, &admin_token, reader_id, "11.00").await;

    let (status, _) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claimed-returned"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claims-returned/resolve"),
            &json!({
                "outcome": "notFound",
                "inventoryChecked": true,
                "bill": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "resolve not found: {body}");
    assert_eq!(body["outcome"], "claimsResolvedNotFound");
    assert_eq!(body["itemStatus"], "lost");
    assert_eq!(body["loanClosed"], true);
    assert_eq!(body["charge"]["chargeType"], "replacement");
    assert_eq!(decimal_str(&body["charge"]["amount"]), "11.00");

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["circulationStatus"], 1);
    assert_eq!(item["borrowable"], false);
}

#[tokio::test]
async fn replacement_charge_coexists_with_open_overdue_fine() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "bothfines").await;
    let (loan_id, _) = create_loan_with_priced_item(&app, &admin_token, reader_id, "8.00").await;

    app.state
        .services
        .repository
        .fines_create(
            loan_id,
            reader_id,
            rust_decimal::Decimal::new(250, 2),
            Some("overdue"),
        )
        .await
        .expect("overdue fine");

    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true, "amount": "8.00" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "lost with overdue: {body}");
    assert_eq!(body["charge"]["chargeType"], "replacement");

    let (fines_status, fines_body) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &admin_token)
        .await;
    assert_eq!(fines_status, StatusCode::OK, "fines: {fines_body}");
    let fines = fines_body["fines"].as_array().expect("fines list");
    assert_eq!(fines.len(), 2, "overdue + replacement: {fines_body}");
    let types: Vec<_> = fines
        .iter()
        .filter_map(|f| f["chargeType"].as_str())
        .collect();
    assert!(types.contains(&"overdue"));
    assert!(types.contains(&"replacement"));
}

#[tokio::test]
async fn physical_return_clears_claims_returned_without_resolve_endpoint() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "retclaim").await;
    let (loan_id, item_id) =
        create_loan_with_priced_item(&app, &admin_token, reader_id, "4.00").await;

    let (status, _) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/claimed-returned"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (return_status, return_body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(return_status, StatusCode::OK, "return: {return_body}");

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["circulationStatus"], 0);
    assert_eq!(item["borrowable"], true);
}
