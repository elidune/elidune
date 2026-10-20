//! Reader public type is derived from the birth date (issue #264).

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use axum::http::StatusCode;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use common::{fixtures, TestApp};
use elidune_server::repository::MaintenanceRepository;
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static SNOWFLAKE_WORKER: AtomicU32 = AtomicU32::new(80);

async fn test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().await
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

/// Birth date whose anniversary is today, so the completed age is exactly `age`.
fn birthdate_aged(age: i32) -> String {
    let today = today();
    let date = today
        .with_year(today.year() - age)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(today.year() - age, 2, 28).expect("feb 28"));
    date.to_string()
}

/// Birth date whose next anniversary is tomorrow, so the completed age is `age - 1`.
fn birthdate_day_before_age(age: i32) -> String {
    let tomorrow = today() + Duration::days(1);
    let date = tomorrow
        .with_year(tomorrow.year() - age)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(tomorrow.year() - age, 2, 28).expect("feb 28"));
    date.to_string()
}

fn reader_body(login: &str, birthdate: &str, public_type: Option<i64>) -> Value {
    let mut body = json!({
        "login": login,
        "password": "readerpass1234",
        "firstname": "Ada",
        "lastname": "Reader",
        "email": format!("{login}@test.local"),
        "accountType": "reader",
        "sex": "f",
        "birthdate": birthdate,
        "addrCity": "Paris"
    });
    if let Some(id) = public_type {
        body["publicType"] = json!(id.to_string());
    }
    body
}

async fn type_id(app: &TestApp, token: &str, name: &str) -> i64 {
    fixtures::public_type_id_by_name(app, token, name).await
}

async fn create_reader(app: &TestApp, token: &str, birthdate: &str, public_type: Option<i64>) -> (StatusCode, Value) {
    let login = format!("rpt_{}", fixtures::unique_suffix());
    app.post_json("/api/v1/users", &reader_body(&login, birthdate, public_type), Some(token)).await
}

fn snowflake() -> i64 {
    let worker = SNOWFLAKE_WORKER.fetch_add(1, Ordering::Relaxed) % 1022 + 1;
    snowflaked::Generator::new(worker as u16).generate::<i64>()
}

