//! Password-reset links are single-use and bound to a nonce.
//! The reset URL is built on the server; client `resetUrl` is ignored.

mod common;

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::error::AppError;
use elidune_server::models::secret::plaintext_password;
use elidune_server::AppConfig;
use serde_json::json;

const ACCEPTED: &str = "Password reset email sent";
const PUBLIC_BASE: &str = "https://library.example.org";

#[tokio::test]
async fn reset_link_is_single_use_and_a_newer_link_supersedes_the_older() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "resetonce").await;
    let user = app
        .state
        .services
        .users
        .get_by_id(reader_id)
        .await
        .expect("reader");
    let login = user.login.expect("login");

    let users = &app.state.services.users;
    let (_, older, _, _) = users
        .request_password_reset(&login)
        .await
        .expect("first reset link");
    let (_, newer, _, _) = users
        .request_password_reset(&login)
        .await
        .expect("second reset link");

    let stale = users
        .reset_password(&older, &plaintext_password("older-link-pass"))
        .await
        .expect_err("older link must be rejected once a newer one exists");
    assert!(
        matches!(stale, AppError::Authentication(_)),
        "stale link: {stale:?}"
    );

    users
        .reset_password(&newer, &plaintext_password("fresh-link-pass"))
        .await
        .expect("newer link resets the password");

    let reused = users
        .reset_password(&newer, &plaintext_password("second-use-pass"))
        .await
        .expect_err("a used link must be rejected");
    assert!(
        matches!(reused, AppError::Authentication(_)),
        "reused link: {reused:?}"
    );
}

fn config_for_reset(
    template: Option<&str>,
    public_base_url: Option<&str>,
    smtp_port: u16,
) -> AppConfig {
    let mut config = AppConfig::for_test();
    config.users.password_reset_url_template = template.map(str::to_string);
    config.server.public_base_url = public_base_url.map(str::to_string);
    config.email.smtp_host = "127.0.0.1".into();
    config.email.smtp_port = smtp_port;
    config.email.smtp_use_tls = false;
    config.email.smtp_username = None;
    config.email.smtp_password = None;
    config
}

/// Accepts TCP connections and never speaks SMTP, so an inline send blocks until the client timeout.
fn spawn_unresponsive_smtp() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("smtp bind");
    let port = listener.local_addr().expect("smtp addr").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(120));
                drop(stream);
            });
        }
    });
    port
}

async fn outbox_bodies(app: &TestApp, to: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT body FROM email_outbox WHERE to_addr = $1 ORDER BY created_at")
        .bind(to)
        .fetch_all(app.state.services.repository.pool())
        .await
        .expect("outbox bodies")
}

fn assert_queued_link(bodies: &[String], link_prefix: &str) {
    assert!(
        bodies.iter().any(|body| body.contains(link_prefix)),
        "queued reset mail missing {link_prefix}: {bodies:?}"
    );
}

async fn reset_nonce(app: &TestApp, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT password_reset_nonce FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("nonce")
}

async fn reader_login(app: &TestApp, admin_token: &str, prefix: &str) -> (i64, String, String) {
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

fn assert_accepted(status: StatusCode, body: &serde_json::Value) {
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["message"], ACCEPTED, "{body}");
    assert!(
        body.get("code").is_none(),
        "generic body has no error code: {body}"
    );
}

#[tokio::test]
async fn forged_reset_url_is_ignored_and_mail_uses_public_base_url() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(&format!("{PUBLIC_BASE}/")), 1),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetpub").await;

    let (status, body) = app
        .post_json_with_headers(
            "/api/v1/auth/request-password-reset",
            &json!({
                "identifier": login,
                "resetUrl": "https://evil.example/steal?token=<token>"
            }),
            None,
            &[
                ("host", "evil.example"),
                ("x-forwarded-host", "evil.example"),
            ],
        )
        .await;
    assert_accepted(status, &body);
    let queued = outbox_bodies(&app, &email).await;
    let expected = format!("{PUBLIC_BASE}/reset-password?token=");
    assert_queued_link(&queued, &expected);
    let raw = queued.join("\n");
    assert!(!raw.contains("evil.example"), "client host leaked: {raw}");
    assert!(!raw.contains("//reset-password"), "double slash: {raw}");
}

