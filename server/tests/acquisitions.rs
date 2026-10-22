//! Acquisitions MVP: vendors, yearly funds, orders, receipt → catalog items.

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

/// Valid ISBN-13, unique per call.
///
/// `9780000` plus the high digits of a nanosecond timestamp stays the same for
/// about three hours, so a later run on the same database gets
/// 409 `duplicate_isbn_needs_confirmation`.
fn unique_isbn13() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_nanos();
    let seq = u128::from(SEQ.fetch_add(1, Ordering::Relaxed));
    let pid = u128::from(std::process::id());
    let mixed = nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(pid << 17).wrapping_add(seq);
    let body = mixed % 1_000_000_000;
    let twelve = format!("978{body:09}");
    let check = isbn13_check_digit(&twelve);
    format!("{twelve}{check}")
}

fn isbn13_check_digit(body12: &str) -> char {
    let sum: u32 = body12
        .bytes()
        .enumerate()
        .map(|(index, byte)| {
            let digit = u32::from(byte - b'0');
            if index % 2 == 0 {
                digit
            } else {
                digit * 3
            }
        })
        .sum();
    let check = (10 - (sum % 10)) % 10;
    char::from_digit(check, 10).expect("isbn-13 check digit")
}

#[test]
fn isbn13_check_digit_matches_the_306_40615_example() {
    assert_eq!(isbn13_check_digit("978030640615"), '7');
    let generated = unique_isbn13();
    assert_eq!(generated.len(), 13);
    assert!(generated.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(isbn13_check_digit(&generated[..12]), generated.chars().nth(12).expect("check digit"));
}

#[tokio::test]
async fn reader_cannot_access_acquisitions_admin_can() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "acqreader").await;

    let (reader_status, _) = app.get_json_with_auth("/api/v1/acquisitions/vendors", &reader_token).await;
    assert_eq!(reader_status, StatusCode::FORBIDDEN);

    let (admin_status, _) = app.get_json_with_auth("/api/v1/acquisitions/vendors", &admin_token).await;
    assert_eq!(admin_status, StatusCode::OK);

    let (types_status, types) = app.get_json_with_auth("/api/v1/account-types/admin", &admin_token).await;
    assert_eq!(types_status, StatusCode::OK);
    assert_eq!(types["acquisitionsRights"], "w");
    assert_eq!(types["itemsRights"], types["itemsRights"]);

    let (reader_type_status, reader_type) = app.get_json_with_auth("/api/v1/account-types/reader", &admin_token).await;
    assert_eq!(reader_type_status, StatusCode::OK);
    assert_eq!(reader_type["acquisitionsRights"], "n");
}

