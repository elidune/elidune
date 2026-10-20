//! Patron purchase suggestions → acquisitions order intents (part of #55).
//! Accept opens a draft purchase-order line (title/author intent). It must not
//! create a biblio or an item. Refuse must not create an order.

mod common;

use axum::http::StatusCode;
use common::{fixtures, TestApp};
use serde_json::json;

/// Catalog and order rows that belong to one suggestion.
///
/// Parallel integration tests share one database. A global `COUNT(*)` moves
/// when another test inserts, so these counts are keyed by this suggestion's
/// id and title (both unique per run).
#[derive(Debug, PartialEq, Eq)]
struct SuggestionEffects {
    orders: i64,
    lines: i64,
    biblios: i64,
    items: i64,
}

async fn suggestion_effects(app: &TestApp, suggestion_id: i64, title: &str) -> SuggestionEffects {
    let pool = app.state.services.repository.pool();
    let note = format!("Opened from patron purchase suggestion {suggestion_id}");
    let orders = sqlx::query_scalar::<_, i64>("SELECT COUNT(*)::bigint FROM purchase_orders WHERE notes = $1")
        .bind(&note)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count suggestion orders: {e}"));
    let lines = sqlx::query_scalar::<_, i64>("SELECT COUNT(*)::bigint FROM purchase_order_lines WHERE title = $1")
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count suggestion lines: {e}"));
    let biblios = sqlx::query_scalar::<_, i64>("SELECT COUNT(*)::bigint FROM biblios WHERE title = $1")
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("count suggestion biblios: {e}"));
    let items = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)::bigint FROM items i \
         INNER JOIN biblios b ON b.id = i.biblio_id \
         WHERE b.title = $1",
    )
    .bind(title)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("count suggestion items: {e}"));
    SuggestionEffects { orders, lines, biblios, items }
}

async fn propose(app: &TestApp, token: &str, title: &str, author: &str, comment: Option<&str>) -> (StatusCode, serde_json::Value) {
    let mut body = json!({ "title": title, "author": author });
    if let Some(comment) = comment {
        body["comment"] = json!(comment);
    }
    app.post_json("/api/v1/suggestions", &body, Some(token)).await
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

    let (status, created) = propose(&app, &reader_token, &format!("Forbidden title {suffix}"), "A. Reader", None).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, body) = app.post_json(&format!("/api/v1/suggestions/{id}/accept"), &json!({}), Some(&reader_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader accept: {body}");

    let (status, body) = app.post_json(&format!("/api/v1/suggestions/{id}/refuse"), &json!({ "staffNote": "no" }), Some(&reader_token)).await;
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

    let (status, mine) = app.get_json_with_auth("/api/v1/suggestions", &reader_a).await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    let mine_items = mine["suggestions"].as_array().expect("suggestions array");
    assert!(mine_items.iter().any(|s| s["title"] == title_a), "{mine}");
    assert!(mine_items.iter().all(|s| s["title"] != title_b), "patron must not see another patron's suggestion: {mine}");

    let staff_a = app
        .find_paged_row(&admin_token, "/api/v1/suggestions", "suggestions", &format!("suggestion {title_a}"), |row| row["title"] == title_a)
        .await;
    let staff_b = app
        .find_paged_row(&admin_token, "/api/v1/suggestions", "suggestions", &format!("suggestion {title_b}"), |row| row["title"] == title_b)
        .await;
    assert_eq!(staff_a["title"], title_a);
    assert_eq!(staff_b["title"], title_b);
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

    let (status, created) = propose(&app, &reader_token, &title, "Intent Author", Some("wishlist")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, accepted) = app
        .post_json(&format!("/api/v1/suggestions/{id}/accept"), &json!({ "staffNote": "Will order next week" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "accept: {accepted}");
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["staffNote"], "Will order next week");
    let order_id = fixtures::json_id(&accepted["purchaseOrderId"]);
    let line_id = fixtures::json_id(&accepted["purchaseOrderLineId"]);

    assert_eq!(
        suggestion_effects(&app, id, &title).await,
        SuggestionEffects {
            orders: 1,
            lines: 1,
            biblios: 0,
            items: 0,
        },
        "accept opens one order intent and does not catalog the title"
    );

    let (status, order) = app.get_json_with_auth(&format!("/api/v1/acquisitions/orders/{order_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{order}");
    assert_eq!(order["order"]["status"], "draft", "{order}");
    assert!(order["order"]["vendorId"].is_null(), "intent is not yet assigned to a vendor: {order}");
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
    let title = format!("Refused title {suffix}");

    let (status, created) = propose(&app, &reader_token, &title, "No Order", None).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = fixtures::json_id(&created["id"]);

    let (status, refused) = app
        .post_json(&format!("/api/v1/suggestions/{id}/refuse"), &json!({ "staffNote": "Already in stock" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "refuse: {refused}");
    assert_eq!(refused["status"], "refused");
    assert_eq!(refused["staffNote"], "Already in stock");
    assert!(refused["purchaseOrderId"].is_null(), "{refused}");
    assert!(refused["purchaseOrderLineId"].is_null(), "{refused}");

    let none = SuggestionEffects {
        orders: 0,
        lines: 0,
        biblios: 0,
        items: 0,
    };
    assert_eq!(suggestion_effects(&app, id, &title).await, none, "refuse must not open an order or catalog the title");

    let (status, again) = app.post_json(&format!("/api/v1/suggestions/{id}/accept"), &json!({}), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "refused stays refused: {again}");
    assert_eq!(suggestion_effects(&app, id, &title).await, none, "a later accept must not create an order for a refused suggestion");
}

#[tokio::test]
async fn create_requires_title_and_author() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "sug_valid").await;

    let (status, body) = app.post_json("/api/v1/suggestions", &json!({ "author": "No Title" }), Some(&reader_token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = app.post_json("/api/v1/suggestions", &json!({ "title": "No Author" }), Some(&reader_token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
