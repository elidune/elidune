//! Checkout and renew due dates respect the opening calendar.

mod common;

use std::sync::LazyLock;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use common::fixtures;
use common::TestApp;
use elidune_server::circulation_calendar::OpeningCalendar;
use elidune_server::models::loan::CreateLoan;
use elidune_server::models::schedule::{CreateScheduleClosure, CreateSchedulePeriod, CreateScheduleSlot};
use elidune_server::renewal::RenewalOverride;
use elidune_server::services::calendar::{self, local_date, slide_due_date};
use serde_json::json;
use tokio::sync::Mutex;

/// Shared DB: serialize schedule writes so parallel tests do not steal each other's period.
static CALENDAR_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn create_loan(user_id: i64, item_id: i64) -> CreateLoan {
    create_loan_at(user_id, item_id, None)
}

fn create_loan_at(user_id: i64, item_id: i64, site_id: Option<i64>) -> CreateLoan {
    CreateLoan {
        user_id,
        item_id: Some(item_id),
        item_identification: None,
        force: false,
        site_id,
        override_reason: None,
    }
}

async fn seed_item(app: &TestApp, admin_token: &str, prefix: &str) -> i64 {
    let payload = json!({
        "title": format!("Calendar due {prefix}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": format!("{prefix}-{}", fixtures::unique_suffix()),
            "borrowable": true
        }]
    });
    let (status, body) = app.post_json("/api/v1/biblios", &payload, Some(admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn seed_period_with_weekdays(app: &TestApp, open_weekdays: &[i16]) -> elidune_server::models::schedule::SchedulePeriod {
    let repo = app.state.services.repository.as_ref();
    let suffix = fixtures::unique_suffix();
    // Latest start_date wins when periods overlap — keep this after leftover 2020 seeds.
    let start = (Utc::now() + Duration::days(20)).date_naive();
    let end = (Utc::now() + Duration::days(400)).date_naive();
    let period = repo
        .schedules_create_period(&CreateSchedulePeriod {
            name: format!("calendar-{suffix}"),
            start_date: start.to_string(),
            end_date: end.to_string(),
            notes: None,
        })
        .await
        .expect("create period");
    for &day in open_weekdays {
        repo.schedules_create_slot(
            period.id,
            &CreateScheduleSlot {
                day_of_week: day,
                open_time: "09:00".into(),
                close_time: "18:00".into(),
            },
        )
        .await
        .expect("create slot");
    }
    period
}

fn paris() -> chrono_tz::Tz {
    calendar::default_zone()
}

fn expected_after_raw(raw: chrono::DateTime<Utc>, calendar: &OpeningCalendar) -> NaiveDate {
    slide_due_date(raw, paris(), None, calendar).date_naive()
}

#[tokio::test]
async fn checkout_skips_weekly_closed_weekday() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "calweek").await;
    let item_id = seed_item(&app, &admin_token, "CALWEEK").await;

    // Close the weekday that a 21-day loan would land on.
    let raw = Utc::now() + Duration::days(21);
    let closed = OpeningCalendar::weekday_monday0(local_date(paris(), raw));
    let open: Vec<i16> = (0..=6).filter(|d| *d != closed).collect();
    seed_period_with_weekdays(&app, &open).await;

    let repo = app.state.services.repository.as_ref();
    let before = Utc::now();
    let outcome = repo.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");
    let after = Utc::now();

    let calendar = OpeningCalendar::with_weekly(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(), open);
    let expected: std::collections::HashSet<_> = [before, after].into_iter().map(|t| expected_after_raw(t + Duration::days(21), &calendar)).collect();
    assert!(expected.contains(&outcome.expiry_at.date_naive()), "expiry {} not in {expected:?}", outcome.expiry_at.date_naive());
    assert_ne!(
        OpeningCalendar::weekday_monday0(local_date(paris(), outcome.expiry_at)),
        closed,
        "due date must not land on the weekly closed day"
    );
}

