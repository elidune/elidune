//! Hold-ready notice shows the hold pickup site, not the copy's current site.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use chrono::Utc;
use common::TestApp;
use elidune_server::{
    models::hold::{Hold, HoldStatus},
    repository::users::HoldReadyUserContact,
    EmailService,
};
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static SNOWFLAKE_WORKER: AtomicU32 = AtomicU32::new(40);

fn snowflake() -> i64 {
    let worker = SNOWFLAKE_WORKER.fetch_add(1, Ordering::Relaxed) % 1022 + 1;
    snowflaked::Generator::new(worker as u16).generate::<i64>()
}

fn email_service(app: &TestApp) -> EmailService {
    let repo = app.state.services.repository.as_ref();
    EmailService::new(app.state.dynamic_config.clone(), repo.pool().clone())
}


#[tokio::test]
async fn email_shows_pickup_site_in_each_notice_language() {
    let _guard = TEST_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool().clone();
    let suffix = Utc::now().timestamp_micros();
    let pickup_name = format!("PickupAlpha{suffix}");
    let home_name = format!("HomeBeta{suffix}");

    let pickup_id: i64 = sqlx::query_scalar(r#"INSERT INTO sites (name, is_archive, "default") VALUES ($1, 0, FALSE) RETURNING id"#)
        .bind(&pickup_name)
        .fetch_one(&pool)
        .await
        .expect("pickup site");
    let home_id: i64 = sqlx::query_scalar(r#"INSERT INTO sites (name, is_archive, "default") VALUES ($1, 0, FALSE) RETURNING id"#)
        .bind(&home_name)
        .fetch_one(&pool)
        .await
        .expect("item site");

    let user_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            id, login, password, firstname, lastname, email, account_type,
            sex, birthdate, language, receive_reminders, token_version, created_at, update_at
        )
        VALUES ($1, $2, 'hash', 'Ada', 'Lovelace', $3, 'reader',
                'f', '1990-01-01', 'french', TRUE, 0, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(format!("hold_ready_{suffix}"))
    .bind(format!("hold-ready-{suffix}@test.local"))
    .fetch_one(&pool)
    .await
    .expect("user");

    let biblio_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO biblios (id, title, media_type, lang)
        VALUES ($1, 'Notice title', 'printedText', 'fre')
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .fetch_one(&pool)
    .await
    .expect("biblio");

    let item_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO items (id, biblio_id, barcode, borrowable, site_id)
        VALUES ($1, $2, $3, TRUE, $4)
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(biblio_id)
    .bind(format!("HR-{suffix}"))
    .bind(home_id)
    .fetch_one(&pool)
    .await
    .expect("item");

    let hold_id = snowflake();
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, item_id, biblio_id, position, status, pickup_site_id, expires_at)
        VALUES ($1, $2, $3, $4, 1, 'ready', $5, NOW() + INTERVAL '7 days')
        "#,
    )
    .bind(hold_id)
    .bind(user_id)
    .bind(item_id)
    .bind(biblio_id)
    .bind(pickup_id)
    .execute(&pool)
    .await
    .expect("hold");

    let hold = Hold {
        id: hold_id,
        user_id,
        biblio_id,
        item_id: Some(item_id),
        pickup_site_id: Some(pickup_id),
        created_at: Utc::now(),
        notified_at: None,
        expires_at: Some(Utc::now()),
        status: HoldStatus::Ready,
        position: 1,
        patron_note: None,
        internal_note: None,
        pick_started_at: None,
    };

    let email = email_service(&app);
    let phrases = [("french", "vous attend à"), ("english", "waiting for you at"), ("german", "wartet in"), ("spanish", "le espera en")];
    for (language, phrase) in phrases {
        let to = format!("{language}-{suffix}@test.local");
        elidune_server::hold_email::send_hold_ready_simple(
            &email,
            Some(HoldReadyUserContact {
                email: Some(to.clone()),
                firstname: Some("Ada".into()),
                lastname: Some("Lovelace".into()),
                language: Some(language.into()),
            }),
            &hold,
            "Notice title",
            Some("HR-1"),
            chrono_tz::Europe::Paris,
        )
        .await
        .unwrap_or_else(|err| panic!("enqueue {language}: {err}"));

        let body: String = sqlx::query_scalar("SELECT body FROM email_outbox WHERE to_addr = $1 ORDER BY id DESC LIMIT 1")
            .bind(&to)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|_| panic!("outbox row for {language}"));
        assert!(body.contains(&pickup_name), "{language} notice omitted the pickup site: {body}");
        assert!(!body.contains(&home_name), "{language} notice showed the item site: {body}");
        assert!(body.contains(phrase), "{language} notice was not rendered in that language: {body}");
    }
}


