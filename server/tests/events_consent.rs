//! Event-announcement consent: migration backfill, recipient filter, unsubscribe, notice.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::fixtures;
use common::TestApp;
use http_body_util::BodyExt;
use serde_json::json;

#[test]
fn backfill_sql_opts_in_active_non_children_without_using_reminders() {
    let sql = include_str!("../migrations/037_events_consent.sql");
    let lower = sql
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    assert!(
        !lower.contains("receive_reminders"),
        "backfill must not copy the overdue-reminder flag"
    );
    assert!(sql.contains("events_consent_source = 'migration'"));
    assert!(sql.contains("u.status <> 'deleted'"));
    assert!(
        sql.contains("pt.name = 'child'"),
        "a child is public_types.name = 'child', the same test #81 uses"
    );
    assert!(
        !lower.contains("birthdate"),
        "age is not how this codebase identifies a child account"
    );
}

#[tokio::test]
async fn backfill_sets_consent_for_active_non_children_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let child_type = fixtures::public_type_id_by_name(&app, &token, "child").await;
    let adult_type = fixtures::public_type_id_by_name(&app, &token, "adult").await;
    let school_type = fixtures::public_type_id_by_name(&app, &token, "school").await;
    let suffix = fixtures::unique_suffix();

    let adult_no_reminders = insert_patron(
        &app,
        &format!("bf_adult_{suffix}"),
        "active",
        Some(adult_type),
        false,
    )
    .await;
    let child = insert_patron(
        &app,
        &format!("bf_child_{suffix}"),
        "active",
        Some(child_type),
        true,
    )
    .await;
    let deleted = insert_patron(
        &app,
        &format!("bf_deleted_{suffix}"),
        "deleted",
        Some(adult_type),
        true,
    )
    .await;
    let blocked = insert_patron(
        &app,
        &format!("bf_blocked_{suffix}"),
        "blocked",
        Some(adult_type),
        true,
    )
    .await;
    let school = insert_patron(
        &app,
        &format!("bf_school_{suffix}"),
        "active",
        Some(school_type),
        false,
    )
    .await;
    let no_type = insert_patron(&app, &format!("bf_notype_{suffix}"), "active", None, false).await;

    let ids = vec![adult_no_reminders, child, deleted, blocked, school, no_type];
    sqlx::query(
        "UPDATE users SET events_consent_at = NULL, events_consent_source = NULL, events_consent_changed_at = NULL, events_consent_notice_at = NULL WHERE id = ANY($1::bigint[])",
    )
    .bind(&ids)
    .execute(app.state.services.repository.pool())
    .await
    .expect("clear consent");

    sqlx::query(&scoped_backfill_sql())
        .bind(&ids)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backfill");

    assert_eq!(
        consent_source(&app, adult_no_reminders).await,
        Some("migration".into())
    );
    assert!(consent_at_present(&app, adult_no_reminders).await);
    assert_eq!(
        consent_source(&app, blocked).await,
        Some("migration".into())
    );
    assert_eq!(consent_source(&app, school).await, Some("migration".into()));
    assert_eq!(
        consent_source(&app, no_type).await,
        Some("migration".into())
    );
    assert!(consent_source(&app, child).await.is_none());
    assert!(!consent_at_present(&app, child).await);
    assert!(consent_source(&app, deleted).await.is_none());
    assert!(!consent_at_present(&app, deleted).await);
}