#[tokio::test]
async fn checkout_skips_one_off_closure() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "calhol").await;
    let item_id = seed_item(&app, &admin_token, "CALHOL").await;

    seed_period_with_weekdays(&app, &(0..=6).collect::<Vec<_>>()).await;
    let raw = Utc::now() + Duration::days(21);
    let holiday = local_date(paris(), raw);
    let closure_id = app
        .state
        .services
        .repository
        .schedules_create_closure(&CreateScheduleClosure {
            closure_date: holiday.to_string(),
            reason: Some("holiday".into()),
            site_id: None,
        })
        .await
        .expect("create closure")
        .id;

    let before = Utc::now();
    let outcome = app.state.services.repository.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");
    let after = Utc::now();

    let mut calendar = OpeningCalendar::with_weekly(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(), 0..=6);
    for t in [before, after] {
        calendar = calendar.with_closure(local_date(paris(), t + Duration::days(21)));
    }
    let due_local = local_date(paris(), outcome.expiry_at);
    assert!(
        due_local > holiday
            || [before, after]
                .into_iter()
                .any(|t| { local_date(paris(), slide_due_date(t + Duration::days(21), paris(), None, &calendar),) == due_local }),
        "expiry {due_local} should skip closure {holiday}"
    );
    assert_ne!(due_local, holiday, "due date must not land on the one-off closure");
    delete_closures(&app, &[closure_id]).await;
}

#[tokio::test]
async fn renew_skips_holiday() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "calrenew").await;
    let item_id = seed_item(&app, &admin_token, "CALRENEW").await;

    seed_period_with_weekdays(&app, &(0..=6).collect::<Vec<_>>()).await;

    let repo = app.state.services.repository.as_ref();
    let checkout = repo.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");

    // Inside the renewal window. The new due date is this expiry plus the loan duration.
    let due = Utc::now() + Duration::days(2);
    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1")
        .bind(checkout.loan_id)
        .bind(due)
        .execute(repo.pool())
        .await
        .expect("move due date into the renewal window");

    let holiday = local_date(paris(), due + Duration::days(21));
    let closure_id = repo
        .schedules_create_closure(&CreateScheduleClosure {
            closure_date: holiday.to_string(),
            reason: Some("renew holiday".into()),
            site_id: None,
        })
        .await
        .expect("create closure")
        .id;

    let renewed = repo.loans_renew(checkout.loan_id, RenewalOverride::default()).await.expect("renew");
    let new_expiry = renewed.expiry_at;
    let renews = renewed.renew_count;

    assert_eq!(renews, 1);
    assert_ne!(local_date(paris(), new_expiry), holiday, "renewed due date must skip the holiday");
    let calendar = OpeningCalendar::with_weekly(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(), NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(), 0..=6).with_closure(holiday);
    let expected = expected_after_raw(due + Duration::days(21), &calendar);
    assert_eq!(
        new_expiry.date_naive(),
        expected,
        "renewed expiry {} should be current due + duration, skipping {holiday}",
        new_expiry.date_naive()
    );
    delete_closures(&app, &[closure_id]).await;
}

#[tokio::test]
async fn policy_off_keeps_closed_due_date() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "caloff").await;
    let item_id = seed_item(&app, &admin_token, "CALOFF").await;

    let raw = Utc::now() + Duration::days(21);
    // Policy off keeps the raw UTC instant, so the closed weekday is that UTC date.
    let closed = OpeningCalendar::weekday_monday0(raw.date_naive());
    let open: Vec<i16> = (0..=6).filter(|d| *d != closed).collect();
    seed_period_with_weekdays(&app, &open).await;

    app.state
        .dynamic_config
        .update_section("circulation", json!({ "skip_closed_days": false, "overridable": true }))
        .expect("disable skip_closed_days");

    let outcome = app.state.services.repository.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");

    assert_eq!(
        OpeningCalendar::weekday_monday0(outcome.expiry_at.date_naive()),
        closed,
        "with the policy off, due dates stay on the closed weekday"
    );
}

