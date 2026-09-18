//! Legal guardian for minor patrons (`child` / `school`) — part of #54.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use axum::http::StatusCode;
use chrono::Utc;
use common::{fixtures, TestApp};
use elidune_server::repository::Repository;
use once_cell::sync::Lazy;
use serde_json::json;
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static SNOWFLAKE_WORKER: AtomicU32 = AtomicU32::new(70);

async fn test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().await
}

#[tokio::test]
async fn create_child_without_guardian_is_rejected() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    reject_minor_without_guardian(&app, &admin_token, "child").await;
}

#[tokio::test]
async fn create_school_without_guardian_is_rejected() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    reject_minor_without_guardian(&app, &admin_token, "school").await;
}

#[tokio::test]
async fn create_child_with_guardian_stores_patron_to_patron_link() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (guardian_id, _) = fixtures::create_reader(&app, &admin_token, "guard").await;

    let child_type = fixtures::public_type_id_by_name(&app, &admin_token, "child").await;
    let login = format!("minor_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Minor",
                "lastname": login,
                "email": format!("{login}@child.local"),
                "accountType": "reader",
                "publicType": child_type.to_string(),
                "sex": "f",
                "birthdate": "2015-06-01",
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create child: {body}");
    assert_eq!(
        body["guardianId"].as_str().unwrap_or(""),
        guardian_id.to_string()
    );

    let child_id = fixtures::json_id(&body["id"]);
    let (status, got) = app
        .get_json_with_auth(&format!("/api/v1/users/{child_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "get child: {got}");
    assert_eq!(
        got["guardianId"].as_str().unwrap_or(""),
        guardian_id.to_string()
    );
}

#[tokio::test]
async fn overdue_reminders_for_a_minor_go_to_the_guardian() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let suffix = fixtures::unique_suffix();
    let guardian_email = format!("guardian_{suffix}@test.local");
    let child_email = format!("child_{suffix}@test.local");

    let (guardian_id, _) = create_reader_with_email(&app, &admin_token, "g", &guardian_email).await;
    let child_id =
        create_minor_with_guardian(&app, &admin_token, guardian_id, "child", &child_email).await;

    let loan_a = insert_overdue_loan(&repo, child_id, 40, 0).await;
    let loan_b = insert_overdue_loan(&repo, child_id, 40, 0).await;

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "send reminders: {body}");

    let to_guardian = detail_for_email(&body, &guardian_email);
    let to_child = detail_for_email(&body, &child_email);
    assert!(
        to_guardian.is_some(),
        "reminder must be addressed to the guardian: {body}"
    );
    assert!(
        to_child.is_none(),
        "reminder must not be addressed to the minor: {body}"
    );
    assert_eq!(to_guardian.unwrap()["loanCount"].as_u64().unwrap_or(0), 2);
    assert_eq!(fixtures::json_id(&to_guardian.unwrap()["userId"]), child_id);

    let _ = (loan_a, loan_b);
}

async fn reject_minor_without_guardian(app: &TestApp, admin_token: &str, type_name: &str) {
    let public_type = fixtures::public_type_id_by_name(app, admin_token, type_name).await;
    let login = format!("{type_name}_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Minor",
                "lastname": login,
                "email": format!("{login}@test.local"),
                "accountType": "reader",
                "publicType": public_type.to_string(),
                "sex": "f",
                "birthdate": "2015-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "enrol {type_name} without guardian: {body}"
    );
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains("guardianId"),
        "error must mention guardianId: {body}"
    );
}

async fn create_reader_with_email(
    app: &TestApp,
    admin_token: &str,
    login: &str,
    email: &str,
) -> (i64, String) {
    let login = format!("{login}_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Guardian",
                "lastname": login,
                "email": email,
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "sex": "m",
                "birthdate": "1980-01-15",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create guardian: {body}");
    (fixtures::json_id(&body["id"]), login)
}

async fn create_minor_with_guardian(
    app: &TestApp,
    admin_token: &str,
    guardian_id: i64,
    type_name: &str,
    email: &str,
) -> i64 {
    let public_type = fixtures::public_type_id_by_name(app, admin_token, type_name).await;
    let login = format!("{type_name}_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Minor",
                "lastname": login,
                "email": email,
                "accountType": "reader",
                "publicType": public_type.to_string(),
                "sex": "f",
                "birthdate": "2015-06-01",
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create minor: {body}");
    fixtures::json_id(&body["id"])
}

fn detail_for_email<'a>(body: &'a serde_json::Value, email: &str) -> Option<&'a serde_json::Value> {
    body["details"]
        .as_array()?
        .iter()
        .find(|d| d["email"].as_str() == Some(email))
}

fn snowflake() -> i64 {
    let worker = SNOWFLAKE_WORKER.fetch_add(1, Ordering::Relaxed) % 1022 + 1;
    snowflaked::Generator::new(worker as u16).generate::<i64>()
}

async fn insert_overdue_loan(
    repo: &Repository,
    user_id: i64,
    overdue_days: i64,
    reminder_count: i32,
) -> i64 {
    let suffix = Utc::now().timestamp_micros();
    let biblio_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO biblios (id, title, media_type, lang)
        VALUES ($1, $2, 'printedText', 'fre')
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(format!("Minor overdue {suffix}"))
    .fetch_one(repo.pool())
    .await
    .expect("insert biblio");

    let item_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO items (id, biblio_id, barcode, borrowable)
        VALUES ($1, $2, $3, TRUE)
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(biblio_id)
    .bind(format!("MIN-{suffix}"))
    .fetch_one(repo.pool())
    .await
    .expect("insert item");

    sqlx::query_scalar(
        r#"
        INSERT INTO loans (id, user_id, item_id, date, expiry_at, reminder_count)
        VALUES (
            $1, $2, $3,
            NOW() - ($4 || ' days')::INTERVAL - INTERVAL '14 days',
            NOW() - ($4 || ' days')::INTERVAL,
            $5
        )
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(user_id)
    .bind(item_id)
    .bind(overdue_days)
    .bind(reminder_count)
    .fetch_one(repo.pool())
    .await
    .expect("insert overdue loan")
}