#[tokio::test]
async fn announcement_uses_events_consent_including_the_guardian() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let audience = create_public_type(&app, &token, &format!("aud{suffix}")).await;
    let adult_type = fixtures::public_type_id_by_name(&app, &token, "adult").await;

    let (yes_id, yes_email) =
        create_patron(&app, &token, &format!("yes_{suffix}"), audience, Some(true)).await;
    let (_no_id, no_email) =
        create_patron(&app, &token, &format!("no_{suffix}"), audience, Some(false)).await;
    sqlx::query("UPDATE users SET receive_reminders = TRUE WHERE id = $1")
        .bind(_no_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("keep reminders on");

    let (guardian_id, guardian_email) = create_patron(
        &app,
        &token,
        &format!("guard_{suffix}"),
        adult_type,
        Some(true),
    )
    .await;
    let (_silent_id, silent_email) =
        create_patron(&app, &token, &format!("silent_{suffix}"), adult_type, None).await;
    sqlx::query("UPDATE users SET receive_reminders = TRUE WHERE id = $1")
        .bind(_silent_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("reminders without event consent");
    let (_child_id, child_email) =
        create_child(&app, &token, guardian_id, &format!("kid_{suffix}"), true).await;
    let (other_guard, other_email) = create_patron(
        &app,
        &token,
        &format!("other_{suffix}"),
        adult_type,
        Some(true),
    )
    .await;
    let (_other_child, other_child_email) = create_child(
        &app,
        &token,
        other_guard,
        &format!("otherkid_{suffix}"),
        true,
    )
    .await;
    sqlx::query(
        "UPDATE users SET events_consent_at = NULL, events_consent_source = 'desk', events_consent_changed_at = NOW(), receive_reminders = TRUE WHERE id = $1",
    )
    .bind(other_guard)
    .execute(app.state.services.repository.pool())
    .await
    .expect("withdraw guardian consent");

    let audience_name = format!("aud{suffix}");
    let event_id = create_event(
        &app,
        &token,
        &format!("Count {suffix}"),
        json!({ "publicTypes": [audience_name] }),
    )
    .await;
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/events/{event_id}/announcement-recipients/count"),
            &token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "count: {body}");
    assert_eq!(
        body["count"], 1,
        "only the consented patron of this audience"
    );

    let (status, sent) = send_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {sent}");
    assert_eq!(outbox_count(&app, event_id, &yes_email).await, 1);
    assert_eq!(outbox_count(&app, event_id, &no_email).await, 0);
    let _ = yes_id;

    let child_event = create_event(
        &app,
        &token,
        &format!("Child {suffix}"),
        json!({ "publicTypes": ["child"] }),
    )
    .await;
    let (status, sent) = send_announcement(&app, &token, child_event).await;
    assert_eq!(status, StatusCode::OK, "child send: {sent}");
    assert_eq!(outbox_count(&app, child_event, &guardian_email).await, 1);
    assert_eq!(outbox_count(&app, child_event, &child_email).await, 0);
    assert_eq!(outbox_count(&app, child_event, &silent_email).await, 0);
    assert_eq!(outbox_count(&app, child_event, &other_email).await, 0);
    assert_eq!(outbox_count(&app, child_event, &other_child_email).await, 0);
}

