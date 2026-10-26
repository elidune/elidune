//! Checkout records `loans.site_id` and statistics count that site (#128).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

async fn create_site(app: &TestApp, token: &str, name: &str) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/sites",
            &json!({ "name": format!("{name}-{}", fixtures::unique_suffix()) }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create site: {body}");
    fixtures::json_id(&body["id"])
}

async fn seed_item(app: &TestApp, token: &str, site_id: i64) -> (i64, String) {
    let barcode = format!("SITE-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Loan site {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "borrowable": true,
                    "siteId": site_id.to_string()
                }]
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {body}");
    (
        fixtures::json_id(&body["biblio"]["items"][0]["id"]),
        barcode,
    )
}

async fn library_default_site(app: &TestApp) -> i64 {
    let sql = format!(
        "SELECT id FROM sites WHERE id = {}",
        elidune_server::services::calendar::LIBRARY_DEFAULT_SITE_ID_SQL
    );
    sqlx::query_scalar(&sql)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("library default site")
}

fn loans_at(body: &Value, site_id: i64) -> i64 {
    body["bySource"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.get("siteId")
                    .is_some_and(|id| fixtures::json_id(id) == site_id)
            })
        })
        .and_then(|row| row["loans"].as_i64())
        .unwrap_or(0)
}

#[tokio::test]
async fn checkout_records_site_and_statistics_count_it() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "loansite").await;
    let home = create_site(&app, &token, "Home").await;
    let desk = create_site(&app, &token, "Desk").await;
    let (item_id, _barcode) = seed_item(&app, &token, home).await;

    let (status, before) = app
        .get_json_with_auth("/api/v1/stats/catalog?bySource=true", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let home_before = loans_at(&before, home);
    let desk_before = loans_at(&before, desk);

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string(),
                "siteId": desk.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    assert_eq!(loan["siteId"], desk.to_string(), "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

    let stored: Option<i64> = sqlx::query_scalar("SELECT site_id FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("loan site");
    assert_eq!(stored, Some(desk));

    let (status, list) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let row = list["items"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| fixtures::json_id(&row["id"]) == loan_id)
        })
        .expect("loan row");
    assert_eq!(row["siteId"], desk.to_string(), "{row}");

    let (status, schema) = app.get_json_with_auth("/api/v1/stats/schema", &token).await;
    assert_eq!(status, StatusCode::OK, "{schema}");
    assert!(schema["entities"]["loans"]["fields"]["site_id"].is_object());
    assert!(schema["entities"]["loans"]["fields"]["effective_site_id"].is_object());
    assert!(schema["entities"]["loans_archives"]["fields"]["site_id"].is_object());

    let (status, stats) = app
        .post_json(
            "/api/v1/stats/query",
            &json!({
                "entity": "loans",
                "unionWith": ["loans_archives"],
                "select": [{ "field": "loans.effective_site_id", "alias": "siteId" }],
                "aggregations": [{ "fn": "count", "field": "loans.id", "alias": "n" }],
                "groupBy": [{ "field": "loans.effective_site_id" }],
                "filters": [{ "field": "loans.id", "op": "eq", "value": loan_id }]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "stats query: {stats}");
    assert!(stats["sqlError"].is_null(), "{stats}");
    let counted = stats["rows"]
        .as_array()
        .and_then(|rows| rows.first())
        .expect("one stats row");
    assert_eq!(fixtures::json_id(&counted["siteId"]), desk, "{counted}");
    assert_eq!(counted["n"].as_i64(), Some(1), "{counted}");

    let (status, returned) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {returned}");
    assert_eq!(returned["loan"]["siteId"], desk.to_string(), "{returned}");

    let archived: Option<i64> = sqlx::query_scalar(
        "SELECT site_id FROM loans_archives WHERE item_id = $1 ORDER BY returned_at DESC LIMIT 1",
    )
    .bind(item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("archived site");
    assert_eq!(archived, Some(desk));

    let (status, after) = app
        .get_json_with_auth("/api/v1/stats/catalog?bySource=true", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(loans_at(&after, desk), desk_before + 1, "{after}");
    assert_eq!(loans_at(&after, home), home_before, "{after}");
}

#[tokio::test]
async fn omitted_site_stays_null_and_unknown_site_is_not_found() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "loannull").await;
    let default_site = library_default_site(&app).await;
    let home = create_site(&app, &token, "NullHome").await;
    assert_ne!(default_site, home);
    let (item_id, _) = seed_item(&app, &token, home).await;

    let (status, before) = app
        .get_json_with_auth("/api/v1/stats/catalog?bySource=true", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let home_before = loans_at(&before, home);

    let (status, missing) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string(),
                "siteId": "999999999999"
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert_eq!(missing["code"], "not_found");
    assert_eq!(missing["error"], "Not Found");
    assert!(
        missing["message"]
            .as_str()
            .is_some_and(|message| message.contains("not found")),
        "{missing}"
    );

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    assert!(loan["siteId"].is_null(), "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    let stored: Option<i64> = sqlx::query_scalar("SELECT site_id FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("null site");
    assert_eq!(stored, None);

    let (status, stats) = app
        .post_json(
            "/api/v1/stats/query",
            &json!({
                "entity": "loans",
                "select": [{ "field": "loans.effective_site_id", "alias": "siteId" }],
                "aggregations": [{ "fn": "count", "field": "loans.id", "alias": "n" }],
                "groupBy": [{ "field": "loans.effective_site_id" }],
                "filters": [{ "field": "loans.id", "op": "eq", "value": loan_id }]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "stats query: {stats}");
    assert!(stats["sqlError"].is_null(), "{stats}");
    let counted = stats["rows"]
        .as_array()
        .and_then(|rows| rows.first())
        .expect("one stats row");
    assert_eq!(
        fixtures::json_id(&counted["siteId"]),
        default_site,
        "{counted}"
    );
    assert_eq!(counted["n"].as_i64(), Some(1), "{counted}");

    let (status, after) = app
        .get_json_with_auth("/api/v1/stats/catalog?bySource=true", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(loans_at(&after, home), home_before, "{after}");
}

#[tokio::test]
async fn deleting_a_site_referenced_by_an_archive_is_refused() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "loanrestrict").await;
    let home = create_site(&app, &token, "RestrictHome").await;
    let desk = create_site(&app, &token, "RestrictDesk").await;
    let (item_id, _) = seed_item(&app, &token, home).await;

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string(),
                "siteId": desk.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

    let (status, returned) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "return: {returned}");

    let pool = app.state.services.repository.pool();
    let archived: Option<i64> = sqlx::query_scalar(
        "SELECT site_id FROM loans_archives WHERE item_id = $1 ORDER BY returned_at DESC LIMIT 1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("archived site");
    assert_eq!(archived, Some(desk));

    let error = sqlx::query("DELETE FROM sites WHERE id = $1")
        .bind(desk)
        .execute(pool)
        .await
        .expect_err("archive site_id is ON DELETE RESTRICT");
    let db = error.as_database_error().expect("postgres error");
    assert_eq!(db.code().as_deref(), Some("23503"), "{db}");
    assert!(db.message().contains("loans_archives"), "{}", db.message());

    let still: Option<i64> = sqlx::query_scalar(
        "SELECT site_id FROM loans_archives WHERE item_id = $1 ORDER BY returned_at DESC LIMIT 1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("archive row remains");
    assert_eq!(still, Some(desk));
}
