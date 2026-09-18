//! Graduated overdue reminders: tiers, no skip, stop-on-return, per-tier enabled flags.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use axum::http::StatusCode;
use chrono::Utc;
use common::{fixtures, TestApp};
use elidune_server::repository::{loans::ReminderTierDelays, Repository};
use once_cell::sync::Lazy;
use serde_json::json;
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static SNOWFLAKE_WORKER: AtomicU32 = AtomicU32::new(40);

async fn test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_LOCK.lock().await
}

fn delays() -> ReminderTierDelays {
    ReminderTierDelays::default()
}

#[tokio::test]
async fn does_not_skip_or_double_send_a_tier() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;

    let (loan_id, email) = seed_overdue_loan(&repo, 40, 0, None).await;

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "first send: {body}");
    assert_eq!(tier_for_email(&body, &email), Some("first"));

    simulate_delivery(&repo, loan_id).await;
    let count = reminder_count(&repo, loan_id).await;
    assert_eq!(count, 1, "first send must advance reminder_count by one");

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "second send: {body}");
    assert_eq!(
        tier_for_email(&body, &email),
        Some("second"),
        "must not skip to formalNotice: {body}"
    );

    simulate_delivery(&repo, loan_id).await;
    assert_eq!(reminder_count(&repo, loan_id).await, 2);

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "formal send: {body}");
    assert_eq!(tier_for_email(&body, &email), Some("formalNotice"));

    simulate_delivery(&repo, loan_id).await;
    assert_eq!(reminder_count(&repo, loan_id).await, 3);

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "fourth send: {body}");
    assert_eq!(
        tier_for_email(&body, &email),
        None,
        "must not send the same tier twice after the chain is done: {body}"
    );
}

#[tokio::test]
async fn groups_same_tier_loans_into_one_mail_per_patron() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;

    let (loan_a, email) = seed_overdue_loan(&repo, 40, 0, None).await;
    let loan_b = seed_second_loan_for_user(&repo, loan_a, 40, 0).await;

    let before = pending_to(&repo, &email).await;
    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "grouped send: {body}");

    let detail = detail_for_email(&body, &email).expect("detail for patron");
    assert_eq!(detail["tier"], "first");
    assert_eq!(detail["loanCount"].as_u64().unwrap_or(0), 2);
    assert_eq!(pending_to(&repo, &email).await, before + 1);

    let eligible = repo
        .loans_get_overdue_for_reminders(delays())
        .await
        .expect("eligible");
    assert!(eligible
        .iter()
        .all(|row| row.loan_id != loan_a && row.loan_id != loan_b));
}

#[tokio::test]
async fn stops_when_returned_lost_or_claimed_returned() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;

    let (returned_id, returned_email) = seed_overdue_loan(&repo, 40, 0, None).await;
    sqlx::query("UPDATE loans SET returned_at = NOW() WHERE id = $1")
        .bind(returned_id)
        .execute(repo.pool())
        .await
        .expect("mark returned");

    let (lost_id, lost_email) = seed_overdue_loan(&repo, 40, 0, Some(1)).await;
    sqlx::query("UPDATE loans SET returned_at = NOW() WHERE id = $1")
        .bind(lost_id)
        .execute(repo.pool())
        .await
        .expect("close lost loan");

    let (claimed_id, claimed_email) = seed_overdue_loan(&repo, 40, 0, Some(3)).await;

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    assert_eq!(
        tier_for_email(&body, &returned_email),
        None,
        "returned: {body}"
    );
    assert_eq!(tier_for_email(&body, &lost_email), None, "lost: {body}");
    assert_eq!(
        tier_for_email(&body, &claimed_email),
        None,
        "claimed-returned: {body}"
    );

    let eligible = repo
        .loans_get_overdue_for_reminders(delays())
        .await
        .expect("eligible");
    let ids: Vec<i64> = eligible.iter().map(|r| r.loan_id).collect();
    assert!(!ids.contains(&returned_id));
    assert!(!ids.contains(&lost_id));
    assert!(!ids.contains(&claimed_id));
}

