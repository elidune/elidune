//! Checkout and renew due dates respect the opening calendar.

mod common;

use std::sync::LazyLock;

use chrono::{Duration, NaiveDate, Utc};
use common::fixtures;
use common::TestApp;
use elidune_server::circulation_calendar::{adjust_due_date, OpeningCalendar};
use elidune_server::models::loan::CreateLoan;
use elidune_server::models::schedule::{
    CreateScheduleClosure, CreateSchedulePeriod, CreateScheduleSlot,
};
use serde_json::json;
use tokio::sync::Mutex;

/// Shared DB: serialize schedule writes so parallel tests do not steal each other's period.
static CALENDAR_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn create_loan(user_id: i64, item_id: i64) -> CreateLoan {
    CreateLoan {
        user_id,
        item_id: Some(item_id),
        item_identification: None,
        force: false,
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
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(
        status,
        axum::http::StatusCode::CREATED,
        "create biblio: {body}"
    );
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn seed_period_with_weekdays(
    app: &TestApp,
    open_weekdays: &[i16],
) -> elidune_server::models::schedule::SchedulePeriod {
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

fn expected_after_raw(raw: chrono::DateTime<Utc>, calendar: &OpeningCalendar) -> NaiveDate {
    adjust_due_date(raw, calendar).date_naive()
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
    let closed = OpeningCalendar::weekday_monday0(raw.date_naive());
    let open: Vec<i16> = (0..=6).filter(|d| *d != closed).collect();
    seed_period_with_weekdays(&app, &open).await;

    let repo = app.state.services.repository.as_ref();
    let before = Utc::now();
    let outcome = repo
        .loans_create(&create_loan(reader_id, item_id))
        .await
        .expect("checkout");
    let after = Utc::now();

    let calendar = OpeningCalendar::with_weekly(
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
        open,
    );
    let expected: std::collections::HashSet<_> = [before, after]
        .into_iter()
        .map(|t| expected_after_raw(t + Duration::days(21), &calendar))
        .collect();
    assert!(
        expected.contains(&outcome.expiry_at.date_naive()),
        "expiry {} not in {expected:?}",
        outcome.expiry_at.date_naive()
    );
    assert_ne!(
        OpeningCalendar::weekday_monday0(outcome.expiry_at.date_naive()),
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
    let holiday = raw.date_naive();
    app.state
        .services
        .repository
        .schedules_create_closure(&CreateScheduleClosure {
            closure_date: holiday.to_string(),
            reason: Some("holiday".into()),
        })
        .await
        .expect("create closure");

    let before = Utc::now();
    let outcome = app
        .state
        .services
        .repository
        .loans_create(&create_loan(reader_id, item_id))
        .await
        .expect("checkout");
    let after = Utc::now();

    let mut calendar = OpeningCalendar::with_weekly(
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
        0..=6,
    );
    for t in [before, after] {
        calendar = calendar.with_closure((t + Duration::days(21)).date_naive());
    }
    assert!(
        outcome.expiry_at.date_naive() > holiday
            || [before, after]
                .into_iter()
                .any(|t| expected_after_raw(t + Duration::days(21), &calendar)
                    == outcome.expiry_at.date_naive()),
        "expiry {} should skip closure {holiday}",
        outcome.expiry_at.date_naive()
    );
    assert_ne!(
        outcome.expiry_at.date_naive(),
        holiday,
        "due date must not land on the one-off closure"
    );
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
    let checkout = repo
        .loans_create(&create_loan(reader_id, item_id))
        .await
        .expect("checkout");

    let holiday = (Utc::now() + Duration::days(21)).date_naive();
    repo.schedules_create_closure(&CreateScheduleClosure {
        closure_date: holiday.to_string(),
        reason: Some("renew holiday".into()),
    })
    .await
    .expect("create closure");

    let before = Utc::now();
    let (new_expiry, renews) = repo.loans_renew(checkout.loan_id).await.expect("renew");
    let after = Utc::now();

    assert_eq!(renews, 1);
    assert_ne!(
        new_expiry.date_naive(),
        holiday,
        "renewed due date must skip the holiday"
    );
    let calendar = OpeningCalendar::with_weekly(
        NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2099, 12, 31).unwrap(),
        0..=6,
    )
    .with_closure(holiday);
    let expected: std::collections::HashSet<_> = [before, after]
        .into_iter()
        .map(|t| expected_after_raw(t + Duration::days(21), &calendar))
        .collect();
    assert!(
        expected.contains(&new_expiry.date_naive()),
        "renewed expiry {} not in {expected:?}",
        new_expiry.date_naive()
    );
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
    let closed = OpeningCalendar::weekday_monday0(raw.date_naive());
    let open: Vec<i16> = (0..=6).filter(|d| *d != closed).collect();
    seed_period_with_weekdays(&app, &open).await;

    app.state
        .dynamic_config
        .update_section(
            "circulation",
            json!({ "skip_closed_days": false, "overridable": true }),
        )
        .expect("disable skip_closed_days");

    let outcome = app
        .state
        .services
        .repository
        .loans_create(&create_loan(reader_id, item_id))
        .await
        .expect("checkout");

    assert_eq!(
        OpeningCalendar::weekday_monday0(outcome.expiry_at.date_naive()),
        closed,
        "with the policy off, due dates stay on the closed weekday"
    );
}
