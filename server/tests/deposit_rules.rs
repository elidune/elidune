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
    let (_biblio, items) = copies(&app, &token, 3, None).await;
    for (item_id, _) in &items {
        attach(&app, batch_id, site_id, *item_id).await;
    }
    shift_return(&app, batch_id, 40).await;
    let (reader_id, _) = fixtures::create_reader(&app, &token, "depclose").await;
    let (status, loan) = checkout(&app, &token, reader_id, items[2].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    assert_eq!(loan["dueDateCappedByDeposit"], false, "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

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
    let (status, report) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["returned"].as_array().unwrap().len(), 1);
    assert_eq!(report["missing"].as_array().unwrap().len(), 1);
    assert_eq!(report["onLoan"].as_array().unwrap().len(), 1);
    assert_eq!(report["onLoan"][0]["barcode"], items[2].1);
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1")
        .bind(batch_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let archived: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM items WHERE deposit_batch_id = $1 AND archived_at IS NOT NULL AND id <> $2",
    )
    .bind(batch_id)
    .bind(items[2].0)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap();
    assert_eq!(archived, 2);
    let loan_archived: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT archived_at FROM items WHERE id = $1")
            .bind(items[2].0)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert!(loan_archived.is_none());

    let (status, missing) = app
        .get_json_with_auth(
            &format!("/api/v1/deposit-batches/{batch_id}/items?state=missing"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    assert_eq!(missing.as_array().unwrap().len(), 1);
    assert_eq!(missing[0]["barcode"], items[1].1);

    let (status, again_report) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/close"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{again_report}");
    assert_eq!(again_report, report);

    let (status, closed) = app
        .post_empty(
            &format!("/api/v1/deposit-batches/{batch_id}/start-return"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{closed}");
    assert_error(&closed, "deposit_batch_not_active");

    let (status, returned) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&token))
        .await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    let archived_after: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT archived_at FROM items WHERE id = $1")
            .bind(items[2].0)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert!(archived_after.is_some());
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
    let own_stock = stats["items"]["ownStock"].as_i64().unwrap();
    let total = stats["items"]["total"].as_i64().unwrap();
    let deposit_total: i64 = deposits
        .iter()
        .map(|row| row["count"].as_i64().unwrap())
        .sum();
    assert_eq!(own_stock + deposit_total, total);

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

    let (_biblio, rated_items) = copies(&app, &token, 2, Some("20.00")).await;
    attach(&app, rated_batch, site_id, rated_items[0].0).await;
    attach(&app, rated_batch, site_id, rated_items[1].0).await;
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

    let flagged: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT loss_flagged_at FROM lenders WHERE id = $1")
            .bind(rated)
            .fetch_one(app.state.services.repository.pool())
            .await
            .unwrap();
    assert!(flagged.is_some());

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
    attach(&app, unrated_batch, site_id, plain[0].0).await;
    attach(&app, unrated_batch, site_id, plain[1].0).await;
    shift_return(&app, unrated_batch, 40).await;
    let (status, loan) = checkout(&app, &token, reader_id, plain[0].0, false).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    let missing_loan = fixtures::json_id(&loan["id"]);
    let (status, missing) = app
        .post_json(
            &format!("/api/v1/loans/{missing_loan}/lost"),
            &json!({"bill": true}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");
    assert_error(&missing, "deposit_replacement_amount_required");
    let (status, zero_amount) = app
        .post_json(
            &format!("/api/v1/loans/{missing_loan}/lost"),
            &json!({"bill": true, "amount": "0"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{zero_amount}");
    assert_error(&zero_amount, "deposit_replacement_amount_required");
    let (status, billed) = app
        .post_json(
            &format!("/api/v1/loans/{missing_loan}/lost"),
            &json!({"bill": false, "amount": "8.50"}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{billed}");
    assert_eq!(billed["charge"]["amount"], "8.50");

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
}
