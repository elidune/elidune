//! Open-loan checkout conflicts (#168) and the pool deadlock they used to cause (#169).

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use common::fixtures;
use common::TestApp;
use elidune_server::models::loan::Loan;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::task::JoinSet;
use tower::ServiceExt;

const OTHER: &str = "item_on_loan_to_other_borrower";
const SAME: &str = "item_already_on_loan_to_borrower";

async fn post_loan(router: axum::Router, token: &str, body: &Value) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/v1/loans")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 3000))));
    let response = router.oneshot(req).await.expect("router oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn loan_body(user_id: i64, item_id: Option<i64>, barcode: Option<&str>, force: bool) -> Value {
    json!({
        "userId": user_id.to_string(),
        "itemId": item_id.map(|id| id.to_string()),
        "itemIdentification": barcode,
        "force": force,
    })
}

fn assert_conflict(status: StatusCode, body: &Value, code: &str) {
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], code, "{body}");
    assert_eq!(body["error"], "Conflict", "{body}");
    let message = body["message"].as_str().unwrap_or("");
    assert!(!message.is_empty(), "{body}");
    if code == SAME {
        assert!(
            message.to_lowercase().contains("renew"),
            "message should point at renewal: {message}"
        );
    }
}

async fn seed_item(app: &TestApp, token: &str, barcode: &str) -> i64 {
    let payload = json!({
        "title": format!("Open loan {barcode}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{ "barcode": barcode, "borrowable": true }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn loan_row(app: &TestApp, loan_id: i64) -> Loan {
    app.state
        .services
        .repository
        .loans_get_by_id(loan_id)
        .await
        .expect("active loan")
}

async fn archive_count(app: &TestApp, item_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM loans_archives WHERE item_id = $1")
        .bind(item_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("archive count")
}

async fn clone_readers(app: &TestApp, template_id: i64, n: usize, tag: &str) -> Vec<i64> {
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        let login = format!("{tag}_{i}_{}", fixtures::unique_suffix());
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO users (
                login, password, firstname, lastname, email,
                birthdate, account_type, public_type, sex, addr_city,
                status, created_at, update_at, expiry_at, must_change_password
            )
            SELECT $1, password, firstname, lastname, $2,
                   birthdate, account_type, public_type, sex, addr_city,
                   status, NOW(), NOW(), expiry_at, false
            FROM users WHERE id = $3
            RETURNING id
            "#,
        )
        .bind(&login)
        .bind(format!("{login}@test.local"))
        .bind(template_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("clone reader");
        ids.push(id);
    }
    ids
}

#[tokio::test]
async fn open_loan_is_409_for_other_and_same_borrower_including_admin_and_barcode() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "open_a").await;
    let (reader_b, _) = fixtures::create_reader(&app, &admin_token, "open_b").await;
    let admin_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE login = 'testadmin'")
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("admin id");

    let suffix = fixtures::unique_suffix();
    let barcode = format!("OPEN-{suffix}");
    let item_id = seed_item(&app, &admin_token, &barcode).await;

    let (status, created) = app
        .post_json(
            "/api/v1/loans",
            &loan_body(reader_a, Some(item_id), None, false),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);
    let before = loan_row(&app, loan_id).await;
    assert_eq!(archive_count(&app, item_id).await, 0);

    for force in [false, true] {
        let (status, body) = app
            .post_json(
                "/api/v1/loans",
                &loan_body(reader_b, Some(item_id), None, force),
                Some(&admin_token),
            )
            .await;
        assert_conflict(status, &body, OTHER);
    }

    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &loan_body(admin_id, None, Some(&barcode), true),
            Some(&admin_token),
        )
        .await;
    assert_conflict(status, &body, OTHER);

    for force in [false, true] {
        let (status, body) = app
            .post_json(
                "/api/v1/loans",
                &loan_body(reader_a, None, Some(&barcode), force),
                Some(&admin_token),
            )
            .await;
        assert_conflict(status, &body, SAME);
    }

    let after = loan_row(&app, loan_id).await;
    assert_eq!(format!("{before:?}"), format!("{after:?}"));
    assert!(after.returned_at.is_none());
    assert_eq!(after.user_id, reader_a);
    assert_eq!(
        app.state
            .services
            .repository
            .loans_get_active_ids_for_item(item_id)
            .await
            .unwrap(),
        vec![loan_id]
    );
    assert_eq!(archive_count(&app, item_id).await, 0);
}

