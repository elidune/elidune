//! Deposit circulation rules: due-date cap, holds, close, exclusions, loss billing.

mod common;

use axum::http::StatusCode;
use chrono::NaiveDate;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn assert_error(body: &Value, code: &str) {
    assert_eq!(body["code"], code, "{body}");
    assert!(body["error"].is_string(), "{body}");
    assert!(body["message"].is_string(), "{body}");
}

fn copy_barcodes(copies: &Value) -> Vec<&str> {
    copies
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["barcode"].as_str())
        .collect()
}

fn copy_ids(copies: &Value) -> Vec<String> {
    copies
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["itemId"].as_str().map(str::to_string))
        .collect()
}

fn assert_stock_identity(stats: &Value) {
    let deposits = stats["items"]["deposits"].as_array().expect("deposits");
    let own_stock = stats["items"]["ownStock"].as_i64().expect("ownStock");
    let total = stats["items"]["total"].as_i64().expect("total");
    let deposit_total: i64 = deposits
        .iter()
        .map(|row| row["count"].as_i64().unwrap())
        .sum();
    assert_eq!(own_stock + deposit_total, total, "{stats}");
}

async fn patch_json(app: &TestApp, uri: &str, body: &Value, token: &str) -> (StatusCode, Value) {
    use axum::body::Body;
    use http_body_util::BodyExt;

    let response = app
        .request(
            axum::http::Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn site(app: &TestApp, token: &str) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/sources",
            &json!({"name": format!("Deposit site {}", fixtures::unique_token())}),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["id"])
}