#[tokio::test]
async fn omitted_reset_url_is_not_a_validation_error() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), 1),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetomit").await;
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    assert_ne!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_accepted(status, &body);
    assert_queued_link(
        &outbox_bodies(&app, &email).await,
        &format!("{PUBLIC_BASE}/reset-password?token="),
    );
}

#[tokio::test]
async fn configured_template_is_used_instead_of_public_base_and_client_url() {
    let template = "https://staff.example/custom?token=<token>";
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(Some(template), Some(PUBLIC_BASE), 1),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetmpl").await;
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({
                "identifier": login,
                "resetUrl": "https://evil.example/steal?token=<token>"
            }),
            None,
        )
        .await;
    assert_accepted(status, &body);
    let queued = outbox_bodies(&app, &email).await;
    let raw = queued.join("\n");
    assert!(raw.contains("https://staff.example/custom?token="), "{raw}");
    assert!(!raw.contains("evil.example"), "{raw}");
    assert!(!raw.contains("/reset-password?token="), "{raw}");
}

#[tokio::test]
async fn missing_template_and_public_base_url_sends_nothing_and_stays_generic() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, None, 1),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (user_id, login, email) = reader_login(&app, &admin_token, "resetnone").await;
    let before = reset_nonce(&app, user_id).await;

    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({
                "identifier": login,
                "resetUrl": "https://evil.example/steal?token=<token>"
            }),
            None,
        )
        .await;
    assert_accepted(status, &body);
    assert_eq!(reset_nonce(&app, user_id).await, before, "no token issued");
    assert!(
        outbox_bodies(&app, &email).await.is_empty(),
        "no email when the link cannot be built"
    );

    let (unknown_status, unknown_body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": "nobody-reset@example.com" }),
            None,
        )
        .await;
    assert_accepted(unknown_status, &unknown_body);
    assert_eq!(unknown_body, body);
    assert!(outbox_bodies(&app, "nobody-reset@example.com")
        .await
        .is_empty());
}

fn reset_options(login_max: u32, reset_max: u32) -> elidune_server::AppBuildOptions {
    let mut options = elidune_server::AppBuildOptions::for_tests();
    options.auth_identifier_max_failures = Some(login_max);
    options.auth_identifier_window_secs = Some(900);
    options.auth_password_reset_max = Some(reset_max);
    options.auth_password_reset_window_secs = Some(3600);
    options
}

#[tokio::test]
async fn five_reset_requests_do_not_block_login() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), 1),
        reset_options(3, 10),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetlogin").await;

    for _ in 0..5 {
        let (status, body) = app
            .post_json(
                "/api/v1/auth/request-password-reset",
                &json!({ "identifier": login }),
                None,
            )
            .await;
        assert_accepted(status, &body);
    }
    let queued = outbox_bodies(&app, &email).await;
    assert_eq!(queued.len(), 5, "each request under the quota queues mail");
    assert_queued_link(&queued, &format!("{PUBLIC_BASE}/reset-password?token="));

    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("token").is_some(), "{body}");
}

#[tokio::test]
async fn fourth_reset_request_sends_no_mail_and_stays_generic() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), 1),
        reset_options(100, 3),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (user_id, login, email) = reader_login(&app, &admin_token, "resetcap").await;

    for _ in 0..3 {
        let (status, body) = app
            .post_json(
                "/api/v1/auth/request-password-reset",
                &json!({ "identifier": login }),
                None,
            )
            .await;
        assert_accepted(status, &body);
    }
    assert_eq!(outbox_bodies(&app, &email).await.len(), 3);
    let nonce = reset_nonce(&app, user_id).await;

    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    assert_accepted(status, &body);
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(reset_nonce(&app, user_id).await, nonce, "no token issued");
    assert_eq!(
        outbox_bodies(&app, &email).await.len(),
        3,
        "quota must not queue another email"
    );
}

