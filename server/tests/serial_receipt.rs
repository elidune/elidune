//! Receiving serial issues, supplements, staff statuses, and late detection.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use chrono::Utc;
use chrono_tz::Tz;
use common::fixtures;
use common::TestApp;
use elidune_server::services::calendar::local_date;
use serde_json::{json, Value};
use sqlx::Connection;

fn by_field<'a>(rows: &'a [Value], field: &str, expected: &str) -> &'a Value {
    rows.iter()
        .find(|row| row[field] == expected)
        .unwrap_or_else(|| panic!("missing {field} {expected}"))
}

async fn frequency_id(app: &TestApp, token: &str, description: &str) -> String {
    let (status, body) = app
        .get_json_with_auth("/api/v1/serials/frequencies?perPage=50", token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["frequencies"].as_array().expect("frequencies");
    by_field(rows, "description", description)["id"]
        .as_str()
        .expect("frequency id")
        .to_string()
}

async fn pattern_id(app: &TestApp, token: &str, formula: &str) -> String {
    let (status, body) = app
        .get_json_with_auth("/api/v1/serials/numbering-patterns?perPage=50", token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["patterns"].as_array().expect("patterns");
    by_field(rows, "formula", formula)["id"]
        .as_str()
        .expect("pattern id")
        .to_string()
}

fn id_string(body: &Value) -> String {
    if let Some(id) = body.as_str() {
        return id.to_string();
    }
    fixtures::json_id(body).to_string()
}

async fn create_site_id(app: &TestApp, token: &str) -> String {
    let (status, body) = app
        .post_json(
            "/api/v1/sites",
            &json!({ "name": format!("Receipt site {}", fixtures::unique_suffix()) }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["id"])
}

async fn create_periodical(app: &TestApp, token: &str, title: &str) -> String {
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "periodic",
                "lang": "french"
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["biblio"]["id"])
}

fn issue_with_x(issues: &[Value], x: i64) -> &Value {
    issues
        .iter()
        .find(|issue| issue["x"].as_i64() == Some(x))
        .unwrap_or_else(|| panic!("missing issue x={x}: {issues:?}"))
}

async fn list_issues(app: &TestApp, token: &str, subscription_id: &str) -> Vec<Value> {
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/issues"),
            token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body.as_array().expect("issues").clone()
}

struct CopyRow {
    borrowable: bool,
    price: Option<i64>,
    volume_designation: Option<String>,
    call_number: Option<String>,
    public_note: Option<String>,
    internal_note: Option<String>,
    barcode: Option<String>,
    place: Option<i16>,
}

async fn copy_row(app: &TestApp, item_id: i64) -> CopyRow {
    let row = sqlx::query_as::<_, (bool, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i16>)>(
        "SELECT borrowable, price, volume_designation, call_number, public_note, internal_note, barcode, place \
         FROM items WHERE id = $1",
    )
    .bind(item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("item");
    CopyRow {
        borrowable: row.0,
        price: row.1,
        volume_designation: row.2,
        call_number: row.3,
        public_note: row.4,
        internal_note: row.5,
        barcode: row.6,
        place: row.7,
    }
}

#[tokio::test]
async fn out_of_order_receipt_keeps_the_newest_planned_issue_in_the_library() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(&app, &admin, &format!("Revue ordre {suffix}")).await;
    let barcode = format!("SER-{suffix}");

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2024-01-31",
                "x": 412,
                "y": 0,
                "z": 0,
                "length": { "kind": "months", "count": 2 },
                "costCents": 12000,
                "issuePriceCents": 600,
                "callNumber": "PER 412",
                "location": "Reading room",
                "publicNote": "On the shelf",
                "internalNote": "Do not bind",
                "notes": "Annual invoice",
                "graceDays": 15
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let subscription_id = created["id"].as_str().expect("id");
    let next_slot = created["nextSlot"].clone();
    let issues = created["issues"].as_array().expect("issues");
    let issue_412 = issue_with_x(issues, 412);
    let issue_413 = issue_with_x(issues, 413);
    assert!(
        issue_412["plannedDate"].as_str().unwrap() < issue_413["plannedDate"].as_str().unwrap(),
        "412 is the earlier planned issue"
    );
    let id_412 = issue_412["id"].as_str().expect("412 id");
    let id_413 = issue_413["id"].as_str().expect("413 id");
    let sequence_412 = issue_412["sequence"].clone();

    let (status, _) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{id_413}/receive"),
            &json!({ "barcode": barcode }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, received_413) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{id_413}/receive"),
            &json!({ "barcode": barcode, "receivedOn": "2026-10-01" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received_413}");
    assert_eq!(received_413["status"], "arrived");
    assert!(received_413["receivedAt"]
        .as_str()
        .unwrap_or("")
        .starts_with("2026-10-01"));
    let item_413: i64 = received_413["itemId"]
        .as_str()
        .expect("item")
        .parse()
        .unwrap();

    let after_413 = list_issues(&app, &admin, subscription_id).await;
    let still_412 = issue_with_x(&after_413, 412);
    assert_eq!(still_412["status"], "expected", "{still_412}");
    assert!(still_412["itemId"].is_null(), "{still_412}");
    assert_eq!(issue_with_x(&after_413, 413)["status"], "arrived");

    let copy_413 = copy_row(&app, item_413).await;
    assert!(!copy_413.borrowable, "413 is the latest planned issue");
    assert_eq!(
        copy_413.price,
        Some(600),
        "annual cost must not be the item price"
    );
    assert_eq!(
        copy_413.volume_designation.as_deref(),
        received_413["displayLabel"].as_str()
    );
    assert_eq!(copy_413.call_number.as_deref(), Some("PER 412"));
    assert_eq!(copy_413.public_note.as_deref(), Some("On the shelf"));
    assert_eq!(copy_413.internal_note.as_deref(), Some("Do not bind"));
    assert_eq!(copy_413.barcode.as_deref(), Some(barcode.as_str()));
    assert_eq!(copy_413.place, None);

    let (status, marked) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/mark-late"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{marked}");
    let marked_ids: Vec<_> = marked["issues"]
        .as_array()
        .expect("marked")
        .iter()
        .map(|issue| issue["id"].as_str().unwrap().to_string())
        .collect();
    assert!(marked_ids.contains(&id_412.to_string()), "{marked}");
    assert!(!marked_ids.contains(&id_413.to_string()), "{marked}");
    let after_late = list_issues(&app, &admin, subscription_id).await;
    assert_eq!(issue_with_x(&after_late, 412)["status"], "late");
    assert_eq!(issue_with_x(&after_late, 413)["status"], "arrived");

    let (status, received_412) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{id_412}/receive"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received_412}");
    assert_eq!(received_412["status"], "arrived");
    let item_412: i64 = received_412["itemId"]
        .as_str()
        .expect("item 412")
        .parse()
        .unwrap();
    assert!(
        copy_row(&app, item_412).await.borrowable,
        "an earlier planned issue that arrives later is loanable"
    );
    assert!(
        !copy_row(&app, item_413).await.borrowable,
        "413 stays in the library"
    );

    let (status, supplement) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/supplements"),
            &json!({
                "plannedDate": "2026-12-01",
                "displayLabel": "hors-série"
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{supplement}");
    assert_eq!(supplement["supplement"], true);
    assert!(supplement["slot"].is_null(), "{supplement}");
    assert!(supplement["sequence"].is_null(), "{supplement}");
    assert_eq!(supplement["status"], "expected");
    let supplement_id = supplement["id"].as_str().expect("supplement id");
    let (status, subscription) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions/{subscription_id}"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{subscription}");
    assert_eq!(subscription["nextSlot"], next_slot);
    let stored = list_issues(&app, &admin, subscription_id).await;
    assert_eq!(issue_with_x(&stored, 412)["sequence"], sequence_412);

    let (status, received_supplement) = app
        .post_json(
            &format!(
                "/api/v1/serials/subscriptions/{subscription_id}/issues/{supplement_id}/receive"
            ),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received_supplement}");
    let supplement_item: i64 = received_supplement["itemId"]
        .as_str()
        .expect("supplement item")
        .parse()
        .unwrap();
    assert!(
        copy_row(&app, supplement_item).await.borrowable,
        "a supplement is not the latest issue"
    );
    assert!(!copy_row(&app, item_413).await.borrowable);
    assert!(copy_row(&app, item_412).await.borrowable);
    let final_issues = list_issues(&app, &admin, subscription_id).await;
    assert_eq!(issue_with_x(&final_issues, 412)["status"], "arrived");
    assert_eq!(issue_with_x(&final_issues, 413)["status"], "arrived");
}

#[tokio::test]
async fn staff_statuses_late_grace_and_receipt_without_an_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let (_, reader) = fixtures::create_reader(&app, &admin, "receiptreader").await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(&app, &admin, &format!("Revue statuts {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2026-12-01",
                "x": 1,
                "y": 0,
                "z": 0,
                "length": { "kind": "issues", "count": 4 },
                "createItems": false,
                "costCents": 5000,
                "graceDays": 15
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let subscription_id = created["id"].as_str().expect("id");
    let issues = created["issues"].as_array().expect("issues");
    assert!(issues.len() >= 4, "{issues:?}");
    let ids: Vec<String> = issues
        .iter()
        .map(|issue| issue["id"].as_str().unwrap().to_string())
        .collect();

    let receive = format!(
        "/api/v1/serials/subscriptions/{subscription_id}/issues/{}/receive",
        ids[0]
    );
    let (status, _) = app.post_json(&receive, &json!({}), Some(&reader)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = app
        .post_json(
            &receive,
            &json!({ "barcode": "SHOULD-NOT-STICK" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let (status, received) = app.post_json(&receive, &json!({}), Some(&admin)).await;
    assert_eq!(status, StatusCode::OK, "{received}");
    assert_eq!(received["status"], "arrived");
    assert!(received["itemId"].is_null(), "{received}");
    assert!(received["receivedAt"].is_string(), "{received}");
    let (status, again) = app.post_json(&receive, &json!({}), Some(&admin)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{again}");

    let (status, marked) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/mark-late"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{marked}");
    assert!(marked["issues"].as_array().expect("issues").is_empty());

    let status_url = |issue: &str| {
        format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{issue}/status")
    };
    let (status, missing) = app
        .post_json(
            &status_url(&ids[1]),
            &json!({ "status": "missing" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    assert_eq!(missing["status"], "missing");
    let (status, claimed) = app
        .post_json(
            &status_url(&ids[1]),
            &json!({ "status": "claimed" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{claimed}");
    assert_eq!(claimed["status"], "claimed");
    let (status, unpublished) = app
        .post_json(
            &status_url(&ids[2]),
            &json!({ "status": "notAvailable" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{unpublished}");
    assert_eq!(unpublished["status"], "notAvailable");
    let (status, deleted) = app
        .post_json(
            &status_url(&ids[3]),
            &json!({ "status": "deleted" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["status"], "deleted");
    let (status, late) = app
        .post_json(
            &status_url(&ids[3]),
            &json!({ "status": "late" }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{late}");

    let priced_biblio = create_periodical(&app, &admin, &format!("Revue sans prix {suffix}")).await;
    let (status, priced) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": priced_biblio,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2026-12-01",
                "x": 1,
                "y": 0,
                "z": 0,
                "length": { "kind": "issues", "count": 1 },
                "costCents": 99999
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{priced}");
    let priced_id = priced["id"].as_str().expect("id");
    let only = &priced["issues"][0];
    let (status, received) = app
        .post_json(
            &format!(
                "/api/v1/serials/subscriptions/{priced_id}/issues/{}/receive",
                only["id"].as_str().unwrap()
            ),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    let item_id: i64 = received["itemId"].as_str().expect("item").parse().unwrap();
    let copy = copy_row(&app, item_id).await;
    assert_eq!(copy.price, None);
    assert!(
        !copy.borrowable,
        "the only regular issue stays in the library"
    );
}

async fn set_borrowable(app: &TestApp, item_id: i64, borrowable: bool) {
    sqlx::query("UPDATE items SET borrowable = $2 WHERE id = $1")
        .bind(item_id)
        .bind(borrowable)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set borrowable");
}

async fn receive(app: &TestApp, token: &str, subscription_id: &str, issue_id: &str) -> Value {
    let (status, body) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{issue_id}/receive"),
            &json!({}),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[tokio::test]
async fn manual_not_borrowable_survives_a_later_receipt() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(&app, &admin, &format!("Revue fragile {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2024-01-31",
                "x": 412,
                "y": 0,
                "z": 0,
                "length": { "kind": "months", "count": 3 },
                "latestIssueLibraryUseOnly": false
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let subscription_id = created["id"].as_str().expect("id");
    let issues = created["issues"].as_array().expect("issues");
    let id_412 = issue_with_x(issues, 412)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id_413 = issue_with_x(issues, 413)["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id_414 = issue_with_x(issues, 414)["id"]
        .as_str()
        .unwrap()
        .to_string();

    let received_413 = receive(&app, &admin, subscription_id, &id_413).await;
    let item_413: i64 = received_413["itemId"].as_str().unwrap().parse().unwrap();
    assert!(
        copy_row(&app, item_413).await.borrowable,
        "the rule is off, so the first copy stays loanable"
    );
    set_borrowable(&app, item_413, false).await;

    let (status, stored) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions/{subscription_id}"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    let mut update = stored.clone();
    update["latestIssueLibraryUseOnly"] = json!(true);
    let (status, updated) = app
        .put_json(
            &format!("/api/v1/serials/subscriptions/{subscription_id}"),
            &update,
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");

    let received_412 = receive(&app, &admin, subscription_id, &id_412).await;
    let item_412: i64 = received_412["itemId"].as_str().unwrap().parse().unwrap();
    assert!(
        copy_row(&app, item_412).await.borrowable,
        "412 is older than 413, so it is loanable"
    );
    assert!(
        !copy_row(&app, item_413).await.borrowable,
        "413 was already non-borrowable before the rule held it"
    );
    set_borrowable(&app, item_412, false).await;

    let received_414 = receive(&app, &admin, subscription_id, &id_414).await;
    let item_414: i64 = received_414["itemId"].as_str().unwrap().parse().unwrap();
    assert!(
        !copy_row(&app, item_414).await.borrowable,
        "414 is the new latest issue"
    );
    assert!(
        !copy_row(&app, item_412).await.borrowable,
        "an older copy marked non-borrowable by staff stays that way"
    );
    assert!(
        !copy_row(&app, item_413).await.borrowable,
        "the previous latest was non-borrowable before the rule, so it stays that way"
    );
}

#[tokio::test]
async fn concurrent_receipt_of_the_same_issue_keeps_one_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(&app, &admin, &format!("Revue course {suffix}")).await;
    let biblio_num: i64 = biblio_id.parse().unwrap();

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2026-12-01",
                "x": 1,
                "y": 0,
                "z": 0,
                "length": { "kind": "issues", "count": 1 },
                "createItems": true
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let subscription_id = created["id"].as_str().expect("id").to_string();
    let issue_id = created["issues"][0]["id"]
        .as_str()
        .expect("issue")
        .to_string();
    let issue_num: i64 = issue_id.parse().unwrap();

    let app = Arc::new(app);
    let pool = app.state.services.repository.pool().clone();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let mut lock_conn = sqlx::postgres::PgConnection::connect(&database_url)
        .await
        .expect("lock connection");
    let mut tx = lock_conn.begin().await.expect("lock transaction");
    sqlx::query("SELECT id FROM serial_issues WHERE id = $1 FOR UPDATE")
        .bind(issue_num)
        .execute(&mut *tx)
        .await
        .expect("lock issue");

    let receive =
        format!("/api/v1/serials/subscriptions/{subscription_id}/issues/{issue_id}/receive");
    let first = {
        let app = Arc::clone(&app);
        let admin = admin.clone();
        let receive = receive.clone();
        tokio::spawn(async move { app.post_json(&receive, &json!({}), Some(&admin)).await })
    };
    let second = {
        let app = Arc::clone(&app);
        let admin = admin.clone();
        tokio::spawn(async move { app.post_json(&receive, &json!({}), Some(&admin)).await })
    };

    // Both receipts must reach the row lock before it is released. Polling
    // pg_stat_activity from the lock holder can stall behind those waiters.
    tokio::time::sleep(Duration::from_secs(1)).await;
    tx.rollback().await.expect("release issue lock");

    let (status_a, body_a) = first.await.expect("first receipt");
    let (status_b, body_b) = second.await.expect("second receipt");
    assert!(
        (status_a == StatusCode::OK && status_b == StatusCode::CONFLICT)
            || (status_a == StatusCode::CONFLICT && status_b == StatusCode::OK),
        "one receipt wins and the other conflicts: {status_a} {body_a} {status_b} {body_b}"
    );

    let copies: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM items WHERE biblio_id = $1 AND archived_at IS NULL",
    )
    .bind(biblio_num)
    .fetch_one(&pool)
    .await
    .expect("count items");
    assert_eq!(
        copies, 1,
        "a conflicting receipt must not leave an orphan copy"
    );

    let linked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM serial_issues \
         WHERE id = $1 AND status = 'arrived' AND item_id IS NOT NULL",
    )
    .bind(issue_num)
    .fetch_one(&pool)
    .await
    .expect("linked issue");
    assert_eq!(linked, 1);
}

#[tokio::test]
async fn mark_late_uses_the_library_local_date() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let pool = app.state.services.repository.pool().clone();
    let previous: Option<String> =
        sqlx::query_scalar("SELECT timezone FROM library_info WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("library timezone");

    let now = Utc::now();
    let utc_today = now.date_naive();
    let ahead: Tz = "Pacific/Kiritimati".parse().expect("kiritimati");
    let behind: Tz = "Pacific/Pago_Pago".parse().expect("pago pago");
    let ahead_today = local_date(ahead, now);
    let behind_today = local_date(behind, now);
    let (zone, planned, expect_late) = if ahead_today > utc_today {
        ("Pacific/Kiritimati", utc_today, true)
    } else {
        assert!(
            behind_today < utc_today,
            "Kiritimati and Pago Pago cannot both share the UTC date"
        );
        ("Pacific/Pago_Pago", behind_today, false)
    };

    let outcome = async {
        sqlx::query("UPDATE library_info SET timezone = $1 WHERE id = 1")
            .bind(zone)
            .execute(&pool)
            .await
            .expect("set timezone");

        let monthly = frequency_id(&app, &admin, "1/month").await;
        let pattern = pattern_id(&app, &admin, "n° {X}").await;
        let site_id = create_site_id(&app, &admin).await;
        let suffix = fixtures::unique_suffix();
        let biblio_id = create_periodical(&app, &admin, &format!("Revue fuseau {suffix}")).await;
        let (status, created) = app
            .post_json(
                "/api/v1/serials/subscriptions",
                &json!({
                    "biblioId": biblio_id,
                    "siteId": site_id,
                    "frequencyId": monthly,
                    "numberingPatternId": pattern,
                    "firstIssueDate": "2026-12-01",
                    "x": 1,
                    "y": 0,
                    "z": 0,
                    "length": { "kind": "issues", "count": 1 },
                    "createItems": false,
                    "graceDays": 0
                }),
                Some(&admin),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let subscription_id = created["id"].as_str().expect("id");
        let issue_num: i64 = created["issues"][0]["id"]
            .as_str()
            .expect("issue")
            .parse()
            .unwrap();
        sqlx::query("UPDATE serial_issues SET planned_date = $2 WHERE id = $1")
            .bind(issue_num)
            .bind(planned)
            .execute(&pool)
            .await
            .expect("planned date");

        let (status, marked) = app
            .post_json(
                &format!("/api/v1/serials/subscriptions/{subscription_id}/mark-late"),
                &json!({}),
                Some(&admin),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{marked}");
        let late_count = marked["issues"].as_array().expect("issues").len();
        let stored: String = sqlx::query_scalar("SELECT status FROM serial_issues WHERE id = $1")
            .bind(issue_num)
            .fetch_one(&pool)
            .await
            .expect("status");
        (late_count, stored)
    }
    .await;

    sqlx::query("UPDATE library_info SET timezone = $1 WHERE id = 1")
        .bind(&previous)
        .execute(&pool)
        .await
        .expect("restore timezone");

    let (late_count, stored) = outcome;
    if expect_late {
        assert_eq!(late_count, 1, "local date is after the planned date");
        assert_eq!(stored, "late");
    } else {
        assert_eq!(
            late_count, 0,
            "UTC is already the next day, but the library date is not"
        );
        assert_eq!(stored, "expected");
    }
}
