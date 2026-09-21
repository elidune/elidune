//! Legal guardian for `child` patrons — part of #54 / #67.
//! `school` is a collectivity and does not require a guardian.

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
    reject_child_without_guardian(&app, &admin_token).await;
}

#[tokio::test]
async fn create_school_without_guardian_succeeds() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let school_type = fixtures::public_type_id_by_name(&app, &admin_token, "school").await;
    let login = format!("school_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "College",
                "lastname": login,
                "email": format!("{login}@school.local"),
                "accountType": "reader",
                "publicType": school_type.to_string(),
                "sex": "m",
                "birthdate": "1990-01-01",
                "addrCity": "Paris"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "enrol school without guardian: {body}"
    );
    assert!(
        body["guardianId"].is_null(),
        "school must not require a guardian: {body}"
    );
}

#[tokio::test]
async fn child_or_school_cannot_be_the_guardian() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (adult_id, _) = fixtures::create_reader(&app, &admin_token, "adult_g").await;
    let child_id = create_child_with_guardian(
        &app,
        &admin_token,
        adult_id,
        &format!("ward_{}@child.local", fixtures::unique_suffix()),
    )
    .await;
    reject_non_major_guardian(&app, &admin_token, child_id, "child").await;

    let school_id = create_school_without_guardian(&app, &admin_token).await;
    reject_non_major_guardian(&app, &admin_token, school_id, "school").await;
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
    let child_id = create_child_with_guardian(&app, &admin_token, guardian_id, &child_email).await;

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

#[tokio::test]
async fn update_child_can_replace_guardian() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (first_id, _) = fixtures::create_reader(&app, &admin_token, "g1").await;
    let (second_id, _) = fixtures::create_reader(&app, &admin_token, "g2").await;
    let child_id = create_child_with_guardian(
        &app,
        &admin_token,
        first_id,
        &format!("ward_rep_{}@child.local", fixtures::unique_suffix()),
    )
    .await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{child_id}"),
            &json!({ "guardianId": second_id.to_string() }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "replace guardian: {body}");
    assert_eq!(
        body["guardianId"].as_str().unwrap_or(""),
        second_id.to_string(),
        "replace must persist the new guardian: {body}"
    );

    let got = get_user(&app, &admin_token, child_id).await;
    assert_eq!(
        got["guardianId"].as_str().unwrap_or(""),
        second_id.to_string(),
        "GET must show the replacement guardian: {got}"
    );
}

#[tokio::test]
async fn update_child_cannot_clear_guardian() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (guardian_id, _) = fixtures::create_reader(&app, &admin_token, "gkeep").await;
    let child_id = create_child_with_guardian(
        &app,
        &admin_token,
        guardian_id,
        &format!("ward_clr_{}@child.local", fixtures::unique_suffix()),
    )
    .await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{child_id}"),
            &json!({ "guardianId": null }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "clear while child must be rejected: {body}"
    );
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains("guardianId") && message.contains("child") && !message.contains("school"),
        "error must mention child only: {body}"
    );

    let got = get_user(&app, &admin_token, child_id).await;
    assert_eq!(
        got["guardianId"].as_str().unwrap_or(""),
        guardian_id.to_string(),
        "failed clear must leave the guardian unchanged: {got}"
    );
}

#[tokio::test]
async fn update_can_clear_guardian_when_leaving_child() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (guardian_id, _) = fixtures::create_reader(&app, &admin_token, "gleave").await;
    let child_id = create_child_with_guardian(
        &app,
        &admin_token,
        guardian_id,
        &format!("ward_leave_{}@child.local", fixtures::unique_suffix()),
    )
    .await;
    let adult_type = fixtures::public_type_id_by_name(&app, &admin_token, "adult").await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{child_id}"),
            &json!({
                "publicType": adult_type.to_string(),
                "guardianId": null
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "clear guardian when leaving child: {body}"
    );
    assert!(
        body["guardianId"].is_null(),
        "guardian must be cleared after leaving child: {body}"
    );
    assert_eq!(
        fixtures::json_id(&body["publicType"]),
        adult_type,
        "public type must become adult: {body}"
    );
}

#[tokio::test]
async fn update_child_omitting_guardian_keeps_link() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (guardian_id, _) = fixtures::create_reader(&app, &admin_token, "gomit").await;
    let child_id = create_child_with_guardian(
        &app,
        &admin_token,
        guardian_id,
        &format!("ward_omit_{}@child.local", fixtures::unique_suffix()),
    )
    .await;

    let (status, body) = app
        .put_json(
            &format!("/api/v1/users/{child_id}"),
            &json!({ "firstname": "StillMinor" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "omit guardian on update: {body}");
    assert_eq!(
        body["guardianId"].as_str().unwrap_or(""),
        guardian_id.to_string(),
        "omitting guardianId must leave the link unchanged: {body}"
    );
}

async fn get_user(app: &TestApp, admin_token: &str, id: i64) -> serde_json::Value {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/users/{id}"), admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "get user {id}: {body}");
    body
}

async fn reject_child_without_guardian(app: &TestApp, admin_token: &str) {
    let public_type = fixtures::public_type_id_by_name(app, admin_token, "child").await;
    let login = format!("child_{}", fixtures::unique_suffix());
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
        "enrol child without guardian: {body}"
    );
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains("guardianId") && message.contains("child") && !message.contains("school"),
        "error must mention child only: {body}"
    );
}

async fn reject_non_major_guardian(app: &TestApp, admin_token: &str, guardian_id: i64, kind: &str) {
    let child_type = fixtures::public_type_id_by_name(app, admin_token, "child").await;
    let login = format!("ward_{kind}_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Ward",
                "lastname": login,
                "email": format!("{login}@child.local"),
                "accountType": "reader",
                "publicType": child_type.to_string(),
                "sex": "f",
                "birthdate": "2015-06-01",
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "{kind} must not be accepted as guardian: {body}"
    );
    let message = body["message"].as_str().unwrap_or("");
    assert!(
        message.contains("major") || message.contains("child") || message.contains("school"),
        "error must reject a non-major guardian: {body}"
    );
}

async fn create_school_without_guardian(app: &TestApp, admin_token: &str) -> i64 {
    let school_type = fixtures::public_type_id_by_name(app, admin_token, "school").await;
    let login = format!("school_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "College",
                "lastname": login,
                "email": format!("{login}@school.local"),
                "accountType": "reader",
                "publicType": school_type.to_string(),
                "sex": "m",
                "birthdate": "1990-01-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create school: {body}");
    fixtures::json_id(&body["id"])
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

async fn create_child_with_guardian(
    app: &TestApp,
    admin_token: &str,
    guardian_id: i64,
    email: &str,
) -> i64 {
    let public_type = fixtures::public_type_id_by_name(app, admin_token, "child").await;
    let login = format!("child_{}", fixtures::unique_suffix());
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