async fn burst(
    app: &TestApp,
    token: &str,
    item_id: i64,
    users: &[i64],
    force: bool,
) -> Vec<(StatusCode, Value)> {
    assert_eq!(
        app.state
            .services
            .repository
            .pool()
            .options()
            .get_max_connections(),
        5,
        "burst must run against the 5-connection test pool"
    );
    let started = Instant::now();
    let mut set = JoinSet::new();
    for user_id in users {
        let router = app.router.clone();
        let token = token.to_string();
        let body = loan_body(*user_id, Some(item_id), None, force);
        set.spawn(async move { post_loan(router, &token, &body).await });
    }
    let mut results = Vec::with_capacity(users.len());
    while let Some(joined) = set.join_next().await {
        results.push(joined.expect("loan task"));
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(20),
        "burst of {} took {elapsed:?}; a pool deadlock shows up near 30s",
        users.len()
    );
    results
}

#[tokio::test]
async fn burst_of_twenty_checkouts_one_loan_no_500() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (template, _) = fixtures::create_reader(&app, &admin_token, "burst_tpl").await;
    let users = clone_readers(&app, template, 20, "burst").await;

    for force in [false, true] {
        let barcode = format!("BURST-{}-{}", force, fixtures::unique_suffix());
        let item_id = seed_item(&app, &admin_token, &barcode).await;
        let results = burst(&app, &admin_token, item_id, &users, force).await;

        let created = results
            .iter()
            .filter(|(status, _)| *status == StatusCode::CREATED)
            .count();
        let conflicts = results
            .iter()
            .filter(|(status, body)| *status == StatusCode::CONFLICT && body["code"] == OTHER)
            .count();
        let failures: Vec<_> = results
            .iter()
            .filter(|(status, body)| {
                *status != StatusCode::CREATED
                    && !(*status == StatusCode::CONFLICT && body["code"] == OTHER)
            })
            .collect();
        assert!(
            failures.is_empty(),
            "force={force}: unexpected responses {failures:?}"
        );
        assert_eq!(created, 1, "force={force}: {results:?}");
        assert_eq!(conflicts, 19, "force={force}");
        assert!(
            results
                .iter()
                .all(|(status, _)| *status != StatusCode::INTERNAL_SERVER_ERROR),
            "force={force}: 500 in {results:?}"
        );
        assert_eq!(
            app.state
                .services
                .repository
                .loans_count_active_for_item(item_id)
                .await
                .unwrap(),
            1
        );
    }
}

fn batch_item<'a>(body: &'a Value, barcode: &str) -> &'a Value {
    body["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|row| row["barcode"] == barcode)
        .unwrap_or_else(|| panic!("missing batch row for {barcode}: {body}"))
}

#[tokio::test]
async fn batch_create_reports_open_loan_codes() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "batch_a").await;
    let (reader_b, _) = fixtures::create_reader(&app, &admin_token, "batch_b").await;
    let suffix = fixtures::unique_suffix();
    let other_barcode = format!("BATCH-OTHER-{suffix}");
    let same_barcode = format!("BATCH-SAME-{suffix}");
    let missing_barcode = format!("BATCH-MISSING-{suffix}");
    let other_item = seed_item(&app, &admin_token, &other_barcode).await;
    let same_item = seed_item(&app, &admin_token, &same_barcode).await;

    for (user_id, item_id) in [(reader_a, other_item), (reader_b, same_item)] {
        let (status, body) = app
            .post_json(
                "/api/v1/loans",
                &loan_body(user_id, Some(item_id), None, false),
                Some(&admin_token),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    let (status, body) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_b.to_string(),
                "barcodes": [other_barcode, same_barcode, missing_barcode],
                "force": true,
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["created"], 0);
    assert_eq!(body["errors"], 3);

    let other = batch_item(&body, &other_barcode);
    assert_eq!(other["success"], false);
    assert_eq!(other["code"], OTHER);
    let other_error = other["error"].as_str().expect("error string");
    assert!(
        other_error.contains("another borrower"),
        "error text should stay the human message: {other_error}"
    );

    let same = batch_item(&body, &same_barcode);
    assert_eq!(same["success"], false);
    assert_eq!(same["code"], SAME);
    let same_error = same["error"].as_str().expect("error string");
    assert!(
        same_error.to_lowercase().contains("renew"),
        "error text should still point at renewal: {same_error}"
    );

    let missing = batch_item(&body, &missing_barcode);
    assert_eq!(missing["success"], false);
    assert_eq!(missing["code"], "not_found");
    assert!(missing["error"]
        .as_str()
        .unwrap_or("")
        .contains("Not found"));

    assert_eq!(
        app.state
            .services
            .repository
            .loans_count_active_for_item(other_item)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        app.state
            .services
            .repository
            .loans_count_active_for_item(same_item)
            .await
            .unwrap(),
        1
    );
}
