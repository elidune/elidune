//! Incoming deposit batches: schema, CRUD, MARC import, and item JSON.
mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{fixtures, TestApp};
use elidune_server::marc::marc_record_with_copies;
use http_body_util::BodyExt;
use serde_json::{json, Value};

async fn patch_json(app: &TestApp, uri: &str, body: &Value, token: &str) -> (StatusCode, Value) {
    let response = app
        .request(
            Request::builder()
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
    let json = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap_or(Value::Null) };
    (status, json)
}

fn assert_error(body: &Value, code: &str) {
    assert_eq!(body["code"], code, "{body}");
    assert!(body["error"].is_string(), "{body}");
    assert!(body["message"].is_string(), "{body}");
}

async fn site(app: &TestApp, token: &str, name: &str) -> i64 {
    let (status, body) = app.post_json("/api/v1/sites", &json!({"name": format!("{name}-{}", fixtures::unique_token())}), Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["id"])
}

async fn lender(app: &TestApp, token: &str) -> (i64, String) {
    let name = format!("Lender {}", fixtures::unique_token());
    let (status, body) = app.post_json("/api/v1/lenders", &json!({"name": name, "contact": "desk@lender.test"}), Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (fixtures::json_id(&body["id"]), name)
}

async fn batch(app: &TestApp, token: &str, lender_id: i64, site_id: i64) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/deposit-batches",
            &json!({
                "lenderId": lender_id.to_string(),
                "reference": format!("CONV-{}", fixtures::unique_token()),
                "siteId": site_id.to_string(),
                "arrivedOn": "2026-03-01",
                "returnDueOn": "2026-09-01",
                "notes": "spring deposit"
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    fixtures::json_id(&body["id"])
}

async fn poll_task(app: &TestApp, token: &str, task_id: &str) -> Value {
    for _ in 0..50 {
        let (status, body) = app.get_json_with_auth(&format!("/api/v1/tasks/{task_id}"), token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        match body["status"].as_str() {
            Some("completed") => return body,
            Some("failed") => panic!("import task failed: {body}"),
            _ => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
        }
    }
    panic!("import task did not finish");
}

#[tokio::test]
async fn deposit_migration_and_batch_api() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let pool = app.state.services.repository.pool();

    let nullable: String = sqlx::query_scalar(
        "SELECT is_nullable FROM information_schema.columns
         WHERE table_name = 'items' AND column_name = 'deposit_batch_id'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(nullable, "YES");

    let delete_action: String = sqlx::query_scalar(
        "SELECT confdeltype::text
         FROM pg_constraint
         WHERE conname = 'items_deposit_batch_id_fkey'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(delete_action, "a", "deposit batch FK must not cascade");

    let site_id = site(&app, &token, "DepSite").await;
    let other_site = site(&app, &token, "OtherSite").await;
    let (lender_id, lender_name) = lender(&app, &token).await;
    let bad_dates = sqlx::query(
        "INSERT INTO deposit_batches (lender_id, reference, site_id, arrived_on, return_due_on)
         VALUES ($1, $2, $3, '2026-05-01', '2026-01-01')",
    )
    .bind(lender_id)
    .bind(format!("bad-{}", fixtures::unique_token()))
    .bind(site_id)
    .execute(pool)
    .await
    .expect_err("return_due_on must be on or after arrived_on");
    let sqlx::Error::Database(db_err) = bad_dates else {
        panic!("expected a database check failure");
    };
    assert_eq!(db_err.code().as_deref(), Some("23514"));

    let (status, dup) = app.post_json("/api/v1/lenders", &json!({"name": format!("  {}  ", lender_name.to_lowercase())}), Some(&token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{dup}");
    assert_error(&dup, "conflict");

    let (spare_id, _) = lender(&app, &token).await;
    let (status, archived) = app.delete_with_auth(&format!("/api/v1/lenders/{spare_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{archived}");
    assert!(archived["archivedAt"].is_string(), "{archived}");
    let (status, active_only) = app.get_json_with_auth("/api/v1/lenders", &token).await;
    assert_eq!(status, StatusCode::OK, "{active_only}");
    assert!(active_only.as_array().unwrap().iter().all(|row| fixtures::json_id(&row["id"]) != spare_id));
    let (status, with_archived) = app.get_json_with_auth("/api/v1/lenders?includeArchived=true", &token).await;
    assert_eq!(status, StatusCode::OK, "{with_archived}");
    assert!(with_archived
        .as_array()
        .unwrap()
        .iter()
        .any(|row| fixtures::json_id(&row["id"]) == spare_id && row["archivedAt"].is_string()));

    let batch_id = batch(&app, &token, lender_id, site_id).await;
    let (status, detail) = app.get_json_with_auth(&format!("/api/v1/deposit-batches/{batch_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["counts"]["total"], 0);
    assert_eq!(detail["counts"]["onShelf"], 0);
    assert_eq!(detail["status"], "active");

    let (status, blocked) = app.delete_with_auth(&format!("/api/v1/lenders/{lender_id}"), &token).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{blocked}");
    assert_error(&blocked, "lender_has_active_batches");

    let (status, own) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": "Own stock",
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("OWN-{}", fixtures::unique_token()),
                    "borrowable": true,
                    "siteId": site_id.to_string()
                }]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{own}");
    let own_item = fixtures::json_id(&own["biblio"]["items"][0]["id"]);
    assert!(own["biblio"]["items"][0]["deposit"].is_null());

    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(own_item)
        .bind(batch_id)
        .execute(pool)
        .await
        .unwrap();
    let deleted = sqlx::query("DELETE FROM deposit_batches WHERE id = $1").bind(batch_id).execute(pool).await;
    assert!(deleted.is_err(), "a batch with copies cannot be deleted");
    sqlx::query("UPDATE items SET deposit_batch_id = NULL WHERE id = $1").bind(own_item).execute(pool).await.unwrap();

    let returned = sqlx::query("UPDATE items SET deposit_returned_at = now() WHERE id = $1 AND deposit_batch_id IS NULL")
        .bind(own_item)
        .execute(pool)
        .await;
    assert!(returned.is_err(), "deposit_returned_at requires a deposit batch");

    let (status, moved) = patch_json(
        &app,
        &format!("/api/v1/deposit-batches/{batch_id}"),
        &json!({"siteId": other_site.to_string(), "notes": "still here"}),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(fixtures::json_id(&moved["siteId"]), other_site);
    let source_after: i64 = sqlx::query_scalar("SELECT site_id FROM items WHERE id = $1").bind(own_item).fetch_one(pool).await.unwrap();
    assert_eq!(source_after, site_id, "changing the batch site does not move copies");

    sqlx::query("UPDATE deposit_batches SET status = 'returning' WHERE id = $1").bind(batch_id).execute(pool).await.unwrap();
    let (status, inactive) = patch_json(&app, &format!("/api/v1/deposit-batches/{batch_id}"), &json!({"notes": "too late"}), &token).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{inactive}");
    assert_error(&inactive, "deposit_batch_not_active");
}

#[tokio::test]
async fn deposit_marc_import_isbn_match_and_barcode_report() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token, "ImpSite").await;
    let other_site = site(&app, &token, "ImpOther").await;
    let (lender_id, _) = lender(&app, &token).await;
    let batch_id = batch(&app, &token, lender_id, site_id).await;

    let (status, missing_source) = app.post_json("/api/v1/biblios/import-marc-batch?batchId=1", &json!({}), Some(&token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing_source}");
    assert_error(&missing_source, "validation_error");

    let (status, conflict) = app
        .post_json(
            &format!("/api/v1/biblios/import-marc-batch?batchId=1&deposit_batch_id={batch_id}&allowDuplicateIsbn=true"),
            &json!({}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{conflict}");
    assert_error(&conflict, "deposit_import_options_conflict");

    let (status, mismatch) = app
        .post_json(
            &format!("/api/v1/biblios/import-marc-batch?batchId=1&depositBatchId={batch_id}&sourceId={other_site}"),
            &json!({}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{mismatch}");
    assert_error(&mismatch, "deposit_site_mismatch");

    let taken = format!("TAKEN-{}", fixtures::unique_token());
    let (status, own) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": "Already catalogued",
                "mediaType": "printedText",
                "isbn": format!("979{}", fixtures::unique_token().chars().filter(|c| c.is_ascii_digit()).collect::<String>()),
                "lang": "french",
                "items": [{
                    "barcode": taken,
                    "borrowable": true,
                    "siteId": site_id.to_string()
                }]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{own}");
    let taken_item = fixtures::json_id(&own["biblio"]["items"][0]["id"]);
    assert!(own["biblio"]["items"][0]["deposit"].is_null());

    let digits: String = fixtures::unique_token().chars().filter(|c| c.is_ascii_digit()).collect();
    let isbn_match = format!("978{digits}");
    let isbn_new = format!("977{digits}");
    let keep_a = format!("LEND-A-{}", fixtures::unique_token());
    let keep_b = format!("LEND-B-{}", fixtures::unique_token());
    let keep_c = format!("LEND-C-{}", fixtures::unique_token());

    let batch_key = app
        .state
        .services
        .marc
        .cache_parsed_records(vec![
            marc_record_with_copies(&isbn_match, "Original title", &[&keep_a]),
            marc_record_with_copies(&isbn_match, "Must not replace the title", &[&keep_b]),
            marc_record_with_copies(&isbn_new, "Brand new notice", &[&taken, &keep_c]),
        ])
        .await
        .expect("cache marc records");

    let (status, accepted) = app
        .post_json(
            &format!("/api/v1/biblios/import-marc-batch?batchId={batch_key}&deposit_batch_id={batch_id}&sourceId={site_id}"),
            &json!({}),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{accepted}");
    let task_id = accepted["taskId"].as_str().expect("task id");
    let task = poll_task(&app, &token, task_id).await;
    let report = &task["result"];
    assert_eq!(report["created"].as_array().unwrap().len(), 2, "{report}");
    assert_eq!(report["attached"].as_array().unwrap().len(), 1, "{report}");
    assert!(report["imported"].as_array().unwrap().iter().any(|key| key == "0"), "{report}");
    assert_eq!(report["attached"][0]["key"], "1", "{report}");
    let relabeled = report["copiesRelabeled"].as_array().unwrap();
    assert_eq!(relabeled.len(), 1, "{report}");
    assert_eq!(relabeled[0]["recordKey"], "2");
    assert_eq!(relabeled[0]["lenderBarcode"], taken);
    assert_ne!(fixtures::json_id(&relabeled[0]["itemId"]), taken_item, "{report}");
    assert!(relabeled[0]["localBarcode"].as_str().unwrap().len() >= 10);

    let matched_id = fixtures::json_id(&report["attached"][0]["biblioId"]);
    let (status, matched) = app.get_json_with_auth(&format!("/api/v1/biblios/{matched_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{matched}");
    assert_eq!(matched["title"], "Original title");
    let copies = matched["items"].as_array().unwrap();
    assert_eq!(copies.len(), 2, "{matched}");
    for copy in copies {
        assert_eq!(copy["deposit"]["batchId"], batch_id.to_string());
        assert_eq!(copy["deposit"]["lenderId"], lender_id.to_string());
        assert!(!copy["deposit"]["lenderName"].as_str().unwrap().is_empty());
        assert!(copy["deposit"]["reference"].is_string());
        assert_eq!(copy["deposit"]["returnDueOn"], "2026-09-01");
        assert_eq!(copy["deposit"]["batchStatus"], "active");
        assert_eq!(fixtures::json_id(&copy["siteId"]), site_id);
        let barcode = copy["barcode"].as_str().unwrap();
        assert!(barcode == keep_a || barcode == keep_b, "{barcode}");
    }

    let (status, search) = app.get_json_with_auth(&format!("/api/v1/biblios?barcode={keep_c}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let found = &search["items"][0]["items"][0];
    assert_eq!(found["barcode"], keep_c);
    assert_eq!(found["deposit"]["batchId"], batch_id.to_string());
    assert!(found["deposit"]["reference"].as_str().unwrap().starts_with("CONV-"));

    let (status, desk) = app.get_json_with_auth(&format!("/api/v1/items/barcode/{keep_c}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{desk}");
    assert_eq!(desk["items"][0]["deposit"]["batchStatus"], "active");
    assert_eq!(desk["items"][0]["barcode"], keep_c);

    let (status, own_again) = app.get_json_with_auth(&format!("/api/v1/items/barcode/{taken}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{own_again}");
    assert!(own_again["items"][0]["deposit"].is_null());
}