async fn create_site(app: &TestApp, admin_token: &str, name: &str) -> i64 {
    let (status, body) = app
        .post_json("/api/v1/sites", &json!({ "name": format!("{name}-{}", fixtures::unique_suffix()) }), Some(admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "create site: {body}");
    fixtures::json_id(&body["id"])
}

async fn seed_item_at(app: &TestApp, admin_token: &str, prefix: &str, site_id: Option<i64>) -> i64 {
    let mut item = json!({
        "barcode": format!("{prefix}-{}", fixtures::unique_suffix()),
        "borrowable": true
    });
    if let Some(site) = site_id {
        item["siteId"] = json!(site.to_string());
    }
    let payload = json!({
        "title": format!("Calendar due {prefix}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [item]
    });
    let (status, body) = app.post_json("/api/v1/biblios", &payload, Some(admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn closure_on(app: &TestApp, date: NaiveDate, site_id: Option<i64>, reason: &str) -> i64 {
    app.state
        .services
        .repository
        .schedules_create_closure(&CreateScheduleClosure {
            closure_date: date.to_string(),
            reason: Some(reason.into()),
            site_id,
        })
        .await
        .expect("create closure")
        .id
}

async fn delete_closures(app: &TestApp, ids: &[i64]) {
    let repo = app.state.services.repository.as_ref();
    for id in ids {
        let _ = repo.schedules_delete_closure(*id).await;
    }
}

#[tokio::test]
async fn library_timezone_round_trip_rejects_unknown_iana_names() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["timezone"], "Europe/Paris");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "timezone": "Not/AZone" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "invalid_timezone");

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "US", "timezone": "America/New_York" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "US");
    assert_eq!(body["timezone"], "America/New_York");

    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["timezone"], "America/New_York");

    let (status, body) = app
        .put_json(
            "/api/v1/library-info",
            &json!({ "name": body["name"], "addrCountry": "FR", "timezone": "Europe/Paris" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "restore timezone: {body}");
    assert_eq!(body["timezone"], "Europe/Paris");
}

#[tokio::test]
async fn library_country_must_be_iso_and_timezone_must_belong_to_it() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "France", "timezone": "Europe/Paris" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "FR");
    assert_eq!(body["timezone"], "Europe/Paris");

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "US", "timezone": "America/Chicago" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "US");
    assert_eq!(body["timezone"], "America/Chicago");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "addrCountry": "Narnia" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "validation_error");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "addrCountry": "XX" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "validation_error");

    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "US");
    assert_eq!(body["timezone"], "America/Chicago");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "timezone": "" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "timezone_required");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "addrCountry": "DE" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "timezone_required");

    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "US");
    assert_eq!(body["timezone"], "America/Chicago");

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "FR", "timezone": "America/New_York" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "timezone_country_mismatch");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "timezone": "Europe/Paris" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "timezone_country_mismatch");

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "FR", "timezone": "Europe/Paris" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "restore: {body}");
    assert_eq!(body["addrCountry"], "FR");
    assert_eq!(body["timezone"], "Europe/Paris");

    let (status, body) = app
        .put_json("/api/v1/library-info", &json!({ "addrCountry": "US", "timezone": "America/Chicago" }), Some(&admin_token))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "addrCountry": "FR", "timezone": "" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["addrCountry"], "FR");
    assert_eq!(body["timezone"], "Europe/Paris");
}

