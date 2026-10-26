//! Reader notices follow the reader's language, then the library language, then French.
//!
//! The whole message, including barcode and loan-list labels, stays in the language
//! of the template that was actually loaded.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use axum::http::StatusCode;
use chrono::Utc;
use common::{fixtures, TestApp};
use elidune_server::models::hold::{Hold, HoldStatus};
use elidune_server::repository::users::HoldReadyUserContact;
use elidune_server::AppConfig;
use serde_json::json;
use sqlx::Row;

static TEST_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

static SNOWFLAKE_WORKER: AtomicU32 = AtomicU32::new(80);

const FRENCH_RESET: &str = "Réinitialisez votre mot de passe Elidune";
const ENGLISH_RESET: &str = "Reset your Elidune password";
const GERMAN_RESET: &str = "Setzen Sie Ihr Elidune-Passwort zurück";

const FRENCH_REMINDER: &str = "Rappel : documents en retard";
const ENGLISH_REMINDER: &str = "Reminder: overdue library items";
const GERMAN_REMINDER: &str = "Erinnerung: überfällige Medien";

const FRENCH_HOLD: &str = "Votre réservation est disponible";
const ENGLISH_HOLD: &str = "Your library hold is ready for pickup";
const GERMAN_HOLD: &str = "Ihre Reservierung liegt zur Abholung bereit";
const SPANISH_HOLD: &str = "Su reserva está disponible";

#[test]
fn german_and_spanish_hold_ready_templates_include_pickup_site() {
    for language in ["german", "spanish"] {
        let path = format!(
            "{}/data/email_templates/hold_ready.{language}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {path}"));
        assert!(
            raw.contains("{{pickup_site}}"),
            "{language} hold_ready template must name the pickup site"
        );
    }
}

fn test_config() -> AppConfig {
    let mut config = AppConfig::for_test();
    config.server.public_base_url = Some("https://library.example.org".into());
    config
}

fn snowflake() -> i64 {
    let worker = SNOWFLAKE_WORKER.fetch_add(1, Ordering::Relaxed) % 1022 + 1;
    snowflaked::Generator::new(worker as u16).generate::<i64>()
}

/// Restores `library_info.language` even when the test panics.
struct RestoreLibraryLanguage {
    database_url: String,
    previous: Option<String>,
}

impl Drop for RestoreLibraryLanguage {
    fn drop(&mut self) {
        let database_url = std::mem::take(&mut self.database_url);
        let previous = self.previous.clone();
        if database_url.is_empty() {
            return;
        }
        let joined = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("restore runtime");
            runtime
                .block_on(async move {
                    let pool = sqlx::postgres::PgPoolOptions::new()
                        .max_connections(1)
                        .connect(&database_url)
                        .await?;
                    sqlx::query("UPDATE library_info SET language = $1 WHERE id = 1")
                        .bind(previous)
                        .execute(&pool)
                        .await?;
                    Ok::<(), sqlx::Error>(())
                })
                .expect("restore library language");
        });
        if let Err(err) = joined.join() {
            eprintln!("library language restore panicked: {err:?}");
        }
    }
}

async fn spawn() -> Option<(TestApp, String, RestoreLibraryLanguage)> {
    let app =
        TestApp::spawn_with_config(test_config(), elidune_server::AppBuildOptions::for_tests())
            .await?;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let previous = library_language(&app).await;
    let restore = RestoreLibraryLanguage {
        database_url: app.state.config.database.url.clone(),
        previous,
    };
    Some((app, admin_token, restore))
}

async fn library_language(app: &TestApp) -> Option<String> {
    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["language"].as_str().map(str::to_string)
}

async fn set_library_language(app: &TestApp, token: &str, language: Option<&str>) {
    let payload = match language {
        Some(code) => json!({ "language": code }),
        None => json!({ "language": null }),
    };
    let (status, body) = app
        .put_json("/api/v1/library-info", &payload, Some(token))
        .await;
    assert_eq!(status, StatusCode::OK, "set library language: {body}");
    match language {
        Some(code) => assert_eq!(body["language"], code, "{body}"),
        None => assert!(body["language"].is_null(), "{body}"),
    }
}

