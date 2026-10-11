//! Password-reset links are single-use and bound to a nonce.
//! The reset URL is built on the server; client `resetUrl` is ignored.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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

fn spawn_smtp() -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("smtp bind");
    let port = listener.local_addr().expect("smtp addr").port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let tx = tx.clone();
            thread::spawn(move || {
                let _ = capture_smtp_message(stream, &tx);
            });
        }
    });
    (port, rx)
}

fn capture_smtp_message(
    stream: std::net::TcpStream,
    tx: &mpsc::Sender<String>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut write = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    write.write_all(b"220 localhost ESMTP\r\n")?;
    write.flush()?;
    let mut line = String::new();
    let mut data = false;
    let mut body = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if data {
            if line.trim_end_matches(['\r', '\n']) == "." {
                data = false;
                let _ = tx.send(body.clone());
                body.clear();
                write.write_all(b"250 OK\r\n")?;
                write.flush()?;
            } else {
                body.push_str(&line);
            }
            continue;
        }
        let cmd = line.to_ascii_uppercase();
        if cmd.starts_with("EHLO") || cmd.starts_with("HELO") {
            write.write_all(b"250-localhost\r\n250 OK\r\n")?;
        } else if cmd.starts_with("DATA") {
            data = true;
            write.write_all(b"354 Go ahead\r\n")?;
        } else if cmd.starts_with("QUIT") {
            write.write_all(b"221 Bye\r\n")?;
            write.flush()?;
            break;
        } else {
            write.write_all(b"250 OK\r\n")?;
        }
        write.flush()?;
    }
    Ok(())
}

async fn reset_nonce(app: &TestApp, user_id: i64) -> i64 {
    sqlx::query_scalar("SELECT password_reset_nonce FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("nonce")
}

async fn reader_login(app: &TestApp, admin_token: &str, prefix: &str) -> (i64, String) {
    let (user_id, _) = fixtures::create_reader(app, admin_token, prefix).await;
    let user = app
        .state
        .services
        .users
        .get_by_id(user_id)
        .await
        .expect("reader");
    (user_id, user.login.expect("login"))
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
    let (port, mail) = spawn_smtp();
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(&format!("{PUBLIC_BASE}/")), port),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login) = reader_login(&app, &admin_token, "resetpub").await;

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
    let raw = mail
        .recv_timeout(Duration::from_secs(5))
        .expect("smtp data");
    let expected = format!("{PUBLIC_BASE}/reset-password?token=");
    assert!(raw.contains(&expected), "mail body: {raw}");
    assert!(!raw.contains("evil.example"), "client host leaked: {raw}");
    assert!(!raw.contains("//reset-password"), "double slash: {raw}");
}

#[tokio::test]
async fn omitted_reset_url_is_not_a_validation_error() {
    let (port, mail) = spawn_smtp();
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, Some(PUBLIC_BASE), port),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login) = reader_login(&app, &admin_token, "resetomit").await;
    let (status, body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": login }),
            None,
        )
        .await;
    assert_ne!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_accepted(status, &body);
    let raw = mail
        .recv_timeout(Duration::from_secs(5))
        .expect("smtp data");
    assert!(raw.contains(&format!("{PUBLIC_BASE}/reset-password?token=")));
}

#[tokio::test]
async fn configured_template_is_used_instead_of_public_base_and_client_url() {
    let (port, mail) = spawn_smtp();
    let template = "https://staff.example/custom?token=<token>";
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(Some(template), Some(PUBLIC_BASE), port),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, login) = reader_login(&app, &admin_token, "resetmpl").await;
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
    let raw = mail
        .recv_timeout(Duration::from_secs(5))
        .expect("smtp data");
    assert!(raw.contains("https://staff.example/custom?token="), "{raw}");
    assert!(!raw.contains("evil.example"), "{raw}");
    assert!(!raw.contains("/reset-password?token="), "{raw}");
}

#[tokio::test]
async fn missing_template_and_public_base_url_sends_nothing_and_stays_generic() {
    let (port, mail) = spawn_smtp();
    let Some(app) = TestApp::spawn_with_config(
        config_for_reset(None, None, port),
        elidune_server::AppBuildOptions::for_tests(),
    )
    .await
    else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (user_id, login) = reader_login(&app, &admin_token, "resetnone").await;
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

    let (unknown_status, unknown_body) = app
        .post_json(
            "/api/v1/auth/request-password-reset",
            &json!({ "identifier": "nobody-reset@example.com" }),
            None,
        )
        .await;
    assert_accepted(unknown_status, &unknown_body);
    assert!(
        mail.recv_timeout(Duration::from_millis(400)).is_err(),
        "no email when the link cannot be built"
    );
}