#[tokio::test]
async fn library_default_language_is_public_and_admin_writable() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "langreader").await;

    let column_default_is_null: bool = sqlx::query_scalar(
        r#"
        SELECT column_default IS NULL AND is_nullable = 'YES'
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'library_info'
          AND column_name = 'language'
        "#,
    )
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("language column metadata");
    assert!(column_default_is_null, "language column must default to NULL");

    let (status, mut body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(body.get("language").is_some(), "public library-info must include language: {body}");
    if !body["language"].is_null() {
        let (status, cleared) = app.put_json("/api/v1/library-info", &json!({ "language": null }), Some(&admin_token)).await;
        assert_eq!(status, axum::http::StatusCode::OK, "reset language: {cleared}");
        body = cleared;
    }
    assert!(body["language"].is_null(), "unset library language is null: {body}");

    let (status, public_body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{public_body}");
    assert!(public_body["language"].is_null(), "{public_body}");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "language": "de" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert_eq!(body["language"], "de");

    let (status, public_body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{public_body}");
    assert_eq!(public_body["language"], "de");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "language": "it" }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "validation_error");

    let (status, public_body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{public_body}");
    assert_eq!(public_body["language"], "de", "invalid write must not stick");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "language": "en" }), Some(&reader_token)).await;
    assert_eq!(status, axum::http::StatusCode::FORBIDDEN, "{body}");

    let (status, public_body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{public_body}");
    assert_eq!(public_body["language"], "de", "non-admin write must not stick");

    let (status, body) = app.put_json("/api/v1/library-info", &json!({ "language": null }), Some(&admin_token)).await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    assert!(body["language"].is_null(), "{body}");

    let (status, public_body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{public_body}");
    assert!(public_body["language"].is_null(), "{public_body}");

    let rejected = sqlx::query("UPDATE library_info SET language = 'it' WHERE id = 1").execute(app.state.services.repository.pool()).await;
    assert!(rejected.is_err(), "database check must reject languages outside fr, en, de, es");
}

#[tokio::test]
async fn three_week_network_closure_slides_checkout_and_renewal() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "calsummer").await;
    let item_id = seed_item(&app, &admin_token, "CALSUMMER").await;

    let raw_local = local_date(paris(), Utc::now() + Duration::days(21));
    let mut ids = Vec::new();
    for offset in 0..21 {
        let date = raw_local + Duration::days(offset);
        ids.push(closure_on(&app, date, None, "summer").await);
    }

    let outcome = app.state.services.repository.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");
    let due_local = local_date(paris(), outcome.expiry_at);
    assert!(due_local >= raw_local + Duration::days(21), "due {due_local} should slide past the 21-day closure starting {raw_local}");

    let repo = app.state.services.repository.as_ref();
    sqlx::query("UPDATE loans SET expiry_at = NOW() WHERE id = $1")
        .bind(outcome.loan_id)
        .execute(repo.pool())
        .await
        .expect("anchor renewal on today");
    let renewed_loan = repo.loans_renew(outcome.loan_id, RenewalOverride::default()).await.expect("renew across the closure");
    let renewed = renewed_loan.expiry_at;
    let renews = renewed_loan.renew_count;
    assert_eq!(renews, 1);
    let renewed_local = local_date(paris(), renewed);
    let renew_raw = local_date(paris(), Utc::now() + Duration::days(21));
    assert!(
        renewed_local >= renew_raw + Duration::days(21) || renewed_local > renew_raw,
        "renewed due {renewed_local} should slide past closures from {renew_raw}"
    );
    assert!(renewed_local >= raw_local + Duration::days(21) || renewed_local > renew_raw);

    delete_closures(&app, &ids).await;
}

#[tokio::test]
async fn site_closure_slides_only_that_sites_loan() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "calsitea").await;
    let (reader_b, _) = fixtures::create_reader(&app, &admin_token, "calsiteb").await;
    let site_a = create_site(&app, &admin_token, "SiteA").await;
    let site_b = create_site(&app, &admin_token, "SiteB").await;
    let item_b = seed_item_at(&app, &admin_token, "CALSITEB", Some(site_b)).await;

    let open_checkout = app
        .state
        .services
        .repository
        .loans_create(&create_loan_at(reader_b, item_b, Some(site_b)))
        .await
        .expect("checkout site B");
    let open_due = local_date(paris(), open_checkout.expiry_at);
    let closure_id = closure_on(&app, open_due, Some(site_a), "site-a only").await;

    let item_a = seed_item_at(&app, &admin_token, "CALSITEA", Some(site_a)).await;
    let closed_checkout = app
        .state
        .services
        .repository
        .loans_create(&create_loan_at(reader_a, item_a, Some(site_a)))
        .await
        .expect("checkout site A");
    let closed_due = local_date(paris(), closed_checkout.expiry_at);
    assert_ne!(closed_due, open_due, "site A closure on {open_due} must move that site's due date");
    assert!(closed_due > open_due, "{closed_due} should follow {open_due}");

    delete_closures(&app, &[closure_id]).await;
}

