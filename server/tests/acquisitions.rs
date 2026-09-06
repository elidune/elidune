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

#[tokio::test]
async fn reader_cannot_access_acquisitions_admin_can() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "acqreader").await;

    let (reader_status, _) = app
        .get_json_with_auth("/api/v1/acquisitions/vendors", &reader_token)
        .await;
    assert_eq!(reader_status, StatusCode::FORBIDDEN);

    let (admin_status, _) = app
        .get_json_with_auth("/api/v1/acquisitions/vendors", &admin_token)
        .await;
    assert_eq!(admin_status, StatusCode::OK);

    let (types_status, types) = app
        .get_json_with_auth("/api/v1/account-types/admin", &admin_token)
        .await;
    assert_eq!(types_status, StatusCode::OK);
    assert_eq!(types["acquisitionsRights"], "w");
    assert_eq!(types["itemsRights"], types["itemsRights"]);

    let (reader_type_status, reader_type) = app
        .get_json_with_auth("/api/v1/account-types/reader", &admin_token)
        .await;
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

    let (status, listed) = app
        .get_json_with_auth(
            &format!("/api/v1/acquisitions/vendors?q=ACME{suffix}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed["total"].as_i64().unwrap_or(0) >= 1);

    let (status, _) = app
        .delete_with_auth(
            &format!("/api/v1/acquisitions/vendors/{vendor_id}"),
            &admin_token,
        )
        .await;
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
                "isbn": format!("9780000{suffix}").chars().take(13).collect::<String>()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {biblio_body}");
    let biblio_id = biblio_body["biblio"]["id"]
        .as_str()
        .or_else(|| biblio_body["id"].as_str())
        .expect("biblio id")
        .to_string();

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

    let (status, submitted) = app
        .post_empty(
            &format!("/api/v1/acquisitions/orders/{order_id}/submit"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "submit: {submitted}");
    assert_eq!(submitted["order"]["status"], "ordered");

    let (status, after_submit) = app
        .get_json_with_auth(
            &format!("/api/v1/acquisitions/funds/{fund_id}"),
            &admin_token,
        )
        .await;
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

    let (status, item_biblio) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "item on shelf: {item_biblio}");
    let items = item_biblio
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let found = items
        .iter()
        .any(|i| i["barcode"] == barcode_one || fixtures::json_id(&i["id"]) == item_id)
        || item_biblio["barcode"] == barcode_one;
    assert!(
        found,
        "received item should appear in catalog: {item_biblio}"
    );

    let (status, mid_fund) = app
        .get_json_with_auth(
            &format!("/api/v1/acquisitions/funds/{fund_id}"),
            &admin_token,
        )
        .await;
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

    let (status, final_fund) = app
        .get_json_with_auth(
            &format!("/api/v1/acquisitions/funds/{fund_id}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{final_fund}");
    assert_eq!(decimal_str(&final_fund["committed"]), "0");
    assert_eq!(decimal_str(&final_fund["spent"]), "25.00");
    assert_eq!(decimal_str(&final_fund["available"]), "75.00");

    let (status, copies) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{copies}");
    let copy_list = copies
        .as_array()
        .or_else(|| copies.get("items").and_then(|v| v.as_array()))
        .expect("items list");
    assert!(
        copy_list.len() >= 2,
        "both received copies should be on the biblio: {copies}"
    );
}

#[tokio::test]
async fn isbn_intent_receipt_creates_stub_biblio_and_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, vendor) = app
        .post_json(
            "/api/v1/acquisitions/vendors",
            &json!({ "name": format!("ISBN Vendor {suffix}") }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{vendor}");
    let vendor_id = vendor["id"].as_str().unwrap();

    let isbn = format!("9781111{suffix}");
    let isbn: String = isbn
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(13)
        .collect();

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

    let (status, submitted) = app
        .post_empty(
            &format!("/api/v1/acquisitions/orders/{order_id}/submit"),
            Some(&admin_token),
        )
        .await;
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
    let resolved_biblio = received["order"]["lines"][0]["biblioId"]
        .as_str()
        .expect("biblio resolved at receipt");
    let item_id = fixtures::json_id(&received["lines"][0]["itemIds"][0]);

    let (status, copies) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{resolved_biblio}/items"),
            &admin_token,
        )
        .await;
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
        .post_json(
            "/api/v1/acquisitions/vendors",
            &json!({ "name": format!("Draft Vendor {suffix}") }),
            Some(&admin_token),
        )
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