async fn set_user_language(app: &TestApp, user_id: i64, language: Option<&str>) {
    sqlx::query("UPDATE users SET language = $1 WHERE id = $2")
        .bind(language)
        .bind(user_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set reader language");
}

async fn reader(app: &TestApp, admin_token: &str, prefix: &str) -> (i64, String, String) {
    let (user_id, _) = fixtures::create_reader(app, admin_token, prefix).await;
    let user = app
        .state
        .services
        .users
        .get_by_id(user_id)
        .await
        .expect("reader");
    (
        user_id,
        user.login.expect("login"),
        user.email.expect("email"),
    )
}

async fn latest_mail(app: &TestApp, to: &str) -> (String, String) {
    let row = sqlx::query(
        "SELECT subject, body FROM email_outbox WHERE to_addr = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(to)
    .fetch_one(app.state.services.repository.pool())
    .await
    .unwrap_or_else(|_| panic!("no outbox row for {to}"));
    (row.get("subject"), row.get("body"))
}

fn assert_notice(subject: &str, body: &str, present: &[&str], absent: &[&str]) {
    let combined = format!("{subject}\n{body}");
    for needle in present {
        assert!(combined.contains(needle), "missing {needle} in {combined}");
    }
    for needle in absent {
        assert!(
            !combined.contains(needle),
            "unexpected {needle} in {combined}"
        );
    }
}

async fn request_reset(app: &TestApp, login: &str) {
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["message"], "Password reset email sent", "{body}");
}

#[tokio::test]
async fn password_reset_follows_reader_then_library_then_french() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    set_library_language(&app, &admin_token, Some("de")).await;

    let (english_id, english_login, english_email) = reader(&app, &admin_token, "noticen").await;
    set_user_language(&app, english_id, Some("english")).await;
    request_reset(&app, &english_login).await;
    let (subject, body) = latest_mail(&app, &english_email).await;
    assert_notice(
        &subject,
        &body,
        &[ENGLISH_RESET],
        &[GERMAN_RESET, FRENCH_RESET],
    );

    let (german_library_id, german_library_login, german_library_email) =
        reader(&app, &admin_token, "noticede").await;
    set_user_language(&app, german_library_id, None).await;
    request_reset(&app, &german_library_login).await;
    let (subject, body) = latest_mail(&app, &german_library_email).await;
    assert_notice(
        &subject,
        &body,
        &[GERMAN_RESET],
        &[ENGLISH_RESET, FRENCH_RESET],
    );

    set_library_language(&app, &admin_token, None).await;
    let (french_id, french_login, french_email) = reader(&app, &admin_token, "noticefr").await;
    set_user_language(&app, french_id, None).await;
    request_reset(&app, &french_login).await;
    let (subject, body) = latest_mail(&app, &french_email).await;
    assert_notice(
        &subject,
        &body,
        &[FRENCH_RESET],
        &[ENGLISH_RESET, GERMAN_RESET],
    );
}

#[tokio::test]
async fn create_without_language_stores_null_and_email_uses_library_language() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    let column_default: Option<String> = sqlx::query_scalar(
        r#"
        SELECT column_default
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'users'
          AND column_name = 'language'
        "#,
    )
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("users.language default");
    assert!(
        column_default.is_none(),
        "users.language must have no default, got {column_default:?}"
    );

    set_library_language(&app, &admin_token, Some("de")).await;
    let (status, body, login) = post_reader(&app, &admin_token, "nolang", None).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["language"].is_null(), "{body}");

    let user_id = fixtures::json_id(&body["id"]);
    let stored: Option<String> = sqlx::query_scalar("SELECT language FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("stored language");
    assert!(
        stored.is_none(),
        "omitted language must be NULL, got {stored:?}"
    );

    request_reset(&app, &login).await;
    let email = body["email"].as_str().expect("email");
    let (subject, mail) = latest_mail(&app, email).await;
    assert_notice(
        &subject,
        &mail,
        &[GERMAN_RESET],
        &[ENGLISH_RESET, FRENCH_RESET],
    );
}

#[tokio::test]
async fn create_with_german_stores_german() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    let (status, body, _) = post_reader(&app, &admin_token, "delang", Some("german")).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["language"], "german", "{body}");

    let user_id = fixtures::json_id(&body["id"]);
    let stored: Option<String> = sqlx::query_scalar("SELECT language FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("stored language");
    assert_eq!(stored.as_deref(), Some("german"));
}