#[tokio::test]
async fn sunday_due_date_slides_to_monday() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "calsun").await;
    let item_id = seed_item(&app, &admin_token, "CALSUN").await;

    let today = local_date(paris(), Utc::now());
    let mut sunday = today;
    while sunday.weekday() != chrono::Weekday::Sun {
        sunday = sunday.pred_opt().expect("previous day");
    }
    let noon = chrono::NaiveTime::from_hms_opt(12, 0, 0).expect("noon");
    let anchor = calendar::local_to_utc(paris(), sunday, noon).expect("sunday noon");
    let target = sunday + Duration::days(21);
    assert_eq!(target.weekday(), chrono::Weekday::Sun);

    let repo = app.state.services.repository.as_ref();
    let period = repo
        .schedules_create_period(&CreateSchedulePeriod {
            name: format!("sunday-{}", fixtures::unique_suffix()),
            start_date: target.to_string(),
            end_date: (target + Duration::days(14)).to_string(),
            notes: None,
        })
        .await
        .expect("period");
    for day in 0..=5 {
        repo.schedules_create_slot(
            period.id,
            &CreateScheduleSlot {
                day_of_week: day,
                open_time: "09:00".into(),
                close_time: "18:00".into(),
            },
        )
        .await
        .expect("slot");
    }
    // Other tests in this file leave network closures (for example today+21).
    // On a Monday that date is the Monday this renewal must land on.
    sqlx::query("DELETE FROM schedule_closures WHERE site_id IS NULL AND closure_date >= $1 AND closure_date <= $2")
        .bind(target)
        .bind(target + Duration::days(14))
        .execute(repo.pool())
        .await
        .expect("clear network closures in the slide window");

    // Other tests in this file leave network closures and later-starting
    // periods in the shared database. Drop anything that covers the target
    // week so Sunday is closed only by the schedule created above.
    let window_end = target + Duration::days(7);
    sqlx::query("DELETE FROM schedule_closures WHERE closure_date >= $1 AND closure_date <= $2")
        .bind(target)
        .bind(window_end)
        .execute(repo.pool())
        .await
        .expect("clear closures on the slide window");
    sqlx::query("DELETE FROM schedule_periods WHERE id <> $1 AND end_date >= $2 AND start_date <= $3")
        .bind(period.id)
        .bind(target)
        .bind(window_end)
        .execute(repo.pool())
        .await
        .expect("clear overlapping periods");

    let checkout = repo.loans_create(&create_loan(reader_id, item_id)).await.expect("checkout");
    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1")
        .bind(checkout.loan_id)
        .bind(anchor)
        .execute(repo.pool())
        .await
        .expect("anchor on Sunday");

    let renewed = repo.loans_renew(checkout.loan_id, RenewalOverride::default()).await.expect("renew").expiry_at;
    assert_eq!(local_date(paris(), renewed), target + Duration::days(1), "Sunday due date slides to Monday");

    let _ = repo.schedules_delete_period(period.id).await;
}

#[tokio::test]
async fn return_after_closure_is_not_fined_for_closed_days() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_closed, _) = fixtures::create_reader(&app, &admin_token, "fineclosed").await;
    let (reader_open, _) = fixtures::create_reader(&app, &admin_token, "fineopen").await;
    let site = create_site(&app, &admin_token, "FineSite").await;
    let other = create_site(&app, &admin_token, "FineOther").await;
    let closed_item = seed_item_at(&app, &admin_token, "FINECL", Some(site)).await;
    let open_item = seed_item_at(&app, &admin_token, "FINEOP", Some(other)).await;
    let repo = app.state.services.repository.as_ref();

    let closed_loan = repo.loans_create(&create_loan_at(reader_closed, closed_item, Some(site))).await.expect("checkout closed site");
    let open_loan = repo.loans_create(&create_loan_at(reader_open, open_item, Some(other))).await.expect("checkout other site");

    let today = local_date(paris(), Utc::now());
    let due = today - Duration::days(30);
    let noon = chrono::NaiveTime::from_hms_opt(12, 0, 0).expect("noon");
    let expiry = calendar::local_to_utc(paris(), due, noon).expect("due noon");
    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1 OR id = $3")
        .bind(closed_loan.loan_id)
        .bind(expiry)
        .bind(open_loan.loan_id)
        .execute(repo.pool())
        .await
        .expect("backdate both loans");

    let calendar = repo.schedules_opening_calendar(due, today).await.expect("calendar");
    let mut to_close = Vec::new();
    let mut cursor = due;
    while let Some(next) = cursor.succ_opt() {
        if next > today {
            break;
        }
        if calendar.is_open_on(Some(site), next) && calendar.is_open_on(Some(other), next) {
            to_close.push(next);
            if to_close.len() == 21 {
                break;
            }
        }
        cursor = next;
    }
    assert!(to_close.len() >= 21, "need 21 open days to close, found {}", to_close.len());

    let mut ids = Vec::new();
    for date in &to_close {
        ids.push(closure_on(&app, *date, Some(site), "site fine closure").await);
    }

    let closed_result = app.state.services.fines.accrue_loan(closed_loan.loan_id).await.expect("accrue closed site");
    let open_result = app.state.services.fines.accrue_loan(open_loan.loan_id).await.expect("accrue other site");
    let closed_days = closed_result.breakdown.as_ref().expect("closed breakdown").overdue_days;
    let open_days = open_result.breakdown.as_ref().expect("open breakdown").overdue_days;
    assert_eq!(open_days - closed_days, 21, "site closure must drop exactly those 21 days (open {open_days}, site {closed_days})");

    delete_closures(&app, &ids).await;
}