#[tokio::test]
async fn age_bounds_are_inclusive_and_client_public_type_is_ignored() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let child = type_id(&app, &token, "child").await;
    let adult = type_id(&app, &token, "adult").await;
    let senior = type_id(&app, &token, "senior").await;
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "rptbound").await;

    let login = format!("rpt12_{}", fixtures::unique_suffix());
    let mut body = reader_body(&login, &birthdate_aged(12), Some(adult));
    body["guardianId"] = json!(guardian_id.to_string());
    let (status, body) = app.post_json("/api/v1/users", &body, Some(&token)).await;
    assert_eq!(status, StatusCode::CREATED, "age 12 is child: {body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), child, "{body}");
    assert_eq!(body["birthDateMissing"], false, "{body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(13), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "age 13: {body}");
    assert_eq!(body["code"], "no_public_type_for_age", "{body}");
    assert!(
        body["message"].as_str().unwrap_or("").to_ascii_lowercase().contains("age band"),
        "message must ask an admin to complete the bands: {body}"
    );

    let (status, body) = create_reader(&app, &token, &birthdate_day_before_age(18), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "day before 18: {body}");
    assert_eq!(body["code"], "no_public_type_for_age", "{body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(18), Some(child)).await;
    assert_eq!(status, StatusCode::CREATED, "age 18 is adult: {body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), adult, "client publicType is ignored: {body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(59), None).await;
    assert_eq!(status, StatusCode::CREATED, "age 59 is adult: {body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), adult, "{body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(60), Some(adult)).await;
    assert_eq!(status, StatusCode::CREATED, "age 60 is senior: {body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), senior, "narrower senior band wins over adult: {body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(99), None).await;
    assert_eq!(status, StatusCode::CREATED, "age 99 is senior: {body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), senior, "{body}");

    let (status, body) = create_reader(&app, &token, &birthdate_aged(100), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "age 100: {body}");
    assert_eq!(body["code"], "no_public_type_for_age", "{body}");
}

#[tokio::test]
async fn new_reader_without_birth_date_is_rejected() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let adult = type_id(&app, &token, "adult").await;
    let login = format!("rptmiss_{}", fixtures::unique_suffix());
    let mut body = reader_body(&login, "1990-01-01", Some(adult));
    body.as_object_mut().expect("object").remove("birthdate");
    let (status, created) = app.post_json("/api/v1/users", &body, Some(&token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{created}");
    assert_eq!(created["code"], "birth_date_required", "{created}");
}

#[tokio::test]
async fn existing_reader_without_birth_date_keeps_public_type() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let adult = type_id(&app, &token, "adult").await;
    let child = type_id(&app, &token, "child").await;
    let (status, created) = create_reader(&app, &token, &birthdate_aged(40), Some(child)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let user_id = fixtures::json_id(&created["id"]);
    assert_eq!(fixtures::json_id(&created["publicType"]), adult, "{created}");

    sqlx::query("UPDATE users SET birthdate = NULL WHERE id = $1")
        .bind(user_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("clear birthdate");

    let (status, updated) = app
        .put_json(
            &format!("/api/v1/users/{user_id}"),
            &json!({
                "phone": "0102030405",
                "publicType": child.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "missing birth date must not 422: {updated}");
    assert_eq!(
        fixtures::json_id(&updated["publicType"]),
        adult,
        "stored public type is kept and the client value is ignored: {updated}"
    );
    assert_eq!(updated["birthDateMissing"], true, "{updated}");
    assert!(updated["birthdate"].is_null(), "{updated}");
}

#[tokio::test]
async fn staff_public_type_is_not_derived() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let staff_type = type_id(&app, &token, "staff").await;
    let login = format!("rptstaff_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Sam",
                "lastname": "Staff",
                "email": format!("{login}@test.local"),
                "accountType": "librarian",
                "publicType": staff_type.to_string(),
                "sex": "m",
                "birthdate": birthdate_aged(40),
                "addrCity": "Lyon"
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(fixtures::json_id(&body["publicType"]), staff_type, "staff keep the client public type: {body}");
    assert_eq!(body["birthDateMissing"], false, "{body}");
}

#[tokio::test]
async fn overlapping_age_bands_are_rejected_on_save_only() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (status, listed) = app.get_json_with_auth("/api/v1/public-types", &token).await;
    assert_eq!(status, StatusCode::OK, "reads stay valid with seeded overlap: {listed}");
    let names: Vec<&str> = listed.as_array().expect("list").iter().filter_map(|row| row["name"].as_str()).collect();
    assert!(names.contains(&"adult") && names.contains(&"senior"), "{listed}");

    let adult = type_id(&app, &token, "adult").await;
    let (status, rejected) = app.put_json(&format!("/api/v1/public-types/{adult}"), &json!({ "label": "Adult" }), Some(&token)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "saving a band that still overlaps is rejected: {rejected}");
    assert_eq!(rejected["code"], "public_type_age_overlap", "{rejected}");
    let message = rejected["message"].as_str().unwrap_or("");
    assert!(message.contains("adult"), "{rejected}");
    assert!(message.contains("senior"), "{rejected}");

    let base = 400 + (fixtures::unique_suffix() % 20_000) as i16;
    let left = format!("band{base}");
    let right = format!("band{}", base + 2);
    let (status, created) = app
        .post_json("/api/v1/public-types", &json!({ "name": left, "label": left, "ageMin": base, "ageMax": base }), Some(&token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "gap band: {created}");

    let (status, overlap) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": right,
                "label": right,
                "ageMin": base,
                "ageMax": base
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{overlap}");
    assert_eq!(overlap["code"], "public_type_age_overlap", "{overlap}");
    let message = overlap["message"].as_str().unwrap_or("");
    assert!(message.contains(&left), "{overlap}");
    assert!(message.contains(&right), "{overlap}");
}

#[tokio::test]
async fn recompute_on_update_leaves_ongoing_loans_and_holds() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let child = type_id(&app, &token, "child").await;
    let adult = type_id(&app, &token, "adult").await;
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "rptguard").await;

    let login = format!("rptchild_{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Mini",
                "lastname": "Reader",
                "email": format!("{login}@test.local"),
                "accountType": "reader",
                "publicType": adult.to_string(),
                "sex": "f",
                "birthdate": birthdate_aged(10),
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(fixtures::json_id(&created["publicType"]), child, "{created}");
    let user_id = fixtures::json_id(&created["id"]);

    let pool = app.state.services.repository.pool();
    let biblio_id: i64 = sqlx::query_scalar("INSERT INTO biblios (id, title, media_type, lang) VALUES ($1, $2, 'printedText', 'fre') RETURNING id")
        .bind(snowflake())
        .bind(format!("RPT {}", fixtures::unique_suffix()))
        .fetch_one(pool)
        .await
        .expect("biblio");
    let item_id: i64 = sqlx::query_scalar("INSERT INTO items (id, biblio_id, barcode, borrowable) VALUES ($1, $2, $3, TRUE) RETURNING id")
        .bind(snowflake())
        .bind(biblio_id)
        .bind(format!("RPT-{}", fixtures::unique_suffix()))
        .fetch_one(pool)
        .await
        .expect("item");
    let loan_expiry = Utc::now() + Duration::days(21);
    let loan_id: i64 = sqlx::query_scalar("INSERT INTO loans (id, user_id, item_id, date, expiry_at) VALUES ($1, $2, $3, NOW(), $4) RETURNING id")
        .bind(snowflake())
        .bind(user_id)
        .bind(item_id)
        .bind(loan_expiry)
        .fetch_one(pool)
        .await
        .expect("loan");
    let hold_expiry = Utc::now() + Duration::days(7);
    let hold_id = snowflake();
    sqlx::query("INSERT INTO holds (id, user_id, biblio_id, status, expires_at) VALUES ($1, $2, $3, 'pending', $4)")
        .bind(hold_id)
        .bind(user_id)
        .bind(biblio_id)
        .bind(hold_expiry)
        .execute(pool)
        .await
        .expect("hold");

    let (status, updated) = app.put_json(&format!("/api/v1/users/{user_id}"), &json!({ "birthdate": birthdate_aged(40) }), Some(&token)).await;
    assert_eq!(status, StatusCode::OK, "recompute on update: {updated}");
    assert_eq!(fixtures::json_id(&updated["publicType"]), adult, "{updated}");
    assert_eq!(updated["birthDateMissing"], false, "{updated}");

    let stored_loan: chrono::DateTime<Utc> = sqlx::query_scalar("SELECT expiry_at FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(pool)
        .await
        .expect("loan expiry");
    assert!(
        (stored_loan - loan_expiry).num_milliseconds().abs() < 2,
        "ongoing loan keeps its due date ({stored_loan} vs {loan_expiry})"
    );
    let stored_hold: chrono::DateTime<Utc> = sqlx::query_scalar("SELECT expires_at FROM holds WHERE id = $1")
        .bind(hold_id)
        .fetch_one(pool)
        .await
        .expect("hold expiry");
    assert!(
        (stored_hold - hold_expiry).num_milliseconds().abs() < 2,
        "ongoing hold keeps its expiry ({stored_hold} vs {hold_expiry})"
    );
}

#[tokio::test]
async fn maintenance_recomputes_readers_only() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let child = type_id(&app, &token, "child").await;
    let adult = type_id(&app, &token, "adult").await;

    let (status, reader) = create_reader(&app, &token, &birthdate_aged(40), None).await;
    assert_eq!(status, StatusCode::CREATED, "{reader}");
    let reader_id = fixtures::json_id(&reader["id"]);
    assert_eq!(fixtures::json_id(&reader["publicType"]), adult);

    let login = format!("rptlib_{}", fixtures::unique_suffix());
    let (status, staff) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Lib",
                "lastname": "Rarian",
                "email": format!("{login}@test.local"),
                "accountType": "librarian",
                "publicType": adult.to_string(),
                "sex": "f",
                "birthdate": birthdate_aged(40),
                "addrCity": "Nice"
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{staff}");
    let staff_id = fixtures::json_id(&staff["id"]);

    let pool = app.state.services.repository.pool();
    sqlx::query("UPDATE users SET birthdate = $1, public_type = $2 WHERE id = $3")
        .bind(NaiveDate::parse_from_str(&birthdate_aged(8), "%Y-%m-%d").unwrap())
        .bind(adult)
        .bind(reader_id)
        .execute(pool)
        .await
        .expect("age the reader down without the API");
    sqlx::query("UPDATE users SET birthdate = $1 WHERE id = $2")
        .bind(NaiveDate::parse_from_str(&birthdate_aged(8), "%Y-%m-%d").unwrap())
        .bind(staff_id)
        .execute(pool)
        .await
        .expect("age the librarian down");

    app.state.services.repository.maintenance_cleanup_users().await.expect("cleanup");

    let reader_type: Option<i64> = sqlx::query_scalar("SELECT public_type FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(pool)
        .await
        .expect("reader type");
    assert_eq!(reader_type, Some(child), "daily job recomputes readers");
    let staff_type: Option<i64> = sqlx::query_scalar("SELECT public_type FROM users WHERE id = $1")
        .bind(staff_id)
        .fetch_one(pool)
        .await
        .expect("staff type");
    assert_eq!(staff_type, Some(adult), "staff are left unchanged");
}

#[tokio::test]
async fn put_reader_with_no_matching_band_is_422_and_writes_nothing() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let stored = birthdate_aged(40);
    let (status, created) = create_reader(&app, &token, &stored, None).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let user_id = fixtures::json_id(&created["id"]);
    let public_type = fixtures::json_id(&created["publicType"]);

    let (status, rejected) = app
        .put_json(
            &format!("/api/v1/users/{user_id}"),
            &json!({
                "birthdate": birthdate_aged(13),
                "phone": "0999999999"
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected}");
    assert_eq!(rejected["code"], "no_public_type_for_age", "{rejected}");

    let (status, current) = app.get_json_with_auth(&format!("/api/v1/users/{user_id}"), &token).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["birthdate"], stored, "{current}");
    assert_eq!(fixtures::json_id(&current["publicType"]), public_type, "{current}");
    assert!(current["phone"].is_null(), "rejected update wrote {current}");
}

#[tokio::test]
async fn staff_and_admin_are_created_without_birth_date() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let adult = type_id(&app, &token, "adult").await;

    for account_type in ["librarian", "admin"] {
        let login = format!("rpt{account_type}_{}", fixtures::unique_suffix());
        let (status, body) = app
            .post_json(
                "/api/v1/users",
                &json!({
                    "login": login,
                    "password": "readerpass1234",
                    "firstname": "No",
                    "lastname": "Birth",
                    "email": format!("{login}@test.local"),
                    "accountType": account_type,
                    "publicType": adult.to_string(),
                    "sex": "m",
                    "addrCity": "Lyon"
                }),
                Some(&token),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{account_type}: {body}");
        assert!(body["birthdate"].is_null(), "{body}");
        assert_eq!(body["birthDateMissing"], false, "{body}");
        assert_eq!(fixtures::json_id(&body["publicType"]), adult, "public type is not computed: {body}");
        assert_eq!(body["accountType"], account_type, "{body}");
    }
}

#[tokio::test]
async fn self_service_accepts_the_stored_birth_date_and_rejects_a_different_one() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "rptself").await;
    let (status, before) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "{before}");
    let stored = before["birthdate"].as_str().unwrap().to_string();

    let (status, saved) = app
        .put_json(
            "/api/v1/auth/profile",
            &json!({
                "birthdate": stored,
                "phone": "0611111111"
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "same birth date on profile: {saved}");
    assert_eq!(saved["birthdate"], stored, "{saved}");
    assert_eq!(saved["phone"], "0611111111", "{saved}");

    let (status, rejected) = app
        .put_json(
            "/api/v1/auth/profile",
            &json!({
                "birthdate": "1980-01-02",
                "phone": "0622222222"
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "different birth date: {rejected}");
    assert_eq!(rejected["code"], "authorization_failed", "{rejected}");

    let (status, current) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["birthdate"], stored, "{current}");
    assert_eq!(current["phone"], "0611111111", "profile 403 wrote {current}");

    let (status, own_saved) = app
        .put_json(
            &format!("/api/v1/users/{reader_id}"),
            &json!({
                "birthdate": stored,
                "email": "same.birth@library.test"
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "same birth date on own record: {own_saved}");
    assert_eq!(own_saved["birthdate"], stored, "{own_saved}");
    assert_eq!(own_saved["email"], "same.birth@library.test", "{own_saved}");

    let (status, own_rejected) = app
        .put_json(
            &format!("/api/v1/users/{reader_id}"),
            &json!({
                "birthdate": "1981-03-04",
                "email": "changed.birth@library.test"
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "own record birth date: {own_rejected}");

    let (status, current) = app.get_json_with_auth(&format!("/api/v1/users/{reader_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["birthdate"], stored, "{current}");
    assert_eq!(current["email"], "same.birth@library.test", "{current}");
    assert_eq!(current["phone"], "0611111111", "{current}");

    let senior = type_id(&app, &admin_token, "senior").await;
    let next = birthdate_aged(70);
    let (status, staff_saved) = app.put_json(&format!("/api/v1/users/{reader_id}"), &json!({ "birthdate": next }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "users write may change birth date: {staff_saved}");
    assert_eq!(staff_saved["birthdate"], next, "{staff_saved}");
    assert_eq!(fixtures::json_id(&staff_saved["publicType"]), senior, "{staff_saved}");
}