#[tokio::test]
async fn disabled_formal_notice_stops_at_second_and_does_not_loop_it() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;

    disable_formal(&app);

    let (loan_id, email) = seed_overdue_loan(&repo, 40, 1, None).await;

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "second send: {body}");
    assert_eq!(tier_for_email(&body, &email), Some("second"));

    simulate_delivery(&repo, loan_id).await;
    assert_eq!(reminder_count(&repo, loan_id).await, 2);

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "must not loop: {body}");
    assert_eq!(
        tier_for_email(&body, &email),
        None,
        "disabled formal notice must not resend the second reminder: {body}"
    );
}

#[tokio::test]
async fn disabling_second_reminder_also_disables_formal_notice() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = admin_token(&app).await;

    let mut value = serde_json::to_value(app.state.dynamic_config.read_reminders()).unwrap();
    value["second_reminder_enabled"] = json!(false);
    value["formal_notice_enabled"] = json!(true);

    let (status, body) = app
        .put_json(
            "/api/v1/admin/config/reminders",
            &json!({ "value": value }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "put reminders: {body}");
    assert_eq!(body["value"]["second_reminder_enabled"], false);
    assert_eq!(
        body["value"]["formal_notice_enabled"], false,
        "write must close the hole: {body}"
    );

    let (status, get_body) = app
        .get_json_with_auth("/api/v1/admin/config", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "get config: {get_body}");
    let reminders = get_body["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .find(|s| s["key"] == "reminders")
        .expect("reminders section");
    assert_eq!(reminders["value"]["second_reminder_enabled"], false);
    assert_eq!(reminders["value"]["formal_notice_enabled"], false);
}

#[tokio::test]
async fn loan_already_at_highest_enabled_tier_is_not_sent_again() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;

    disable_formal(&app);

    let (loan_id, email) = seed_overdue_loan(&repo, 40, 2, None).await;

    let overdue = app
        .get_json_with_auth("/api/v1/loans/overdue", &admin_token)
        .await;
    assert_eq!(overdue.0, StatusCode::OK, "overdue list: {}", overdue.1);
    let listed = overdue.1["loans"]
        .as_array()
        .expect("loans")
        .iter()
        .find(|l| loan_id_of(l) == Some(loan_id));
    if let Some(row) = listed {
        assert!(
            row["nextReminderTier"].is_null(),
            "highest enabled tier must show no next tier: {row}"
        );
    }

    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    assert_eq!(
        tier_for_email(&body, &email),
        None,
        "loan already at the highest enabled tier must not be mailed: {body}"
    );
}

#[tokio::test]
async fn overdue_list_reports_next_tier() {
    let _guard = test_guard().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let repo = app.state.services.repository.as_ref().clone();
    let admin_token = admin_token(&app).await;
    let (loan_id, _) = seed_overdue_loan(&repo, 10, 0, None).await;

    let (status, body) = app
        .get_json_with_auth("/api/v1/loans/overdue", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "overdue: {body}");
    let row = body["loans"]
        .as_array()
        .expect("loans")
        .iter()
        .find(|l| loan_id_of(l) == Some(loan_id))
        .expect("seeded loan");
    assert_eq!(row["nextReminderTier"], "first");
    assert_eq!(row["reminderCount"].as_i64().unwrap_or(-1), 0);
}

fn disable_formal(app: &TestApp) {
    let mut value = serde_json::to_value(app.state.dynamic_config.read_reminders()).unwrap();
    value["formal_notice_enabled"] = json!(false);
    app.state
        .dynamic_config
        .update_section("reminders", value)
        .expect("disable formal notice");
}