async fn library_default_site(app: &TestApp) -> i64 {
    let sql = format!("SELECT id FROM sources WHERE id = {}", elidune_server::services::calendar::LIBRARY_DEFAULT_SITE_ID_SQL);
    sqlx::query_scalar(&sql).fetch_one(app.state.services.repository.pool()).await.expect("library default site")
}

/// Item home site A, checkout recorded at site B: due date, renewal, and the
/// stored column follow B. A loan with no recorded site follows the library
/// default site, including after that site is closed.
#[tokio::test]
async fn loan_recorded_at_another_site_follows_that_sites_closures() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "caldesk").await;
    let site_a = create_site(&app, &admin_token, "HomeA").await;
    let site_b = create_site(&app, &admin_token, "DeskB").await;
    let repo = app.state.services.repository.as_ref();

    let default_site = library_default_site(&app).await;
    assert_ne!(default_site, site_a);
    assert_ne!(default_site, site_b);
    let probe = seed_item_at(&app, &admin_token, "CALPROBE", Some(site_b)).await;
    let open_checkout = repo.loans_create(&create_loan_at(reader_id, probe, Some(site_b))).await.expect("probe checkout");
    let open_due = local_date(paris(), open_checkout.expiry_at);
    let closure_id = closure_on(&app, open_due, Some(site_b), "desk closed").await;

    let away = seed_item_at(&app, &admin_token, "CALAWAY", Some(site_a)).await;
    let at_desk = repo.loans_create(&create_loan_at(reader_id, away, Some(site_b))).await.expect("checkout at desk B");
    let desk_due = local_date(paris(), at_desk.expiry_at);
    assert_ne!(desk_due, open_due, "site B closure on {open_due} must move a loan made at B");
    assert!(desk_due > open_due, "{desk_due} should follow {open_due}");
    let stored_desk: Option<i64> = sqlx::query_scalar("SELECT site_id FROM loans WHERE id = $1")
        .bind(at_desk.loan_id)
        .fetch_one(repo.pool())
        .await
        .expect("read loan site");
    assert_eq!(stored_desk, Some(site_b));

    let home_item = seed_item_at(&app, &admin_token, "CALHOME", Some(site_a)).await;
    let at_home = repo.loans_create(&create_loan(reader_id, home_item)).await.expect("checkout with no recorded site");
    assert_eq!(
        local_date(paris(), at_home.expiry_at),
        open_due,
        "a null loan site follows the library default site, which is still open"
    );
    let stored_home: Option<i64> = sqlx::query_scalar("SELECT site_id FROM loans WHERE id = $1")
        .bind(at_home.loan_id)
        .fetch_one(repo.pool())
        .await
        .expect("read null loan site");
    assert_eq!(stored_home, None);

    sqlx::query("UPDATE loans SET expiry_at = NOW() WHERE id = $1 OR id = $2")
        .bind(at_desk.loan_id)
        .bind(at_home.loan_id)
        .execute(repo.pool())
        .await
        .expect("anchor both renewals on today");
    let renewed_home = repo.loans_renew(at_home.loan_id, RenewalOverride::default()).await.expect("renew home-site loan").expiry_at;
    let renewed_desk = repo.loans_renew(at_desk.loan_id, RenewalOverride::default()).await.expect("renew desk-site loan").expiry_at;
    let home_renewed = local_date(paris(), renewed_home);
    let desk_renewed = local_date(paris(), renewed_desk);
    assert_eq!(home_renewed, open_due, "renewal of a null loan site stays on the open default-site day");
    assert!(desk_renewed > home_renewed, "renewal at site B must slide past {home_renewed}, got {desk_renewed}");

    let default_closure = closure_on(&app, open_due, Some(default_site), "default closed").await;
    let (reader_null, _) = fixtures::create_reader(&app, &admin_token, "calnull").await;
    let null_item = seed_item_at(&app, &admin_token, "CALNULL", Some(site_a)).await;
    let slid = repo.loans_create(&create_loan(reader_null, null_item)).await.expect("null loan after the default site is closed");
    let slid_due = local_date(paris(), slid.expiry_at);
    assert!(slid_due > open_due, "a null loan site follows the default site closure on {open_due}, got {slid_due}");
    let stored_slid: Option<i64> = sqlx::query_scalar("SELECT site_id FROM loans WHERE id = $1")
        .bind(slid.loan_id)
        .fetch_one(repo.pool())
        .await
        .expect("read slid loan site");
    assert_eq!(stored_slid, None);

    delete_closures(&app, &[closure_id, default_closure]).await;
}