#[tokio::test]
async fn create_with_invalid_language_is_422() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    let (status, body, login) = post_reader(&app, &admin_token, "badlang", Some("italian")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body["code"].is_string(), "{body}");
    assert!(body["error"].is_string(), "{body}");
    assert!(body["message"].is_string(), "{body}");

    let stored: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE login = $1")
        .bind(&login)
        .fetch_optional(app.state.services.repository.pool())
        .await
        .expect("lookup rejected create");
    assert!(stored.is_none(), "invalid language must not insert a user");
}

async fn post_reader(
    app: &TestApp,
    admin_token: &str,
    prefix: &str,
    language: Option<&str>,
) -> (StatusCode, serde_json::Value, String) {
    let public_type = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let login = format!("{prefix}_{}", fixtures::unique_suffix());
    let mut payload = json!({
        "login": login,
        "password": "readerpass1234",
        "firstname": "Reader",
        "lastname": "Notice",
        "email": format!("{login}@test.local"),
        "accountType": "reader",
        "publicType": public_type.to_string(),
        "sex": "f",
        "birthdate": "1990-06-01",
        "addrCity": "Paris"
    });
    if let Some(language) = language {
        payload["language"] = json!(language);
    }
    let (status, body) = app
        .post_json("/api/v1/users", &payload, Some(admin_token))
        .await;
    (status, body, login)
}

#[tokio::test]
async fn overdue_reminder_follows_reader_then_library_then_french() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    set_library_language(&app, &admin_token, Some("de")).await;

    let english_email = seed_overdue(&app, Some("english")).await;
    send_reminders(&app, &admin_token).await;
    let (subject, body) = latest_mail(&app, &english_email).await;
    assert_notice(
        &subject,
        &body,
        &[ENGLISH_REMINDER, "Due date"],
        &[
            GERMAN_REMINDER,
            "Fälligkeitsdatum",
            FRENCH_REMINDER,
            "Date de retour",
        ],
    );

    let german_email = seed_overdue(&app, None).await;
    send_reminders(&app, &admin_token).await;
    let (subject, body) = latest_mail(&app, &german_email).await;
    assert_notice(
        &subject,
        &body,
        &[GERMAN_REMINDER, "Fälligkeitsdatum", "ausgeliehen"],
        &[
            ENGLISH_REMINDER,
            "Due date",
            FRENCH_REMINDER,
            "Date de retour",
            "emprunté le",
        ],
    );

    set_library_language(&app, &admin_token, None).await;
    let french_email = seed_overdue(&app, None).await;
    send_reminders(&app, &admin_token).await;
    let (subject, body) = latest_mail(&app, &french_email).await;
    assert_notice(
        &subject,
        &body,
        &[FRENCH_REMINDER, "Date de retour", "emprunté le"],
        &[
            ENGLISH_REMINDER,
            GERMAN_REMINDER,
            "Fälligkeitsdatum",
            "Due date",
        ],
    );
}