async fn lender(app: &TestApp, token: &str, rate: Option<i64>) -> i64 {
    let mut payload = json!({
        "name": format!("Lender {}", fixtures::unique_token()),
        "contact": "desk@lender.test"
    });
    if let Some(rate) = rate {
        payload["replacementRateCents"] = json!(rate);
    }
    let (status, body) = app
        .post_json("/api/v1/lenders", &payload, Some(token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["id"])
}

async fn batch(app: &TestApp, token: &str, lender_id: i64, site_id: i64) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/deposit-batches",
            &json!({
                "lenderId": lender_id.to_string(),
                "reference": format!("RET-{}", fixtures::unique_token()),
                "siteId": site_id.to_string(),
                "arrivedOn": "2026-03-01",
                "returnDueOn": "2026-12-01"
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["id"])
}

async fn copies(
    app: &TestApp,
    token: &str,
    count: usize,
    price: Option<&str>,
) -> (i64, Vec<(i64, String)>) {
    let items: Vec<Value> = (0..count)
        .map(|index| {
            let mut item = json!({
                "barcode": format!("DEP-{}-{index}", fixtures::unique_suffix()),
                "borrowable": true
            });
            if let Some(price) = price {
                item["price"] = json!(price);
            }
            item
        })
        .collect();
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Deposit title {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": items
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let rows = body["biblio"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| {
            (
                fixtures::json_id(&item["id"]),
                item["barcode"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    (biblio_id, rows)
}

async fn attach(app: &TestApp, batch_id: i64, site_id: i64, item_id: i64) {
    sqlx::query("UPDATE items SET deposit_batch_id = $1, source_id = $2 WHERE id = $3")
        .bind(batch_id)
        .bind(site_id)
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
}

async fn shift_return(app: &TestApp, batch_id: i64, days: i32) -> NaiveDate {
    sqlx::query_scalar(
        r#"
        UPDATE deposit_batches
        SET return_due_on = (
            (CURRENT_TIMESTAMP AT TIME ZONE COALESCE(
                (SELECT timezone FROM library_info WHERE id = 1),
                'Europe/Paris'
            ))::date + $2
        )
        WHERE id = $1
        RETURNING return_due_on
        "#,
    )
    .bind(batch_id)
    .bind(days)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap()
}

async fn checkout(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(token),
    )
    .await
}

async fn local_due(app: &TestApp, loan_id: i64) -> NaiveDate {
    sqlx::query_scalar(
        r#"
        SELECT (expiry_at AT TIME ZONE COALESCE(
            (SELECT timezone FROM library_info WHERE id = 1),
            'Europe/Paris'
        ))::date
        FROM loans WHERE id = $1
        "#,
    )
    .bind(loan_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap()
}

async fn renews(app: &TestApp, loan_id: i64) -> i32 {
    sqlx::query_scalar("SELECT COALESCE(nb_renews, 0) FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn loan_cap_renewal_and_return_due_are_not_forceable() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, None).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let (_biblio, items) = copies(&app, &token, 2, None).await;
    attach(&app, batch_id, site_id, items[0].0).await;
    attach(&app, batch_id, site_id, items[1].0).await;
    let return_due = shift_return(&app, batch_id, 5).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "deploan").await;

    let (status, body) = checkout(&app, &token, reader_id, items[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["dueDateCappedByDeposit"], true, "{body}");
    let loan_id = fixtures::json_id(&body["id"]);
    assert_eq!(local_due(&app, loan_id).await, return_due);

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/renew"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_error(&body, "deposit_renewal_no_extension");
    assert_eq!(renews(&app, loan_id).await, 0);

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_error(&body, "deposit_renewal_no_extension");
    assert_eq!(renews(&app, loan_id).await, 0);

    shift_return(&app, batch_id, 0).await;
    let (status, body) = checkout(&app, &token, reader_id, items[1].0, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_error(&body, "deposit_return_due");

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_error(&body, "deposit_return_due");
    assert_eq!(renews(&app, loan_id).await, 0);
}

#[tokio::test]
async fn holds_skip_returning_and_past_due_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, None).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "dephold").await;

    let past_batch = batch(&app, &token, lender_id, site_id).await;
    let (biblio_id, items) = copies(&app, &token, 2, None).await;
    let deposit_item = items[0].0;
    let owned_item = items[1].0;
    attach(&app, past_batch, site_id, deposit_item).await;
    sqlx::query("UPDATE items SET source_id = $1 WHERE id = $2")
        .bind(site_id)
        .bind(owned_item)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    shift_return(&app, past_batch, -2).await;

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{hold}");
    assert_eq!(hold["itemId"], owned_item.to_string(), "{hold}");

    let (copy_reader, _) = fixtures::create_reader(&app, &token, "depholdcopy").await;
    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": copy_reader.to_string(),
                "itemId": deposit_item.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_error(&body, "deposit_batch_returning");

    let (reader2, _) = fixtures::create_reader(&app, &token, "dephold2").await;
    let returning = batch(&app, &token, lender_id, site_id).await;
    let (_biblio2, pinned_items) = copies(&app, &token, 2, None).await;
    attach(&app, returning, site_id, pinned_items[0].0).await;
    sqlx::query("UPDATE items SET source_id = $1 WHERE id = $2")
        .bind(site_id)
        .bind(pinned_items[1].0)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    shift_return(&app, returning, 30).await;
    let (status, pinned) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader2.to_string(),
                "itemId": pinned_items[0].0.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{pinned}");
    let hold_id = fixtures::json_id(&pinned["id"]);

    let (status, started) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{returning}/start-return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["status"], "returning");

    let (moved_item, moved_status): (Option<i64>, String) =
        sqlx::query_as("SELECT item_id, status FROM holds WHERE id = $1")
            .bind(hold_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert_eq!(moved_item, Some(pinned_items[1].0));
    assert_ne!(moved_status, "ready");

    let (status, refused) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": pinned_items[0].0.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_error(&refused, "deposit_batch_returning");
}

#[tokio::test]
async fn close_archives_without_delete_and_reports_missing_and_loans() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, None).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let (_biblio, items) = copies(&app, &token, 4, None).await;
    for (item_id, _) in &items {
        attach(&app, batch_id, site_id, *item_id).await;
    }
    shift_return(&app, batch_id, 40).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "depclose").await;
    let (status, loan) = checkout(&app, &token, reader_id, items[2].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    assert_eq!(loan["dueDateCappedByDeposit"], false, "{loan}");
    let (status, still_out) = checkout(&app, &token, reader_id, items[3].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{still_out}");
    let lost_later_id = fixtures::json_id(&still_out["id"]);

    let (status, early) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": items[0].1}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{early}");
    assert_error(&early, "deposit_batch_not_returning");
    let (status, early_close) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{early_close}");
    assert_error(&early_close, "deposit_batch_not_returning");

    let (status, _) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/start-return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, empty) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": " "}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{empty}");
    let (status, missing_barcode) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": "NOT-IN-BATCH"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing_barcode}");

    let (status, scanned) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": items[0].1}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{scanned}");
    assert_eq!(scanned["alreadyScanned"], false);
    let (status, again) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": items[0].1}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["alreadyScanned"], true);
    assert_eq!(again["depositReturnedAt"], scanned["depositReturnedAt"]);

    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1")
        .bind(batch_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    let (status, refused) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_error(&refused, "deposit_batch_has_active_loans");
    let refused_ids = copy_ids(&refused["copies"]);
    let refused_barcodes = copy_barcodes(&refused["copies"]);
    assert!(refused_ids.contains(&items[2].0.to_string()), "{refused}");
    assert!(refused_ids.contains(&items[3].0.to_string()), "{refused}");
    assert!(refused_barcodes.contains(&items[2].1.as_str()), "{refused}");
    assert!(refused_barcodes.contains(&items[3].1.as_str()), "{refused}");
    assert_eq!(refused["copies"].as_array().unwrap().len(), 2, "{refused}");
    let (status, still_returning) = app
        .get_json_with_auth(&format!("/api/v1/deposit-batches/{batch_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{still_returning}");
    assert_eq!(still_returning["status"], "returning", "{still_returning}");
    let archived_early: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1 AND archived_at IS NOT NULL",
    )
    .bind(batch_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(archived_early, 0);

    let (status, scanned_loan) = app
        .post_json(
            &format!("/api/v1/deposit-batches/{batch_id}/scan-out"),
            &json!({"barcode": items[2].1}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{scanned_loan}");
    assert_error(&scanned_loan, "deposit_item_on_loan");
    assert_eq!(scanned_loan["itemId"], items[2].0.to_string(), "{scanned_loan}");
    assert_eq!(scanned_loan["barcode"], items[2].1, "{scanned_loan}");
    let not_stamped: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deposit_returned_at FROM items WHERE id = $1")
            .bind(items[2].0)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert!(not_stamped.is_none(), "{not_stamped:?}");

    let (status, returned) = app
        .post_empty(
            &format!("/api/v1/loans/items/{}/return", items[2].1),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    assert_eq!(returned["status"], "returned", "{returned}");
    assert_eq!(returned["returnToLender"], true, "{returned}");
    let (archived_after, marked_returned): (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT archived_at, deposit_returned_at FROM items WHERE id = $1")
        .bind(items[2].0)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    assert!(archived_after.is_none());
    assert!(marked_returned.is_some());

    let (status, still_one) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{still_one}");
    assert_error(&still_one, "deposit_batch_has_active_loans");
    assert_eq!(
        copy_barcodes(&still_one["copies"]),
        vec![items[3].1.as_str()]
    );
    assert_eq!(copy_ids(&still_one["copies"]), vec![items[3].0.to_string()]);

    let (status, declared_lost) = app
        .post_json(
            &format!("/api/v1/loans/{lost_later_id}/lost"),
            &json!({"bill": true, "amount": "12.00"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{declared_lost}");
    let (lost_archived, lost_returned_at): (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT archived_at, deposit_returned_at FROM items WHERE id = $1")
        .bind(items[3].0)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    assert!(lost_archived.is_none());
    assert!(lost_returned_at.is_none());

    let (status, report) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert!(report.get("onLoan").is_none(), "{report}");
    let returned_barcodes = copy_barcodes(&report["returned"]);
    let missing_barcodes = copy_barcodes(&report["missing"]);
    assert!(returned_barcodes.contains(&items[0].1.as_str()), "{report}");
    assert!(returned_barcodes.contains(&items[2].1.as_str()), "{report}");
    assert!(missing_barcodes.contains(&items[1].1.as_str()), "{report}");
    assert!(missing_barcodes.contains(&items[3].1.as_str()), "{report}");
    assert_eq!(returned_barcodes.len(), 2, "{report}");
    assert_eq!(missing_barcodes.len(), 2, "{report}");
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1")
        .bind(batch_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let archived: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1 AND archived_at IS NOT NULL",
    )
    .bind(batch_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(archived, 4);
    let lost_returned_after: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT deposit_returned_at FROM items WHERE id = $1")
            .bind(items[3].0)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert!(lost_returned_after.is_none());

    let (status, missing) = app
        .get_json_with_auth(
            &format!("/api/v1/deposit-batches/{batch_id}/items?state=missing"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    let missing_state = copy_barcodes(&missing);
    assert!(missing_state.contains(&items[1].1.as_str()), "{missing}");
    assert!(missing_state.contains(&items[3].1.as_str()), "{missing}");
    assert!(!missing_state.contains(&items[2].1.as_str()), "{missing}");

    let (status, again_report) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{again_report}");
    assert_eq!(again_report, report);
    assert!(again_report.get("onLoan").is_none(), "{again_report}");

    let (status, closed) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/start-return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{closed}");
    assert_error(&closed, "deposit_batch_not_active");

    let (_own_biblio, own) = copies(&app, &token, 1, Some("10.00")).await;
    sqlx::query("UPDATE items SET source_id = $1 WHERE id = $2")
        .bind(site_id)
        .bind(own[0].0)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    let (own_reader, _) = fixtures::create_reader(&app, &token, "depown").await;
    let (status, own_loan) = checkout(&app, &token, own_reader, own[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{own_loan}");
    let own_loan_id = fixtures::json_id(&own_loan["id"]);
    let (status, own_returned) = app
        .post_empty(&format!("/api/v1/loans/{own_loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "{own_returned}");
    assert_eq!(own_returned["returnToLender"], false, "{own_returned}");
}

#[tokio::test]
async fn inventory_weeding_and_stats_exclude_deposit_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, None).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let (_owned_biblio, owned) = copies(&app, &token, 1, None).await;
    let (_deposit_biblio, deposit) = copies(&app, &token, 1, None).await;
    sqlx::query("UPDATE items SET source_id = $1 WHERE id = $2")
        .bind(site_id)
        .bind(owned[0].0)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    attach(&app, batch_id, site_id, deposit[0].0).await;

    let (status, session) = app
        .post_json(
            "/api/v1/inventory/sessions",
            &json!({
                "name": format!("Deposit inventory {}", fixtures::unique_token()),
                "scopeSourceId": site_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    assert_eq!(session["expectedInScope"], 1, "{session}");
    let session_id = fixtures::json_id(&session["session"]["id"]);
    let (status, scan) = app
        .post_json(
            &format!("/api/v1/inventory/sessions/{session_id}/scan"),
            &json!({"barcode": deposit[0].1}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{scan}");
    assert_eq!(scan["result"], "found_out_of_scope");

    let (status, weed) = app
        .post_json(
            &format!("/api/v1/items/{}/weeding", deposit[0].0),
            &json!({"status": "candidate", "reason": "deposit"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{weed}");
    assert_error(&weed, "deposit_item_not_owned");
    let (status, withdrawn) = app
        .post_json(
            &format!("/api/v1/items/{}/withdrawals", deposit[0].0),
            &json!({"reason": "worn", "destination": "pulp"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{withdrawn}");
    assert_error(&withdrawn, "deposit_item_not_owned");

    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let agent_id = fixtures::json_id(&me["id"]);
    sqlx::query(
        r#"
        INSERT INTO item_withdrawals (id, item_id, reason, destination, agent_id)
        VALUES ($1, $2, 'worn', 'pulp', $3)
        "#,
    )
    .bind((fixtures::unique_suffix() % (i64::MAX as u128)) as i64)
    .bind(deposit[0].0)
    .bind(agent_id)
    .execute(app.state.services.repository.pool())
    .await
    .unwrap();
    let (status, list) = app
        .get_json_with_auth(
            &format!("/api/v1/item-withdrawals?siteId={site_id}&perPage=50"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let listed = list["items"].as_array().unwrap();
    let deposit_item = deposit[0].0.to_string();
    assert!(
        listed
            .iter()
            .all(|row| row["itemId"].as_str() != Some(deposit_item.as_str())),
        "{list}"
    );

    let (status, stats) = app
        .get_json_with_auth("/api/v1/stats?year=2026", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    let deposits = stats["items"]["deposits"].as_array().unwrap();
    let lender_key = lender_id.to_string();
    let row = deposits
        .iter()
        .find(|row| row["lenderId"].as_str() == Some(lender_key.as_str()))
        .expect("lender row");
    assert_eq!(row["count"], 1);

    let audience = format!("d{}", fixtures::unique_suffix() % 1_000_000_000);
    let (image_biblio_status, image_body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Image deposit {}", fixtures::unique_suffix()),
                "mediaType": "images",
                "lang": "french",
                "items": [
                    {"barcode": format!("IMG-{}-o", fixtures::unique_suffix()), "borrowable": true},
                    {"barcode": format!("IMG-{}-d", fixtures::unique_suffix()), "borrowable": true}
                ]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(image_biblio_status, StatusCode::CREATED, "{image_body}");
    let image_biblio = fixtures::json_id(&image_body["biblio"]["id"]);
    let image_own = fixtures::json_id(&image_body["biblio"]["items"][0]["id"]);
    let image_deposit = fixtures::json_id(&image_body["biblio"]["items"][1]["id"]);
    sqlx::query("UPDATE biblios SET audience_type = $1 WHERE id = $2")
        .bind(&audience)
        .bind(image_biblio)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    attach(&app, batch_id, site_id, image_deposit).await;
    let stats_uri = format!("/api/v1/stats?year=2026&publicType={audience}");
    let (status, scoped) = app.get_json_with_auth(&stats_uri, &token).await;
    assert_eq!(status, StatusCode::OK, "{scoped}");
    assert_eq!(scoped["items"]["acquisitions"], 1, "{scoped}");
    assert_eq!(scoped["items"]["ownStock"], 1, "{scoped}");
    assert_eq!(scoped["items"]["total"], 2, "{scoped}");
    assert_eq!(scoped["items"]["withdrawals"], 0, "{scoped}");
    sqlx::query("UPDATE items SET archived_at = NOW() WHERE id = $1")
        .bind(image_deposit)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    let (status, withdrawn_deposit) = app.get_json_with_auth(&stats_uri, &token).await;
    assert_eq!(status, StatusCode::OK, "{withdrawn_deposit}");
    assert_eq!(withdrawn_deposit["items"]["withdrawals"], 0);
    assert_eq!(withdrawn_deposit["items"]["ownStock"], 1);
    sqlx::query("UPDATE items SET archived_at = NOW() WHERE id = $1")
        .bind(image_own)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    let (status, withdrawn_own) = app.get_json_with_auth(&stats_uri, &token).await;
    assert_eq!(status, StatusCode::OK, "{withdrawn_own}");
    assert_eq!(withdrawn_own["items"]["withdrawals"], 1);
    assert_eq!(withdrawn_own["items"]["ownStock"], 0);
}

#[tokio::test]
async fn loss_bills_the_lender_rate_or_requires_an_amount() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let (status, zero) = app
        .post_json(
            "/api/v1/lenders",
            &json!({
                "name": format!("Zero {}", fixtures::unique_token()),
                "replacementRateCents": 0
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{zero}");

    let rated = lender(&app, &token, Some(1500)).await;
    let unrated = lender(&app, &token, None).await;
    let rated_batch = batch(&app, &token, rated, site_id).await;
    let unrated_batch = batch(&app, &token, unrated, site_id).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "deploss").await;

    let (_biblio, rated_items) = copies(&app, &token, 3, Some("20.00")).await;
    for (item_id, _) in &rated_items {
        attach(&app, rated_batch, site_id, *item_id).await;
    }
    shift_return(&app, rated_batch, 40).await;
    let (status, loan) = checkout(&app, &token, reader_id, rated_items[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    let (status, lost) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({"bill": false, "amount": "0"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{lost}");
    assert_eq!(lost["charge"]["amount"], "15.00", "{lost}");

    let (status, losses) = app
        .get_json_with_auth(
            &format!("/api/v1/deposit-losses?lenderId={rated}"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{losses}");
    let rated_item = rated_items[0].0.to_string();
    let loss_row = losses
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["itemId"].as_str() == Some(rated_item.as_str()))
        .expect("unnotified loss");
    assert_eq!(loss_row["kind"], "lost", "{loss_row}");
    assert_eq!(loss_row["amountCents"], 1500, "{loss_row}");
    assert!(loss_row["depositLossNotifiedAt"].is_null(), "{loss_row}");

    let mut saw_audit = false;
    for _ in 0..20 {
        let (status, audit) = app
            .get_json_with_auth(
                "/api/v1/audit?eventType=deposit_item_lost&perPage=50",
                &token,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{audit}");
        let lender_key = rated.to_string();
        saw_audit = audit["entries"].as_array().unwrap().iter().any(|entry| {
            entry["payload"]["lenderId"].as_str() == Some(lender_key.as_str())
                && entry["payload"]["kind"] == "lost"
                && entry["payload"]["amountCents"] == 1500
        });
        if saw_audit {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(saw_audit, "deposit_item_lost audit");

    let (_biblio, plain) = copies(&app, &token, 2, Some("20.00")).await;
    let (_free_biblio, free) = copies(&app, &token, 1, Some("20.00")).await;
    let (_unknown_biblio, unknown) = copies(&app, &token, 1, None).await;
    sqlx::query("UPDATE items SET price = 0 WHERE id = $1")
        .bind(free[0].0)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    for item_id in [plain[0].0, plain[1].0, free[0].0, unknown[0].0] {
        attach(&app, unrated_batch, site_id, item_id).await;
    }
    shift_return(&app, unrated_batch, 40).await;

    let (status, slip_loan) = checkout(&app, &token, reader_id, plain[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{slip_loan}");
    let slip_id = fixtures::json_id(&slip_loan["id"]);
    let (status, from_slip) = app
        .post_json(
            &format!("/api/v1/loans/{slip_id}/lost"),
            &json!({"bill": false}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{from_slip}");
    assert_eq!(from_slip["charge"]["amount"], "20.00", "{from_slip}");

    let (status, slip_override) = checkout(&app, &token, reader_id, plain[1].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{slip_override}");
    let slip_override_id = fixtures::json_id(&slip_override["id"]);
    let (status, price_wins) = app
        .post_json(
            &format!("/api/v1/loans/{slip_override_id}/lost"),
            &json!({"bill": true, "amount": "8.50"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{price_wins}");
    assert_eq!(price_wins["charge"]["amount"], "20.00", "{price_wins}");

    let (status, free_loan) = checkout(&app, &token, reader_id, free[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{free_loan}");
    let free_id = fixtures::json_id(&free_loan["id"]);
    let (status, missing) = app
        .post_json(
            &format!("/api/v1/loans/{free_id}/lost"),
            &json!({"bill": true}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");
    assert_error(&missing, "deposit_replacement_amount_required");
    let (status, zero_amount) = app
        .post_json(
            &format!("/api/v1/loans/{free_id}/lost"),
            &json!({"bill": true, "amount": "0"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{zero_amount}");
    assert_error(&zero_amount, "deposit_replacement_amount_required");
    let (status, billed) = app
        .post_json(
            &format!("/api/v1/loans/{free_id}/lost"),
            &json!({"bill": false, "amount": "8.50"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{billed}");
    assert_eq!(billed["charge"]["amount"], "8.50");

    let (status, unknown_loan) = checkout(&app, &token, reader_id, unknown[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{unknown_loan}");
    let unknown_id = fixtures::json_id(&unknown_loan["id"]);
    let (status, unknown_missing) = app
        .post_json(
            &format!("/api/v1/loans/{unknown_id}/lost"),
            &json!({"bill": true, "amount": "0"}),
            Some(&token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{unknown_missing}"
    );
    assert_error(&unknown_missing, "deposit_replacement_amount_required");

    let (status, damage_loan) = checkout(&app, &token, reader_id, rated_items[1].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{damage_loan}");
    let damage_id = fixtures::json_id(&damage_loan["id"]);
    let (status, damaged) = app
        .post_json(
            &format!("/api/v1/loans/{damage_id}/damaged"),
            &json!({"disposition": "return", "bill": false, "amount": "1.00"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{damaged}");
    assert_eq!(damaged["charge"]["amount"], "15.00");
    assert_eq!(damaged["charge"]["chargeType"], "damage");

    let (status, rate_change) = patch_json(
        &app,
        &format!("/api/v1/lenders/{rated}"),
        &json!({"replacementRateCents": 4200}),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rate_change}");
    assert_eq!(rate_change["replacementRateCents"], 4200, "{rate_change}");
    let frozen_lost: i64 = sqlx::query_scalar(
        "SELECT amount FROM fines WHERE loan_id = $1 AND charge_type = 'replacement'",
    )
    .bind(loan_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    let frozen_damage: i64 = sqlx::query_scalar(
        "SELECT amount FROM fines WHERE loan_id = $1 AND charge_type = 'damage'",
    )
    .bind(damage_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(frozen_lost, 1500);
    assert_eq!(frozen_damage, 1500);

    let (status, later_loan) = checkout(&app, &token, reader_id, rated_items[2].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{later_loan}");
    let later_id = fixtures::json_id(&later_loan["id"]);
    let (status, later_lost) = app
        .post_json(
            &format!("/api/v1/loans/{later_id}/lost"),
            &json!({"bill": false, "amount": "0"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{later_lost}");
    assert_eq!(later_lost["charge"]["amount"], "42.00", "{later_lost}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT amount FROM fines WHERE loan_id = $1 AND charge_type = 'replacement'"
        )
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap(),
        1500
    );
}

#[tokio::test]
async fn stats_own_stock_plus_deposit_counts_equal_total() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, None).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let (owned_biblio, owned) = copies(&app, &token, 1, None).await;
    let (deposit_biblio, deposit) = copies(&app, &token, 1, None).await;
    let audience = format!("d{}", fixtures::unique_suffix() % 1_000_000_000);
    sqlx::query("UPDATE biblios SET audience_type = $1 WHERE id = $2 OR id = $3")
        .bind(&audience)
        .bind(owned_biblio)
        .bind(deposit_biblio)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE items SET source_id = $1 WHERE id = $2")
        .bind(site_id)
        .bind(owned[0].0)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
    attach(&app, batch_id, site_id, deposit[0].0).await;

    let live = format!("/api/v1/stats?publicType={audience}");
    let year_uri = format!("/api/v1/stats?year=2026&publicType={audience}");
    for uri in [&live, &year_uri] {
        let (status, stats) = app.get_json_with_auth(uri, &token).await;
        assert_eq!(status, StatusCode::OK, "{uri} {stats}");
        assert_stock_identity(&stats);
        assert_eq!(stats["items"]["ownStock"], 1, "{uri} {stats}");
        assert_eq!(stats["items"]["total"], 2, "{uri} {stats}");
        let lender_key = lender_id.to_string();
        let row = stats["items"]["deposits"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["lenderId"].as_str() == Some(lender_key.as_str()))
            .expect("lender row");
        assert_eq!(row["count"], 1, "{uri} {stats}");
    }
}

#[tokio::test]
async fn lender_replacement_rate_can_be_cleared() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let lender_id = lender(&app, &token, Some(1500)).await;

    let (status, kept) = patch_json(
        &app,
        &format!("/api/v1/lenders/{lender_id}"),
        &json!({"name": "Kept rate"}),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    assert_eq!(kept["replacementRateCents"], 1500, "{kept}");
    assert!(kept.get("lossFlaggedAt").is_none(), "{kept}");

    let (status, cleared) = patch_json(
        &app,
        &format!("/api/v1/lenders/{lender_id}"),
        &json!({"replacementRateCents": null}),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["replacementRateCents"].is_null(), "{cleared}");

    let (status, set) = patch_json(
        &app,
        &format!("/api/v1/lenders/{lender_id}"),
        &json!({"replacementRateCents": 4200}),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["replacementRateCents"], 4200, "{set}");

    for bad in [json!(0), json!(-5)] {
        let (status, refused) = patch_json(
            &app,
            &format!("/api/v1/lenders/{lender_id}"),
            &json!({"replacementRateCents": bad}),
            &token,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
        assert_error(&refused, "validation_error");
    }
    let (status, still) = app
        .get_json_with_auth("/api/v1/lenders?includeArchived=true", &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{still}");
    let lender_key = lender_id.to_string();
    let row = still
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_str() == Some(lender_key.as_str()))
        .expect("lender");
    assert_eq!(row["replacementRateCents"], 4200, "{row}");
}

#[tokio::test]
async fn deposit_loss_notice_is_per_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let lender_id = lender(&app, &token, Some(1500)).await;
    let other_lender = lender(&app, &token, None).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let other_batch = batch(&app, &token, other_lender, site_id).await;
    let (biblio_id, items) = copies(&app, &token, 3, None).await;
    let (_other_biblio, other_items) = copies(&app, &token, 1, None).await;
    let (_own_biblio, own) = copies(&app, &token, 1, Some("10.00")).await;
    for (item_id, _) in &items {
        attach(&app, batch_id, site_id, *item_id).await;
    }
    attach(&app, other_batch, site_id, other_items[0].0).await;
    shift_return(&app, batch_id, 40).await;
    shift_return(&app, other_batch, 40).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "deplossn").await;

    let (status, biblio) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let embedded = biblio["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == items[0].0)
        .expect("copy");
    assert!(embedded["deposit"]["depositLossNotifiedAt"].is_null(), "{embedded}");

    let (status, lost_loan) = checkout(&app, &token, reader_id, items[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{lost_loan}");
    let lost_id = fixtures::json_id(&lost_loan["id"]);
    let (status, lost) = app
        .post_json(
            &format!("/api/v1/loans/{lost_id}/lost"),
            &json!({"bill": false}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{lost}");

    let (status, damage_loan) = checkout(&app, &token, reader_id, items[1].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{damage_loan}");
    let damage_id = fixtures::json_id(&damage_loan["id"]);
    let (status, damaged) = app
        .post_json(
            &format!("/api/v1/loans/{damage_id}/damaged"),
            &json!({"disposition": "return", "bill": false}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{damaged}");
    assert_eq!(damaged["charge"]["chargeType"], "damage");

    let (status, other_loan) = checkout(&app, &token, reader_id, other_items[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{other_loan}");
    let other_loan_id = fixtures::json_id(&other_loan["id"]);
    let (status, other_lost) = app
        .post_json(
            &format!("/api/v1/loans/{other_loan_id}/lost"),
            &json!({"bill": false, "amount": "4.00"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{other_lost}");

    let (status, shelf) = app
        .post_empty(
            &format!("/api/v1/items/{}/deposit-loss-notified", items[2].0),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{shelf}");
    assert_error(&shelf, "deposit_loss_not_declared");

    let (status, own_notice) = app
        .post_empty(
            &format!("/api/v1/items/{}/deposit-loss-notified", own[0].0),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{own_notice}");
    assert_error(&own_notice, "deposit_loss_not_declared");

    let (status, missing) = app
        .post_empty(
            "/api/v1/items/999999999999/deposit-loss-notified",
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert_error(&missing, "not_found");

    let (status, unknown_lender) = app
        .get_json_with_auth("/api/v1/deposit-losses?lenderId=999999999999", &token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{unknown_lender}");
    assert_error(&unknown_lender, "not_found");

    let list_uri = format!("/api/v1/deposit-losses?lenderId={lender_id}");
    let (status, open) = app.get_json_with_auth(&list_uri, &token).await;
    assert_eq!(status, StatusCode::OK, "{open}");
    let open_rows = open.as_array().unwrap();
    assert_eq!(open_rows.len(), 2, "{open}");
    let lost_key = items[0].0.to_string();
    let damage_key = items[1].0.to_string();
    let lost_row = open_rows
        .iter()
        .find(|row| row["itemId"].as_str() == Some(lost_key.as_str()))
        .expect("lost");
    let damage_row = open_rows
        .iter()
        .find(|row| row["itemId"].as_str() == Some(damage_key.as_str()))
        .expect("damaged");
    assert_eq!(lost_row["kind"], "lost", "{lost_row}");
    assert_eq!(lost_row["amountCents"], 1500, "{lost_row}");
    assert_eq!(lost_row["lenderId"], lender_id.to_string(), "{lost_row}");
    assert_eq!(lost_row["batchId"], batch_id.to_string(), "{lost_row}");
    assert!(lost_row["reference"].is_string(), "{lost_row}");
    assert!(lost_row["barcode"].is_string(), "{lost_row}");
    assert!(lost_row["fineId"].is_string(), "{lost_row}");
    assert!(lost_row["depositLossNotifiedAt"].is_null(), "{lost_row}");
    assert_eq!(damage_row["kind"], "damaged", "{damage_row}");
    assert_eq!(damage_row["amountCents"], 1500, "{damage_row}");

    let (status, dated) = app
        .post_json(
            &format!("/api/v1/items/{}/deposit-loss-notified", items[0].0),
            &json!({"notifiedOn": "2026-04-01"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{dated}");
    assert_eq!(dated["itemId"], lost_key, "{dated}");
    assert!(dated["depositLossNotifiedAt"].is_string(), "{dated}");
    let notified_day: chrono::NaiveDate = sqlx::query_scalar(
        r#"
        SELECT (deposit_loss_notified_at AT TIME ZONE COALESCE(
            (SELECT timezone FROM library_info WHERE id = 1),
            'Europe/Paris'
        ))::date
        FROM items WHERE id = $1
        "#,
    )
    .bind(items[0].0)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(notified_day, chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());

    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE event_type = 'deposit_loss_notified' AND entity_id = $1",
    )
    .bind(items[0].0)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(audits, 1);
    let (status, audit) = app
        .get_json_with_auth(
            "/api/v1/audit?eventType=deposit_loss_notified&perPage=20",
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{audit}");
    let lender_key = lender_id.to_string();
    let saw = audit["entries"].as_array().unwrap().iter().any(|entry| {
        entry["payload"]["lenderId"].as_str() == Some(lender_key.as_str())
            && entry["payload"]["itemId"].as_str() == Some(lost_key.as_str())
    });
    assert!(saw, "{audit}");

    let (status, again) = app
        .post_json(
            &format!("/api/v1/items/{}/deposit-loss-notified", items[0].0),
            &json!({"notifiedOn": "2026-05-02T15:04:05Z"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_error(&again, "deposit_loss_already_notified");
    let audits_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE event_type = 'deposit_loss_notified' AND entity_id = $1",
    )
    .bind(items[0].0)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(audits_after, 1);

    let (status, still_open) = app.get_json_with_auth(&list_uri, &token).await;
    assert_eq!(status, StatusCode::OK, "{still_open}");
    let still_rows = still_open.as_array().unwrap();
    assert_eq!(still_rows.len(), 1, "{still_open}");
    assert_eq!(still_rows[0]["itemId"], damage_key, "{still_open}");

    let (status, notified_list) = app
        .get_json_with_auth(&format!("{list_uri}&notified=true"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{notified_list}");
    assert_eq!(notified_list.as_array().unwrap().len(), 1, "{notified_list}");
    assert_eq!(notified_list[0]["itemId"], lost_key, "{notified_list}");

    let (status, stamped) = app
        .post_json(
            &format!("/api/v1/items/{}/deposit-loss-notified", items[1].0),
            &json!({"notifiedOn": "2026-05-02T15:04:05Z"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stamped}");
    assert_eq!(stamped["depositLossNotifiedAt"], "2026-05-02T15:04:05Z", "{stamped}");
    assert_eq!(stamped["kind"], "damaged", "{stamped}");

    let (status, item) = app
        .get_json_with_auth(&format!("/api/v1/items/{}", items[1].0), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(
        item["items"][0]["deposit"]["depositLossNotifiedAt"],
        "2026-05-02T15:04:05Z",
        "{item}"
    );
}