#[tokio::test]
async fn unsubscribe_post_json_and_one_click_are_idempotent() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let audience = create_public_type(&app, &token, &format!("unsub{suffix}")).await;
    let (user_id, email) = create_patron(
        &app,
        &token,
        &format!("unsub_{suffix}"),
        audience,
        Some(true),
    )
    .await;
    let (other_id, other_email) = create_patron(
        &app,
        &token,
        &format!("unsub2_{suffix}"),
        audience,
        Some(true),
    )
    .await;
    let event_id = create_event(
        &app,
        &token,
        &format!("Unsub {suffix}"),
        json!({ "publicTypes": [format!("unsub{suffix}")] }),
    )
    .await;
    let (status, sent) = send_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {sent}");

    let raw = outbox_body(&app, event_id, &email).await;
    let mail: serde_json::Value = serde_json::from_str(&raw).expect("outbox json");
    let plain = mail["plain"].as_str().expect("plain");
    assert!(
        plain.contains("/events/unsubscribe?token="),
        "email link is the UI page: {plain}"
    );
    assert!(
        !plain.contains("/api/v1/events/unsubscribe"),
        "the visible link is not the API endpoint: {plain}"
    );
    let headers = mail["headers"].as_array().expect("headers");
    let list_unsubscribe = headers
        .iter()
        .find(|header| header["name"] == "List-Unsubscribe")
        .expect("List-Unsubscribe");
    let header_value = list_unsubscribe["value"].as_str().expect("header value");
    assert!(
        header_value.contains("/api/v1/events/unsubscribe?token="),
        "one-click header targets the API: {header_value}"
    );
    assert!(headers.iter().any(|header| {
        header["name"] == "List-Unsubscribe-Post" && header["value"] == "List-Unsubscribe=One-Click"
    }));

    let ui_token = token_after(plain, "/events/unsubscribe?token=");
    let (status, response_body) = app
        .post_json(
            "/api/v1/events/unsubscribe",
            &json!({ "token": ui_token }),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "json unsubscribe: {response_body}"
    );
    assert!(!consent_at_present(&app, user_id).await);
    assert_eq!(
        consent_source(&app, user_id).await.as_deref(),
        Some("unsubscribe")
    );
    let changed = changed_at(&app, user_id).await;

    let (status, response_body) = app
        .post_json(
            "/api/v1/events/unsubscribe",
            &json!({ "token": ui_token }),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "second unsubscribe: {response_body}"
    );
    assert_eq!(changed_at(&app, user_id).await, changed);

    let (status, response_body) = post_raw(
        &app,
        "/api/v1/events/unsubscribe",
        "application/json",
        r#"{"token":"tampered.token"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response_body, "Invalid request.\n");
    assert_eq!(
        consent_source(&app, other_id).await.as_deref(),
        Some("desk")
    );

    let other_raw = outbox_body(&app, event_id, &other_email).await;
    let other_mail: serde_json::Value = serde_json::from_str(&other_raw).expect("other json");
    let other_header = other_mail["headers"]
        .as_array()
        .expect("headers")
        .iter()
        .find(|header| header["name"] == "List-Unsubscribe")
        .expect("header");
    let api_token = token_after(
        other_header["value"].as_str().expect("value"),
        "/api/v1/events/unsubscribe?token=",
    );
    let api_token = api_token.trim_end_matches('>');
    let (status, response_body) = post_raw(
        &app,
        &format!("/api/v1/events/unsubscribe?token={api_token}"),
        "application/x-www-form-urlencoded",
        "List-Unsubscribe=One-Click",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "one-click: {response_body}");
    assert!(!consent_at_present(&app, other_id).await);
    assert_eq!(
        consent_source(&app, other_id).await.as_deref(),
        Some("unsubscribe")
    );

    let (status, _) = post_raw(
        &app,
        &format!("/api/v1/events/unsubscribe?token={api_token}"),
        "application/x-www-form-urlencoded",
        "not-the-one-click-body",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn migration_notice_is_included_once() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let audience = create_public_type(&app, &token, &format!("note{suffix}")).await;
    let (user_id, email) = create_patron(
        &app,
        &token,
        &format!("note_{suffix}"),
        audience,
        Some(true),
    )
    .await;
    sqlx::query(
        "UPDATE users SET language = 'english', events_consent_source = 'migration', events_consent_notice_at = NULL WHERE id = $1",
    )
    .bind(user_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("mark migrated");

    let name = format!("note{suffix}");
    let first = create_event(
        &app,
        &token,
        &format!("Notice {suffix}"),
        json!({ "publicTypes": [name] }),
    )
    .await;
    let (status, sent) = send_announcement(&app, &token, first).await;
    assert_eq!(status, StatusCode::OK, "first send: {sent}");
    let raw = outbox_body(&app, first, &email).await;
    let mail: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let plain = mail["plain"].as_str().expect("plain");
    assert!(
        plain.contains("existing library account was opted in"),
        "migration notice: {plain}"
    );
    assert!(plain.contains("Unsubscribe from event announcements: "));
    assert!(!plain.contains(" : "));
    let notice_at = notice_timestamp(&app, user_id)
        .await
        .expect("notice stamped");

    let second = create_event(
        &app,
        &token,
        &format!("Notice again {suffix}"),
        json!({ "publicTypes": [name] }),
    )
    .await;
    let (status, sent) = send_announcement(&app, &token, second).await;
    assert_eq!(status, StatusCode::OK, "second send: {sent}");
    let raw = outbox_body(&app, second, &email).await;
    let mail: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let plain = mail["plain"].as_str().expect("plain");
    assert!(
        !plain.contains("existing library account was opted in"),
        "notice is once: {plain}"
    );
    assert!(plain.contains("/events/unsubscribe?token="));
    assert_eq!(notice_timestamp(&app, user_id).await, Some(notice_at));
}

#[tokio::test]
async fn desk_and_profile_stamp_source_and_child_consent_is_ignored() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let adult_type = fixtures::public_type_id_by_name(&app, &token, "adult").await;
    let (user_id, _email) = create_patron(
        &app,
        &token,
        &format!("desk_{suffix}"),
        adult_type,
        Some(true),
    )
    .await;
    let (status, created) = app
        .get_json_with_auth(&format!("/api/v1/users/{user_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "read: {created}");
    assert_eq!(created["eventsConsent"], true);
    assert_eq!(created["eventsConsentSource"], "desk");
    assert!(created["eventsConsentAt"].is_string());

    sqlx::query("UPDATE users SET events_consent_at = '2020-01-15T00:00:00Z' WHERE id = $1")
        .bind(user_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("pin consent date");
    let (status, updated) = app
        .put_json(
            &format!("/api/v1/users/{user_id}"),
            &json!({ "eventsConsent": true }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "reaffirm: {updated}");
    assert_eq!(updated["eventsConsentAt"], "2020-01-15T00:00:00Z");
    assert_eq!(updated["eventsConsentSource"], "desk");

    let (status, withdrawn) = app
        .put_json(
            &format!("/api/v1/users/{user_id}"),
            &json!({ "eventsConsent": false }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "withdraw: {withdrawn}");
    assert_eq!(withdrawn["eventsConsent"], false);
    assert!(withdrawn["eventsConsentAt"].is_null());
    assert_eq!(withdrawn["eventsConsentSource"], "desk");

    let (reader_id, reader_token) =
        fixtures::create_reader_with_public_type(&app, &token, "profile", Some(adult_type)).await;
    let (status, profile) = app
        .put_json(
            "/api/v1/auth/profile",
            &json!({ "eventsConsent": true }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "profile: {profile}");
    assert_eq!(profile["eventsConsent"], true);
    assert_eq!(profile["eventsConsentSource"], "profile");
    let _ = reader_id;

    let (guardian_id, _) =
        create_patron(&app, &token, &format!("parent_{suffix}"), adult_type, None).await;
    let (child_id, _) =
        create_child(&app, &token, guardian_id, &format!("child_{suffix}"), true).await;
    let (status, child) = app
        .get_json_with_auth(&format!("/api/v1/users/{child_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "child: {child}");
    assert_eq!(child["eventsConsent"], false);
    assert!(child["eventsConsentAt"].is_null());
    assert!(child["eventsConsentSource"].is_null());
    assert!(!consent_at_present(&app, child_id).await);
}

fn scoped_backfill_sql() -> String {
    let migration = include_str!("../migrations/037_events_consent.sql");
    let start = migration
        .find("-- BEGIN EVENTS CONSENT BACKFILL")
        .expect("start marker");
    let end = migration
        .find("-- END EVENTS CONSENT BACKFILL")
        .expect("end marker");
    let statement = migration[start..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    let statement = statement.trim().trim_end_matches(';').trim();
    format!("{statement}\n  AND u.id = ANY($1::bigint[])")
}

async fn insert_patron(
    app: &TestApp,
    login: &str,
    status: &str,
    public_type: Option<i64>,
    receive_reminders: bool,
) -> i64 {
    sqlx::query_scalar(
        r#"
        INSERT INTO users (
            login, firstname, lastname, email, account_type, status, public_type,
            receive_reminders, sex, birthdate, token_version, created_at, update_at
        )
        VALUES ($1, 'Back', 'Fill', $2, 'reader', $3, $4, $5, 'm', '1990-01-01', 0, NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(login)
    .bind(format!("{login}@test.local"))
    .bind(status)
    .bind(public_type)
    .bind(receive_reminders)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("insert patron")
}

async fn create_public_type(app: &TestApp, token: &str, name: &str) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({ "name": name, "label": name }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {body}");
    fixtures::json_id(&body["id"])
}

async fn create_patron(
    app: &TestApp,
    token: &str,
    login: &str,
    public_type: i64,
    events_consent: Option<bool>,
) -> (i64, String) {
    let login = format!("{login}_{}", fixtures::unique_suffix());
    let email = format!("{login}@test.local");
    let mut payload = json!({
        "login": login,
        "password": "readerpass1234",
        "firstname": "Patron",
        "lastname": login,
        "email": email,
        "accountType": "reader",
        "publicType": public_type.to_string(),
        "sex": "f",
        "birthdate": "1991-02-02",
        "addrCity": "Lyon"
    });
    if let Some(consent) = events_consent {
        payload["eventsConsent"] = json!(consent);
    }
    let (status, body) = app.post_json("/api/v1/users", &payload, Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "create patron: {body}");
    (fixtures::json_id(&body["id"]), email)
}

async fn create_child(
    app: &TestApp,
    token: &str,
    guardian_id: i64,
    login: &str,
    events_consent: bool,
) -> (i64, String) {
    let child_type = fixtures::public_type_id_by_name(app, token, "child").await;
    let login = format!("{login}_{}", fixtures::unique_suffix());
    let email = format!("{login}@test.local");
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Child",
                "lastname": login,
                "email": email,
                "accountType": "reader",
                "publicType": child_type.to_string(),
                "sex": "f",
                "birthdate": "2016-04-01",
                "addrCity": "Lyon",
                "guardianId": guardian_id.to_string(),
                "eventsConsent": events_consent
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create child: {body}");
    (fixtures::json_id(&body["id"]), email)
}

async fn create_event(app: &TestApp, token: &str, name: &str, audience: serde_json::Value) -> i64 {
    let mut payload = json!({
        "name": name,
        "eventDate": "2026-12-01"
    });
    let obj = payload.as_object_mut().expect("object");
    for (key, value) in audience.as_object().expect("audience") {
        obj.insert(key.clone(), value.clone());
    }
    let (status, body) = app.post_json("/api/v1/events", &payload, Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "create event: {body}");
    fixtures::json_id(&body["id"])
}

async fn send_announcement(
    app: &TestApp,
    token: &str,
    event_id: i64,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        &format!("/api/v1/events/{event_id}/send-announcement"),
        &json!({
            "subject": "Library event",
            "bodyPlain": "Join us.",
            "bodyHtml": "<p>Join us.</p>"
        }),
        Some(token),
    )
    .await
}

async fn outbox_count(app: &TestApp, event_id: i64, email: &str) -> i64 {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM email_outbox o
        JOIN email_outbox_event_announcements ea ON ea.outbox_id = o.id
        WHERE ea.event_id = $1 AND o.to_addr = $2
        "#,
    )
    .bind(event_id)
    .bind(email)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("count outbox")
}

async fn outbox_body(app: &TestApp, event_id: i64, email: &str) -> String {
    sqlx::query_scalar(
        r#"
        SELECT o.body
        FROM email_outbox o
        JOIN email_outbox_event_announcements ea ON ea.outbox_id = o.id
        WHERE ea.event_id = $1 AND o.to_addr = $2
        ORDER BY o.id
        LIMIT 1
        "#,
    )
    .bind(event_id)
    .bind(email)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("outbox body")
}

async fn consent_source(app: &TestApp, user_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT events_consent_source FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("source")
}

async fn consent_at_present(app: &TestApp, user_id: i64) -> bool {
    let at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT events_consent_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("consent at");
    at.is_some()
}

async fn changed_at(app: &TestApp, user_id: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar("SELECT events_consent_changed_at FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("changed at")
}

async fn notice_timestamp(app: &TestApp, user_id: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar("SELECT events_consent_notice_at FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("notice at")
}

fn token_after(text: &str, marker: &str) -> String {
    let start = text.find(marker).expect("marker") + marker.len();
    let rest = &text[start..];
    let end = rest
        .find(|ch: char| {
            ch == '"' || ch == '\\' || ch == '<' || ch == ' ' || ch == '\n' || ch == '>'
        })
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

async fn post_raw(
    app: &TestApp,
    uri: &str,
    content_type: &str,
    body: &str,
) -> (StatusCode, String) {
    let response = app
        .request(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", content_type)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}