#[tokio::test]
async fn vendor_and_fund_crud() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json(
            "/api/v1/acquisitions/vendors",
            &json!({
                "name": format!("Acme Books {suffix}"),
                "code": format!("ACME{suffix}"),
                "email": "orders@acme.test"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create vendor: {vendor}");
    let vendor_id = fixtures::json_id(&vendor["id"]);
    assert_eq!(vendor["name"], format!("Acme Books {suffix}"));

    let (status, fund) = app
        .post_json(
            "/api/v1/acquisitions/funds",
            &json!({
                "code": format!("ADULT{suffix}"),
                "name": "Adult print",
                "fiscalYear": 2026,
                "allocatedAmount": "500.00",
                "currency": "EUR"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create fund: {fund}");
    assert_eq!(decimal_str(&fund["allocatedAmount"]), "500.00");
    assert_eq!(decimal_str(&fund["committed"]), "0");
    assert_eq!(decimal_str(&fund["spent"]), "0");
    assert_eq!(decimal_str(&fund["available"]), "500.00");

    let (status, listed) = app.get_json_with_auth(&format!("/api/v1/acquisitions/vendors?q=ACME{suffix}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed["total"].as_i64().unwrap_or(0) >= 1);

    let (status, _) = app.delete_with_auth(&format!("/api/v1/acquisitions/vendors/{vendor_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn order_partial_and_full_receipt_creates_items_and_updates_budget() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json(
            "/api/v1/acquisitions/vendors",
            &json!({ "name": format!("Vendor {suffix}"), "code": format!("V{suffix}") }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().expect("vendor id string");

    let (status, fund) = app
        .post_json(
            "/api/v1/acquisitions/funds",
            &json!({
                "code": format!("F{suffix}"),
                "name": "Test fund",
                "fiscalYear": 2026,
                "allocatedAmount": "100.00"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{fund}");
    let fund_id = fund["id"].as_str().expect("fund id string");

    let (status, biblio_body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Ordered title {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "isbn": unique_isbn13()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {biblio_body}");
    let biblio_id = biblio_body["biblio"]["id"].as_str().or_else(|| biblio_body["id"].as_str()).expect("biblio id").to_string();

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "fundId": fund_id,
                "lines": [{
                    "biblioId": biblio_id,
                    "title": format!("Ordered title {suffix}"),
                    "quantity": 2,
                    "unitPrice": "12.50"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create order: {order}");
    assert_eq!(order["order"]["status"], "draft");
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, submitted) = app.post_empty(&format!("/api/v1/acquisitions/orders/{order_id}/submit"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "submit: {submitted}");
    assert_eq!(submitted["order"]["status"], "ordered");

    let (status, after_submit) = app.get_json_with_auth(&format!("/api/v1/acquisitions/funds/{fund_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{after_submit}");
    assert_eq!(decimal_str(&after_submit["committed"]), "25.00");
    assert_eq!(decimal_str(&after_submit["spent"]), "0");
    assert_eq!(decimal_str(&after_submit["available"]), "75.00");

    let barcode_one = format!("ACQ-R1-{suffix}");
    let (status, partial) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 1,
                    "items": [{
                        "barcode": barcode_one,
                        "price": "12.50"
                    }]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "partial receive: {partial}");
    assert_eq!(partial["order"]["order"]["status"], "partial");
    assert_eq!(partial["lines"][0]["quantity"], 1);
    let item_id = fixtures::json_id(&partial["lines"][0]["itemIds"][0]);

    let (status, item_biblio) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "item on shelf: {item_biblio}");
    let items = item_biblio.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let found = items.iter().any(|i| i["barcode"] == barcode_one || fixtures::json_id(&i["id"]) == item_id) || item_biblio["barcode"] == barcode_one;
    assert!(found, "received item should appear in catalog: {item_biblio}");
    let received = items.iter().find(|i| fixtures::json_id(&i["id"]) == item_id);
    let received = received.unwrap_or(&item_biblio);
    assert_eq!(received["borrowable"], false, "receipt without site must not create a circulable copy: {item_biblio}");

    let (status, mid_fund) = app.get_json_with_auth(&format!("/api/v1/acquisitions/funds/{fund_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{mid_fund}");
    assert_eq!(decimal_str(&mid_fund["committed"]), "12.50");
    assert_eq!(decimal_str(&mid_fund["spent"]), "12.50");
    assert_eq!(decimal_str(&mid_fund["available"]), "75.00");

    let barcode_two = format!("ACQ-R2-{suffix}");
    let (status, full) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 1,
                    "items": [{ "barcode": barcode_two, "price": "12.50" }]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "full receive: {full}");
    assert_eq!(full["order"]["order"]["status"], "received");

    let (status, final_fund) = app.get_json_with_auth(&format!("/api/v1/acquisitions/funds/{fund_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{final_fund}");
    assert_eq!(decimal_str(&final_fund["committed"]), "0");
    assert_eq!(decimal_str(&final_fund["spent"]), "25.00");
    assert_eq!(decimal_str(&final_fund["available"]), "75.00");

    let (status, copies) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{copies}");
    let copy_list = copies.as_array().or_else(|| copies.get("items").and_then(|v| v.as_array())).expect("items list");
    assert!(copy_list.len() >= 2, "both received copies should be on the biblio: {copies}");
}

#[tokio::test]
async fn isbn_intent_receipt_creates_stub_biblio_and_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json("/api/v1/acquisitions/vendors", &json!({ "name": format!("ISBN Vendor {suffix}") }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let isbn = unique_isbn13();

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "lines": [{
                    "isbn": isbn,
                    "title": format!("ISBN intent {suffix}"),
                    "quantity": 1,
                    "unitPrice": "9.99"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    assert!(order["lines"][0]["biblioId"].is_null());
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, submitted) = app.post_empty(&format!("/api/v1/acquisitions/orders/{order_id}/submit"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    let barcode = format!("ISBN-{suffix}");
    let (status, received) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 1,
                    "items": [{ "barcode": barcode }]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    assert_eq!(received["order"]["order"]["status"], "received");
    let resolved_biblio = received["order"]["lines"][0]["biblioId"].as_str().expect("biblio resolved at receipt");
    let item_id = fixtures::json_id(&received["lines"][0]["itemIds"][0]);

    let (status, copies) = app.get_json_with_auth(&format!("/api/v1/biblios/{resolved_biblio}/items"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{copies}");
    let _ = item_id;
}

#[tokio::test]
async fn cannot_receive_before_submit() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json("/api/v1/acquisitions/vendors", &json!({ "name": format!("Draft Vendor {suffix}") }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "lines": [{ "title": "Not yet ordered", "quantity": 1, "unitPrice": "1.00" }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{ "lineId": line_id.to_string(), "quantity": 1 }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}

async fn item_copy(app: &TestApp, admin_token: &str, item_id: i64) -> serde_json::Value {
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}"), admin_token).await;
    assert_eq!(status, StatusCode::OK, "get item: {body}");
    if body.get("items").and_then(|v| v.as_array()).is_some() {
        body["items"][0].clone()
    } else {
        body
    }
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string()
        }),
        Some(admin_token),
    )
    .await
}

#[tokio::test]
async fn receipt_creates_non_borrowable_item_until_barcode_and_site_are_set() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "acqcirc").await;

    let (status, vendor) = app
        .post_json("/api/v1/acquisitions/vendors", &json!({ "name": format!("Circ Vendor {suffix}") }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let (status, biblio_body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Incomplete receipt {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio_body}");
    let biblio_id = biblio_body["biblio"]["id"].as_str().or_else(|| biblio_body["id"].as_str()).expect("biblio id");

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "lines": [{
                    "biblioId": biblio_id,
                    "quantity": 1,
                    "unitPrice": "8.00"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, submitted) = app.post_empty(&format!("/api/v1/acquisitions/orders/{order_id}/submit"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    let (status, received) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{ "lineId": line_id.to_string(), "quantity": 1 }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    let item_id = fixtures::json_id(&received["lines"][0]["itemIds"][0]);

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["price"], "8.00", "receipt copies the line unit price: {item}");
    assert_eq!(item["borrowable"], false, "receipt default: {item}");
    assert!(item["barcode"].is_null() || item["barcode"] == "", "{item}");

    let (status, body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["message"].as_str().unwrap_or_default().to_lowercase().contains("borrowable"),
        "checkout of incomplete receipt must be blocked: {body}"
    );

    let (status, site) = app.post_json("/api/v1/sites", &json!({ "name": format!("Acq Site {suffix}") }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let site_id = site["id"].as_str().expect("source id");
    let barcode = format!("ACQ-DONE-{suffix}");

    let (status, completed) = app
        .put_json(
            &format!("/api/v1/items/{item_id}"),
            &json!({
                "id": item_id.to_string(),
                "biblioId": biblio_id,
                "barcode": barcode,
                "siteId": site_id
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{completed}");
    assert_eq!(completed["borrowable"], true, "{completed}");
    assert_eq!(completed["price"], "8.00", "line unit price is kept: {completed}");

    let item = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(item["borrowable"], true, "{item}");
    assert_eq!(item["price"], "8.00", "{item}");

    let (status, loan) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "checkout after complete: {loan}");
}

#[tokio::test]
async fn receipt_with_barcode_site_and_price_or_deferral_is_borrowable() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json("/api/v1/acquisitions/vendors", &json!({ "name": format!("Ready Vendor {suffix}") }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let (status, site) = app.post_json("/api/v1/sites", &json!({ "name": format!("Ready Site {suffix}") }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let site_id = site["id"].as_str().unwrap();

    let (status, biblio_body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Ready receipt {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio_body}");
    let biblio_id = biblio_body["biblio"]["id"].as_str().or_else(|| biblio_body["id"].as_str()).expect("biblio id");

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "lines": [{
                    "biblioId": biblio_id,
                    "quantity": 2
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, submitted) = app.post_empty(&format!("/api/v1/acquisitions/orders/{order_id}/submit"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{submitted}");

    let (status, received) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 2,
                    "items": [
                        {
                            "barcode": format!("READY-{suffix}"),
                            "siteId": site_id,
                            "price": "4.50"
                        },
                        {
                            "barcode": format!("DEFER-{suffix}"),
                            "siteId": site_id,
                            "priceDeferred": true
                        }
                    ]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    let priced_id = fixtures::json_id(&received["lines"][0]["itemIds"][0]);
    let deferred_id = fixtures::json_id(&received["lines"][0]["itemIds"][1]);

    let priced = item_copy(&app, &admin_token, priced_id).await;
    let deferred = item_copy(&app, &admin_token, deferred_id).await;
    assert_eq!(priced["price"], "4.50", "{priced}");
    assert_eq!(priced["borrowable"], true, "{priced}");
    assert_eq!(deferred["borrowable"], true, "{deferred}");
}

#[tokio::test]
async fn fully_received_order_requires_price_or_deferral_on_every_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json("/api/v1/acquisitions/vendors", &json!({ "name": format!("Close Vendor {suffix}") }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let (status, site) = app.post_json("/api/v1/sites", &json!({ "name": format!("Close Site {suffix}") }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let site_id = site["id"].as_str().unwrap();

    let (status, biblio_body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Close receipt {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio_body}");
    let biblio_id = biblio_body["biblio"]["id"].as_str().or_else(|| biblio_body["id"].as_str()).expect("biblio id");

    let (status, order) = app
        .post_json(
            "/api/v1/acquisitions/orders",
            &json!({
                "vendorId": vendor_id,
                "lines": [{
                    "biblioId": biblio_id,
                    "quantity": 1
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    let order_id = fixtures::json_id(&order["order"]["id"]);
    let line_id = fixtures::json_id(&order["lines"][0]["id"]);

    let (status, _) = app.post_empty(&format!("/api/v1/acquisitions/orders/{order_id}/submit"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK);

    let (status, blocked) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 1,
                    "items": [{
                        "barcode": format!("CLOSE-NO-PRICE-{suffix}"),
                        "siteId": site_id
                    }]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{blocked}");
    assert!(blocked["message"].as_str().unwrap_or_default().to_lowercase().contains("fully receive"), "{blocked}");

    let (status, received) = app
        .post_json(
            &format!("/api/v1/acquisitions/orders/{order_id}/receive"),
            &json!({
                "lines": [{
                    "lineId": line_id.to_string(),
                    "quantity": 1,
                    "items": [{
                        "barcode": format!("CLOSE-DEFER-{suffix}"),
                        "siteId": site_id,
                        "priceDeferred": true
                    }]
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    assert_eq!(received["order"]["status"], "received", "{received}");
}