#[tokio::test]
async fn hold_ready_stays_in_one_language() {
    let _lock = TEST_LOCK.lock().await;
    let Some((app, admin_token, _restore)) = spawn().await else {
        return;
    };

    set_library_language(&app, &admin_token, Some("en")).await;
    let italian = unique_email("holdit");
    queue_hold(&app, &italian, Some("italian")).await;
    let (subject, body) = latest_mail(&app, &italian).await;
    assert_notice(
        &subject,
        &body,
        &[ENGLISH_HOLD, "Barcode: ABC-1", "(unknown title)"],
        &[
            FRENCH_HOLD,
            GERMAN_HOLD,
            SPANISH_HOLD,
            "Code-barres",
            "Código de barras",
            "(titre inconnu)",
            "(unbekannter Titel)",
            "(título desconocido)",
        ],
    );

    set_library_language(&app, &admin_token, Some("de")).await;
    let preferred = unique_email("holden");
    queue_hold(&app, &preferred, Some("english")).await;
    let (subject, body) = latest_mail(&app, &preferred).await;
    assert_notice(
        &subject,
        &body,
        &[ENGLISH_HOLD, "(unknown title)"],
        &[
            GERMAN_HOLD,
            FRENCH_HOLD,
            "(unbekannter Titel)",
            "(titre inconnu)",
        ],
    );

    let library_german = unique_email("holdde");
    queue_hold(&app, &library_german, None).await;
    let (subject, body) = latest_mail(&app, &library_german).await;
    assert_notice(
        &subject,
        &body,
        &[GERMAN_HOLD, "(unbekannter Titel)", "Barcode: ABC-1"],
        &[
            ENGLISH_HOLD,
            FRENCH_HOLD,
            SPANISH_HOLD,
            "Code-barres",
            "Código de barras",
            "(unknown title)",
            "(titre inconnu)",
        ],
    );

    set_library_language(&app, &admin_token, None).await;
    let spanish = unique_email("holdes");
    queue_hold(&app, &spanish, Some("spanish")).await;
    let (subject, body) = latest_mail(&app, &spanish).await;
    assert_notice(
        &subject,
        &body,
        &[
            SPANISH_HOLD,
            "Código de barras: ABC-1",
            "(título desconocido)",
        ],
        &[
            FRENCH_HOLD,
            ENGLISH_HOLD,
            GERMAN_HOLD,
            "Code-barres",
            "(titre inconnu)",
            "(unknown title)",
            "(unbekannter Titel)",
        ],
    );
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}_{}@test.local", fixtures::unique_suffix())
}

async fn send_reminders(app: &TestApp, admin_token: &str) {
    let (status, body) = app
        .post_empty("/api/v1/loans/send-overdue-reminders", Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "send reminders: {body}");
}

async fn seed_overdue(app: &TestApp, language: Option<&str>) -> String {
    let suffix = fixtures::unique_suffix();
    let email = format!("notice_{suffix}@test.local");
    let pool = app.state.services.repository.pool();
    let user_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            id, login, password, firstname, lastname, email, account_type,
            sex, birthdate, language, receive_reminders, token_version, created_at, update_at
        )
        VALUES ($1, $2, 'hash', 'Notice', 'Patron', $3, 'reader',
                'm', '1990-01-01', $4, TRUE, 0, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(format!("notice_user_{suffix}"))
    .bind(&email)
    .bind(language)
    .fetch_one(pool)
    .await
    .expect("insert reader");

    let biblio_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO biblios (id, title, media_type, lang)
        VALUES ($1, $2, 'printedText', 'fre')
        RETURNING id
        "#,
    )
    .bind(snowflake())
    .bind(format!("Notice title {suffix}"))
    .fetch_one(pool)
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
    .bind(format!("NOTICE-{suffix}"))
    .fetch_one(pool)
    .await
    .expect("insert item");

    sqlx::query(
        r#"
        INSERT INTO loans (id, user_id, item_id, date, expiry_at, reminder_count)
        VALUES (
            $1, $2, $3,
            NOW() - INTERVAL '54 days',
            NOW() - INTERVAL '40 days',
            0
        )
        "#,
    )
    .bind(snowflake())
    .bind(user_id)
    .bind(item_id)
    .execute(pool)
    .await
    .expect("insert overdue loan");

    email
}

async fn queue_hold(app: &TestApp, email: &str, language: Option<&str>) {
    let now = Utc::now();
    let hold = Hold {
        id: snowflake(),
        user_id: snowflake(),
        biblio_id: snowflake(),
        item_id: None,
        pickup_site_id: None,
        created_at: now,
        notified_at: None,
        expires_at: Some(now),
        status: HoldStatus::Ready,
        position: 1,
        patron_note: None,
        internal_note: None,
        pick_started_at: None,
    };
    let contact = HoldReadyUserContact {
        email: Some(email.to_string()),
        firstname: Some("Ada".into()),
        lastname: Some("Lovelace".into()),
        language: language.map(str::to_string),
    };
    elidune_server::hold_email::send_hold_ready_simple(
        &app.state.services.email,
        Some(contact),
        &hold,
        "(unknown title)",
        Some("ABC-1"),
        elidune_server::services::calendar::default_zone(),
    )
    .await
    .expect("queue hold ready");
}