#[tokio::test]
async fn unknown_account_gets_the_same_generic_success() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), 1),
        reset_options(100, 10),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetknown").await;

    let (known_status, known_body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    assert_accepted(known_status, &known_body);
    assert_eq!(outbox_bodies(&app, &email).await.len(), 1);

    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": "nobody-quota@example.com" }),
            None,
        )
        .await;
    assert_accepted(status, &body);
    assert_eq!(body, known_body);
    assert!(
        outbox_bodies(&app, "nobody-quota@example.com")
            .await
            .is_empty(),
        "unknown account queues no mail"
    );
}

/// Well under the SMTP client timeout, so an inline send to a silent relay cannot pass.
const RESET_ACCEPT_BUDGET: Duration = Duration::from_secs(2);

#[tokio::test]
async fn unreachable_smtp_accepts_immediately_and_queues_the_reset() {
    let port = spawn_unresponsive_smtp();
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), port),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login, email) = reader_login(&app, &admin_token, "resetsmtp").await;

    let started = Instant::now();
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    let elapsed = started.elapsed();
    assert_accepted(status, &body);
    assert!(
        elapsed < RESET_ACCEPT_BUDGET,
        "password reset blocked on SMTP for {elapsed:?}"
    );

    let queued = outbox_bodies(&app, &email).await;
    assert_eq!(
        queued.len(),
        1,
        "reset mail must be queued, not sent inline"
    );
    assert_queued_link(&queued, &format!("{PUBLIC_BASE}/reset-password?token="));
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM email_outbox WHERE to_addr = $1 AND status = 'pending'",
    )
    .bind(&email)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("pending count");
    assert_eq!(
        pending, 1,
        "queued reset stays pending until the outbox worker"
    );

    let started = Instant::now();
    let (unknown_status, unknown_body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": "nobody-smtp@example.com" }),
            None,
        )
        .await;
    assert!(
        started.elapsed() < RESET_ACCEPT_BUDGET,
        "unknown account must not wait on SMTP"
    );
    assert_accepted(unknown_status, &unknown_body);
    assert_eq!(
        unknown_body, body,
        "response must not reveal account existence"
    );
    assert!(outbox_bodies(&app, "nobody-smtp@example.com")
        .await
        .is_empty());
}

#[tokio::test]
async fn login_then_email_share_one_reset_quota_of_three() {
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), 1),
        reset_options(100, 3),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (user_id, login, email) = reader_login(&app, &admin_token, "resetshare").await;

    for identifier in [login.as_str(), email.as_str(), login.as_str()] {
        let (status, body) = app
            .post_json(
                "/api/v1/auth/request-password-reset",
                &json!({ "identifier": identifier }),
                None,
            )
            .await;
        assert_accepted(status, &body);
    }
    assert_eq!(
        outbox_bodies(&app, &email).await.len(),
        3,
        "login and email share one quota"
    );
    let nonce = reset_nonce(&app, user_id).await;

    let (status, blocked) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": email }),
            None,
        )
        .await;
    assert_accepted(status, &blocked);
    assert_ne!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(reset_nonce(&app, user_id).await, nonce, "no token issued");
    assert_eq!(
        outbox_bodies(&app, &email).await.len(),
        3,
        "shared quota must not queue another email"
    );

    let (unknown_status, unknown_body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": "nobody-share@example.com" }),
            None,
        )
        .await;
    assert_accepted(unknown_status, &unknown_body);
    assert_eq!(unknown_body, blocked, "quota must not reveal the account");
    assert!(outbox_bodies(&app, "nobody-share@example.com")
        .await
        .is_empty());

    let (_, other_login, other_email) = reader_login(&app, &admin_token, "resetother").await;
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": other_login }),
            None,
        )
        .await;
    assert_accepted(status, &body);
    assert_eq!(
        outbox_bodies(&app, &other_email).await.len(),
        1,
        "another account keeps its own quota"
    );
}