/// Overdue days for a null `loans.site_id` follow the library default site.
/// The copy's home site stays open, so a fallback to `items.source_id` would not drop these days.
#[tokio::test]
async fn fine_null_site_follows_default_site() {
    let _guard = CALENDAR_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_null, _) = fixtures::create_reader(&app, &admin_token, "finedef").await;
    let (reader_other, _) = fixtures::create_reader(&app, &admin_token, "fineother").await;
    let default_site = library_default_site(&app).await;
    let home = create_site(&app, &admin_token, "FineHome").await;
    let other = create_site(&app, &admin_token, "FineOther").await;
    assert_ne!(default_site, home);
    assert_ne!(default_site, other);
    let null_item = seed_item_at(&app, &admin_token, "FINENULL", Some(home)).await;
    let other_item = seed_item_at(&app, &admin_token, "FINEOTH", Some(home)).await;
    let repo = app.state.services.repository.as_ref();

    let at_null = repo.loans_create(&create_loan(reader_null, null_item)).await.expect("checkout with no recorded site");
    let at_other = repo.loans_create(&create_loan_at(reader_other, other_item, Some(other))).await.expect("checkout at another site");

    let today = local_date(paris(), Utc::now());
    let due = today - Duration::days(30);
    let noon = chrono::NaiveTime::from_hms_opt(12, 0, 0).expect("noon");
    let expiry = calendar::local_to_utc(paris(), due, noon).expect("due noon");
    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1 OR id = $3")
        .bind(at_null.loan_id)
        .bind(expiry)
        .bind(at_other.loan_id)
        .execute(repo.pool())
        .await
        .expect("backdate both loans");

    let opening = repo.schedules_opening_calendar(due, today).await.expect("calendar");
    let mut to_close = Vec::new();
    let mut cursor = due;
    while let Some(next) = cursor.succ_opt() {
        if next > today {
            break;
        }
        if opening.is_open_on(Some(default_site), next) && opening.is_open_on(Some(other), next) {
            to_close.push(next);
            if to_close.len() == 21 {
                break;
            }
        }
        cursor = next;
    }
    assert!(to_close.len() >= 21, "need 21 open days to close, found {}", to_close.len());
    let mut ids = Vec::new();
    for date in &to_close {
        ids.push(closure_on(&app, *date, Some(default_site), "default fine closure").await);
    }

    let null_result = app.state.services.fines.accrue_loan(at_null.loan_id).await.expect("accrue null-site loan");
    let other_result = app.state.services.fines.accrue_loan(at_other.loan_id).await.expect("accrue other-site loan");
    let null_days = null_result.breakdown.as_ref().expect("null breakdown").overdue_days;
    let other_days = other_result.breakdown.as_ref().expect("other breakdown").overdue_days;
    assert_eq!(other_days - null_days, 21, "default-site closures drop those days (other {other_days}, null {null_days})");

    delete_closures(&app, &ids).await;
}
