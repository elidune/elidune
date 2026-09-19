//! Patron purchase suggestions → acquisitions order intents (part of #55).
//! Accept opens a draft purchase-order line (title/author intent). It must not
//! create a biblio or an item. Refuse must not create an order.

mod common;

use axum::http::StatusCode;
use common::{fixtures, TestApp};
use serde_json::json;

async fn table_count(app: &TestApp, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*)::bigint FROM {table}");
    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap_or_else(|e| panic!("count {table}: {e}"))
}

async fn propose(
    app: &TestApp,
    token: &str,
    title: &str,
    author: &str,
    comment: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut body = json!({ "title": title, "author": author });
    if let Some(comment) = comment {
        body["comment"] = json!(comment);
    }
    app.post_json("/api/v1/suggestions", &body, Some(token))
        .await
}

#[tokio::test]
async fn patron_can_propose_title() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_patron").await;
    let suffix = fixtures::unique_suffix();

    let (status, body) = propose(
        &app,
        &reader_token,
        &format!("The Left Hand of Darkness {suffix}"),
        "Ursula K. Le Guin",
        Some("Please consider for the SF shelf"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "propose: {body}");
    assert_eq!(body["status"], "proposed");
    assert_eq!(body["title"], format!("The Left Hand of Darkness {suffix}"));
    assert_eq!(body["author"], "Ursula K. Le Guin");
    assert_eq!(body["comment"], "Please consider for the SF shelf");
    assert!(body["purchaseOrderId"].is_null(), "{body}");
    assert!(body["purchaseOrderLineId"].is_null(), "{body}");
}

#[tokio::test]
async fn reader_cannot_accept_or_refuse() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_forbid").await;
    let suffix = fixtures::unique_suffix();

    let (status, created) = propose(
        &app,
        &reader_token,
        &format!("Forbidden title {suffix}"),
        "A. Reader",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/suggestions/{id}/accept"),
            &json!({}),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader accept: {body}");

    let (status, body) = app
        .post_json(
            &format!("/api/v1/suggestions/{id}/refuse"),
            &json!({ "staffNote": "no" }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader refuse: {body}");
}

#[tokio::test]
async fn staff_lists_suggestions_patrons_see_own() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_a) = fixtures::create_reader(&app, &admin_token, "sug_a").await;
    let (_, reader_b) = fixtures::create_reader(&app, &admin_token, "sug_b").await;
    let suffix = fixtures::unique_suffix();
    let title_a = format!("Title A {suffix}");
    let title_b = format!("Title B {suffix}");

    let (status, a) = propose(&app, &reader_a, &title_a, "Author A", None).await;
    assert_eq!(status, StatusCode::CREATED, "{a}");
    let (status, b) = propose(&app, &reader_b, &title_b, "Author B", None).await;
    assert_eq!(status, StatusCode::CREATED, "{b}");

    let (status, mine) = app
        .get_json_with_auth("/api/v1/suggestions", &reader_a)
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    let mine_items = mine["suggestions"].as_array().expect("suggestions array");
    assert!(mine_items.iter().any(|s| s["title"] == title_a), "{mine}");
    assert!(
        mine_items.iter().all(|s| s["title"] != title_b),
        "patron must not see another patron's suggestion: {mine}"
    );

    let (status, staff) = app
        .get_json_with_auth("/api/v1/suggestions", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff}");
    let staff_items = staff["suggestions"].as_array().expect("suggestions array");
    assert!(
        staff_items.iter().any(|s| s["title"] == title_a)
            && staff_items.iter().any(|s| s["title"] == title_b),
        "staff list must include both: {staff}"
    );
}

#[tokio::test]
async fn accept_opens_order_intent_without_biblio_or_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_accept").await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Intent only {suffix}");

    let biblios_before = table_count(&app, "biblios").await;
    let items_before = table_count(&app, "items").await;
    let orders_before = table_count(&app, "purchase_orders").await;
    let lines_before = table_count(&app, "purchase_order_lines").await;

    let (status, created) = propose(
        &app,
        &reader_token,
        &title,
        "Intent Author",
        Some("wishlist"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, accepted) = app
        .post_json(
            &format!("/api/v1/suggestions/{id}/accept"),
            &json!({ "staffNote": "Will order next week" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "accept: {accepted}");
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["staffNote"], "Will order next week");
    let order_id = fixtures::json_id(&accepted["purchaseOrderId"]);
    let line_id = fixtures::json_id(&accepted["purchaseOrderLineId"]);

    let biblios_after = table_count(&app, "biblios").await;
    let items_after = table_count(&app, "items").await;
    assert_eq!(
        biblios_after, biblios_before,
        "accept must not create a bibliographic record"
    );
    assert_eq!(
        items_after, items_before,
        "accept must not create an item / received copy"
    );
    assert_eq!(
        table_count(&app, "purchase_orders").await,
        orders_before + 1
    );
    assert_eq!(
        table_count(&app, "purchase_order_lines").await,
        lines_before + 1
    );

    let (status, order) = app
        .get_json_with_auth(
            &format!("/api/v1/acquisitions/orders/{order_id}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{order}");
    assert_eq!(order["order"]["status"], "draft", "{order}");
    assert!(
        order["order"]["vendorId"].is_null(),
        "intent is not yet assigned to a vendor: {order}"
    );
    let line = order["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .find(|l| fixtures::json_id(&l["id"]) == line_id)
        .expect("accepted line");
    assert_eq!(line["title"], title);
    assert!(line["biblioId"].is_null(), "no biblio on intent: {line}");
    assert_eq!(line["quantityReceived"], 0);
    assert_eq!(line["quantityOrdered"], 1);
}

#[tokio::test]
async fn refuse_does_not_create_order_intent() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_refuse").await;
    let suffix = fixtures::unique_suffix();

    let orders_before = table_count(&app, "purchase_orders").await;
    let lines_before = table_count(&app, "purchase_order_lines").await;
    let biblios_before = table_count(&app, "biblios").await;
    let items_before = table_count(&app, "items").await;

    let (status, created) = propose(
        &app,
        &reader_token,
        &format!("Refused title {suffix}"),
        "No Order",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, refused) = app
        .post_json(
            &format!("/api/v1/suggestions/{id}/refuse"),
            &json!({ "staffNote": "Already in stock" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "refuse: {refused}");
    assert_eq!(refused["status"], "refused");
    assert_eq!(refused["staffNote"], "Already in stock");
    assert!(refused["purchaseOrderId"].is_null(), "{refused}");
    assert!(refused["purchaseOrderLineId"].is_null(), "{refused}");

    assert_eq!(table_count(&app, "purchase_orders").await, orders_before);
    assert_eq!(
        table_count(&app, "purchase_order_lines").await,
        lines_before
    );
    assert_eq!(table_count(&app, "biblios").await, biblios_before);
    assert_eq!(table_count(&app, "items").await, items_before);

    let (status, again) = app
        .post_json(
            &format!("/api/v1/suggestions/{id}/accept"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "refused stays refused: {again}"
    );
    assert_eq!(table_count(&app, "purchase_orders").await, orders_before);
}

#[tokio::test]
async fn create_requires_title_and_author() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_valid").await;

    let (status, body) = app
        .post_json(
            "/api/v1/suggestions",
            &json!({ "author": "No Title" }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = app
        .post_json(
            "/api/v1/suggestions",
            &json!({ "title": "No Author" }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