async fn simulate_delivery(repo: &Repository, loan_id: i64) {
    repo.loans_update_reminder_sent(&[loan_id])
        .await
        .expect("mark reminded");
    sqlx::query(
        r#"
        UPDATE email_outbox o
        SET status = 'sent', sent_at = NOW()
        FROM email_outbox_reminder_loans rl
        WHERE rl.outbox_id = o.id AND rl.loan_id = $1 AND o.status = 'pending'
        "#,
    )
    .bind(loan_id)
    .execute(repo.pool())
    .await
    .expect("mark outbox sent");
    sqlx::query(
        r#"
        DELETE FROM email_outbox_reminder_loans rl
        USING email_outbox o
        WHERE rl.outbox_id = o.id AND rl.loan_id = $1
        "#,
    )
    .bind(loan_id)
    .execute(repo.pool())
    .await
    .expect("release reservation");
}

async fn reminder_count(repo: &Repository, loan_id: i64) -> i32 {
    sqlx::query_scalar("SELECT COALESCE(reminder_count, 0) FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(repo.pool())
        .await
        .expect("reminder_count")
}

fn loan_id_of(loan: &serde_json::Value) -> Option<i64> {
    loan["loanId"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| loan["loanId"].as_i64())
}

fn tier_for_email<'a>(body: &'a serde_json::Value, email: &str) -> Option<&'a str> {
    detail_for_email(body, email)?.get("tier")?.as_str()
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

async fn pending_to(repo: &Repository, email: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM email_outbox WHERE to_addr = $1 AND status = 'pending'",
    )
    .bind(email)
    .fetch_one(repo.pool())
    .await
    .expect("pending count")
}

async fn admin_token(app: &TestApp) -> String {
    let (_, health) = app.get_json("/api/v1/health").await;
    if health["setup"]["needFirstSetup"].as_bool() == Some(true) {
        return fixtures::ensure_first_setup(app).await;
    }
    fixtures::ensure_first_setup(app).await
}

/// Returns (loan_id, patron_email). `circulation_status` is written on the item.
async fn seed_overdue_loan(
    repo: &Repository,
    overdue_days: i64,
    reminder_count: i32,
    circulation_status: Option<i16>,
) -> (i64, String) {
    let suffix = Utc::now().timestamp_micros();
    let patron_email = format!("grad_{suffix}@test.local");
    let user_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            id, login, password, firstname, lastname, email, account_type,
            sex, birthdate, language, receive_reminders, token_version, created_at, update_at
        )
        VALUES ($1, $2, 'hash', 'Grad', 'Patron', $3, 'reader',
                'm', '1990-01-01', 'french', TRUE, 0, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(format!("grad_user_{suffix}"))
    .bind(&patron_email)
    .fetch_one(repo.pool())
    .await
    .expect("insert user");

    let loan_id = insert_loan_for_user(
        repo,
        user_id,
        overdue_days,
        reminder_count,
        circulation_status,
    )
    .await;
    (loan_id, patron_email)
}

async fn seed_second_loan_for_user(
    repo: &Repository,
    existing_loan_id: i64,
    overdue_days: i64,
    reminder_count: i32,
) -> i64 {
    let user_id: i64 = sqlx::query_scalar("SELECT user_id FROM loans WHERE id = $1")
        .bind(existing_loan_id)
        .fetch_one(repo.pool())
        .await
        .expect("user for loan");
    insert_loan_for_user(repo, user_id, overdue_days, reminder_count, None).await
}

async fn insert_loan_for_user(
    repo: &Repository,
    user_id: i64,
    overdue_days: i64,
    reminder_count: i32,
    circulation_status: Option<i16>,
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
    .bind(format!("Graduated title {suffix}"))
    .fetch_one(repo.pool())
    .await
    .expect("insert biblio");

    let item_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO items (id, biblio_id, barcode, borrowable, circulation_status)
        VALUES ($1, $2, $3, TRUE, $4)
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(biblio_id)
    .bind(format!("GRAD-{suffix}"))
    .bind(circulation_status)
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
