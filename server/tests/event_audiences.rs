//! Event audiences: API create/update/read and legacy column migration.

mod common;

use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;
use sqlx::Row;

fn legacy_copy_statements() -> Vec<String> {
    let migration = include_str!("../migrations/036_events_audiences.sql");
    let start = migration
        .find("-- BEGIN LEGACY AUDIENCE COPY")
        .expect("start marker");
    let end = migration
        .find("-- END LEGACY AUDIENCE COPY")
        .expect("end marker");
    migration[start..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
        .split(';')
        .map(|statement| statement.trim().to_string())
        .filter(|statement| !statement.is_empty())
        .collect()
}

#[tokio::test]
async fn migration_copies_null_and_single_public_type() {
    if std::env::var("DATABASE_URL").is_err() && std::env::var("CI").is_err() {
        eprintln!("Skipping integration test: DATABASE_URL not set");
        return;
    }
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");

    let mut tx = pool.begin().await.expect("begin");

    sqlx::query(
        r#"
        CREATE TEMP TABLE events (
            id BIGINT PRIMARY KEY,
            public_type VARCHAR(50),
            all_audiences BOOLEAN NOT NULL DEFAULT FALSE
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("temp events");

    sqlx::query(
        r#"
        CREATE TEMP TABLE event_audiences (
            event_id BIGINT NOT NULL,
            audience VARCHAR(50) NOT NULL,
            PRIMARY KEY (event_id, audience)
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("temp event_audiences");

    sqlx::query(
        "INSERT INTO events (id, public_type, all_audiences) VALUES (1, NULL, FALSE), (2, 'child', FALSE)",
    )
    .execute(&mut *tx)
    .await
    .expect("fixture rows");

    for statement in legacy_copy_statements() {
        sqlx::query(&statement)
            .execute(&mut *tx)
            .await
            .unwrap_or_else(|err| panic!("legacy copy failed: {statement}\n{err}"));
    }

    let all_flag: bool = sqlx::query_scalar("SELECT all_audiences FROM events WHERE id = 1")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(all_flag, "NULL public_type becomes all_audiences");
    let all_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM event_audiences WHERE event_id = 1")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(all_links, 0);

    let child_flag: bool = sqlx::query_scalar("SELECT all_audiences FROM events WHERE id = 2")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(!child_flag);
    let audience: String =
        sqlx::query_scalar("SELECT audience FROM event_audiences WHERE event_id = 2")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(audience, "child");

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn event_audiences_create_update_read_and_reject_empty() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    // Shared CI database: first-setup may already have run, and requests need a peer
    // address for the rate limiter. `ensure_first_setup` and `TestApp::request` cover both.
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, types_body) = app.get_json_with_auth("/api/v1/public-types", &token).await;
    assert_eq!(status, StatusCode::OK, "public types: {types_body}");
    let names: Vec<String> = types_body
        .as_array()
        .expect("public types array")
        .iter()
        .filter_map(|row| row["name"].as_str().map(str::to_string))
        .collect();
    assert!(
        names.len() >= 2,
        "need two public types to test multi-audience, got {names:?}"
    );
    let first = &names[0];
    let second = &names[1];

    let (empty_status, empty_body) = app
        .post_json(
            "/api/v1/events",
            &json!({
                "name": format!("No audience {suffix}"),
                "eventDate": "2026-09-01",
                "allAudiences": false,
                "publicTypes": []
            }),
            Some(&token),
        )
        .await;
    assert_eq!(
        empty_status,
        StatusCode::BAD_REQUEST,
        "empty selection: {empty_body}"
    );

    let (legacy_status, legacy_body) = app
        .post_json(
            "/api/v1/events",
            &json!({
                "name": format!("Legacy single {suffix}"),
                "eventDate": "2026-09-02",
                "publicType": first
            }),
            Some(&token),
        )
        .await;
    assert_eq!(legacy_status, StatusCode::CREATED, "legacy: {legacy_body}");
    assert_eq!(legacy_body["allAudiences"], false);
    assert_eq!(legacy_body["publicTypes"], json!([first]));
    let legacy_id = fixtures::json_id(&legacy_body["id"]);

    let (multi_status, multi_body) = app
        .post_json(
            "/api/v1/events",
            &json!({
                "name": format!("Two audiences {suffix}"),
                "eventDate": "2026-09-03",
                "publicTypes": [first, second, first]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(multi_status, StatusCode::CREATED, "multi: {multi_body}");
    assert_eq!(multi_body["allAudiences"], false);
    let mut stored = multi_body["publicTypes"]
        .as_array()
        .expect("publicTypes")
        .clone();
    stored.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    let mut expected = vec![json!(first), json!(second)];
    expected.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    assert_eq!(stored, expected, "duplicates dropped");
    let multi_id = fixtures::json_id(&multi_body["id"]);

    let (read_status, read_body) = app
        .get_json_with_auth(&format!("/api/v1/events/{multi_id}"), &token)
        .await;
    assert_eq!(read_status, StatusCode::OK, "read: {read_body}");
    assert_eq!(read_body["publicTypes"], multi_body["publicTypes"]);

    let (all_status, all_body) = app
        .put_json(
            &format!("/api/v1/events/{multi_id}"),
            &json!({
                "allAudiences": true,
                "publicTypes": []
            }),
            Some(&token),
        )
        .await;
    assert_eq!(all_status, StatusCode::OK, "update all: {all_body}");
    assert_eq!(all_body["allAudiences"], true);
    assert_eq!(all_body["publicTypes"], json!([]));

    let (list_status, list_body) = app
        .get_json_with_auth(
            "/api/v1/events?startDate=2026-09-03&endDate=2026-09-03&perPage=100",
            &token,
        )
        .await;
    assert_eq!(list_status, StatusCode::OK, "list: {list_body}");
    let listed = list_body["events"]
        .as_array()
        .expect("events")
        .iter()
        .find(|event| fixtures::json_id(&event["id"]) == multi_id)
        .expect("updated event in list");
    assert_eq!(listed["allAudiences"], true);

    for id in [legacy_id, multi_id] {
        let response = app
            .request(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/events/{id}"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}

/// A child-only event emails the guardian once and names the child.
/// The child's own `receive_reminders` does not matter.
#[tokio::test]
async fn child_only_event_emails_the_guardian_and_names_the_child() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_guard").await;
    grant_events_consent(&app, guardian_id).await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let (child_id, child_email) = create_child(
        &app,
        &token,
        guardian_id,
        "Lea",
        &format!("Martin{suffix}"),
        &format!("child_{suffix}"),
    )
    .await;
    sqlx::query("UPDATE users SET receive_reminders = FALSE WHERE id = $1")
        .bind(child_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("clear child opt-in");

    let event_id = create_event(
        &app,
        &token,
        &format!("Child only {suffix}"),
        "2026-11-01",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send child announcement: {body}");
    let mailed = outbox_bodies_for(&app, event_id, &guardian_email).await;
    assert_eq!(mailed.len(), 1, "guardian gets one email: {mailed:?}");
    assert!(
        mailed[0].contains(&format!("Lea Martin{suffix}")),
        "email must name the child: {}",
        mailed[0]
    );
    assert!(
        mailed[0].contains("Cette invitation concerne"),
        "default language is french: {}",
        mailed[0]
    );
    assert_eq!(
        outbox_rows_for(&app, event_id, &child_email).await,
        0,
        "child address must not be queued"
    );
}

/// One guardian, two targeted children: a single email names both.
#[tokio::test]
async fn guardian_of_two_children_gets_one_email_naming_both() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_two").await;
    grant_events_consent(&app, guardian_id).await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let _ = create_child(
        &app,
        &token,
        guardian_id,
        "Lea",
        &format!("One{suffix}"),
        &format!("child_a_{suffix}"),
    )
    .await;
    let _ = create_child(
        &app,
        &token,
        guardian_id,
        "Noe",
        &format!("Two{suffix}"),
        &format!("child_b_{suffix}"),
    )
    .await;

    let event_id = create_event(
        &app,
        &token,
        &format!("Two children {suffix}"),
        "2026-11-03",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    let mailed = outbox_bodies_for(&app, event_id, &guardian_email).await;
    assert_eq!(mailed.len(), 1, "one email for both children: {mailed:?}");
    assert!(
        mailed[0].contains(&format!("Lea One{suffix}")),
        "missing first child: {}",
        mailed[0]
    );
    assert!(
        mailed[0].contains(&format!("Noe Two{suffix}")),
        "missing second child: {}",
        mailed[0]
    );
}

/// A guardian who is also in the audience still receives a single email.
#[tokio::test]
async fn guardian_also_in_the_audience_gets_one_email() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_both").await;
    grant_events_consent(&app, guardian_id).await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let _ = create_child(
        &app,
        &token,
        guardian_id,
        "Ada",
        &format!("Both{suffix}"),
        &format!("child_both_{suffix}"),
    )
    .await;

    let event_id = create_event(
        &app,
        &token,
        &format!("Adult and child {suffix}"),
        "2026-11-04",
        json!({ "publicTypes": ["adult", "child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    let mailed = outbox_bodies_for(&app, event_id, &guardian_email).await;
    assert_eq!(
        mailed.len(),
        1,
        "direct match and guardian route collapse to one email: {mailed:?}"
    );
    assert!(
        mailed[0].contains(&format!("Ada Both{suffix}")),
        "the one email still names the child: {}",
        mailed[0]
    );
}

/// An erased child does not cause an email to their guardian.
#[tokio::test]
async fn erased_child_does_not_email_the_guardian() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_erased_child").await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let (child_id, _) = create_child(
        &app,
        &token,
        guardian_id,
        "Gone",
        &format!("Erased{suffix}"),
        &format!("child_erased_{suffix}"),
    )
    .await;

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{child_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "erase child: {body}");

    let event_id = create_event(
        &app,
        &token,
        &format!("Erased child {suffix}"),
        "2026-11-05",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    assert_eq!(
        outbox_rows_for(&app, event_id, &guardian_email).await,
        0,
        "erased child must not route to the guardian"
    );
}

/// The guardian's event consent is required. `receive_reminders` and the child's consent are not substitutes.
#[tokio::test]
async fn guardian_without_consent_gets_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_nocon").await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let (child_id, child_email) = create_child(
        &app,
        &token,
        guardian_id,
        "Mia",
        &format!("NoCon{suffix}"),
        &format!("child_nocon_{suffix}"),
    )
    .await;
    sqlx::query(
        "UPDATE users SET receive_reminders = TRUE, events_consent_at = NULL, events_consent_source = NULL WHERE id = $1",
    )
    .bind(guardian_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("clear guardian event consent");
    sqlx::query(
        "UPDATE users SET events_consent_at = NOW(), events_consent_source = 'desk', events_consent_changed_at = NOW() WHERE id = $1",
    )
    .bind(child_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("child consent must not count");

    let event_id = create_event(
        &app,
        &token,
        &format!("No consent {suffix}"),
        "2026-11-06",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    assert_eq!(outbox_rows_for(&app, event_id, &guardian_email).await, 0);
    assert_eq!(outbox_rows_for(&app, event_id, &child_email).await, 0);
}

/// A targeted child with no guardian link is not emailed, and nobody else is on their behalf.
#[tokio::test]
async fn child_without_guardian_gets_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_nolink").await;
    let guardian_email = patron_email(&app, &token, guardian_id).await;
    let (child_id, child_email) = create_child(
        &app,
        &token,
        guardian_id,
        "Orphan",
        &format!("NoLink{suffix}"),
        &format!("child_nolink_{suffix}"),
    )
    .await;
    sqlx::query("DELETE FROM user_guardians WHERE child_id = $1")
        .bind(child_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("drop guardian link");

    let event_id = create_event(
        &app,
        &token,
        &format!("No guardian {suffix}"),
        "2026-11-07",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_template_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send: {body}");
    assert_eq!(outbox_rows_for(&app, event_id, &guardian_email).await, 0);
    assert_eq!(outbox_rows_for(&app, event_id, &child_email).await, 0);
}

/// Erasure sets `status = deleted` and `receive_reminders = false`. That user stays out
/// of the recipient query even if an email address is still present.
#[tokio::test]
async fn erased_patron_receives_no_announcement() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (reader_id, _) = fixtures::create_reader(&app, &token, "announce_erase").await;

    let (status, user) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::OK, "read patron: {user}");
    let email = user["email"].as_str().expect("patron email").to_string();

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{reader_id}"), &token)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "erase patron: {body}");

    let row = sqlx::query("SELECT status, receive_reminders, email FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("erased row");
    let status_name: Option<String> = row.get("status");
    let receive_reminders: bool = row.get("receive_reminders");
    let stored_email: Option<String> = row.get("email");
    assert_eq!(status_name.as_deref(), Some("deleted"));
    assert!(!receive_reminders);
    assert!(stored_email.is_none(), "erasure clears email");

    // Put the address and consent back so a dropped status filter would enqueue this patron.
    sqlx::query(
        "UPDATE users SET email = $1, events_consent_at = NOW(), events_consent_source = 'desk', events_consent_changed_at = NOW() WHERE id = $2",
    )
    .bind(&email)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("restore email and consent");

    let event_id = create_event(
        &app,
        &token,
        &format!("All audiences {suffix}"),
        "2026-11-02",
        json!({ "allAudiences": true }),
    )
    .await;

    let (status, body) = send_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send after erasure: {body}");
    assert_eq!(
        outbox_rows_for(&app, event_id, &email).await,
        0,
        "erased patron must not be queued: {body}"
    );
}

async fn create_event(
    app: &TestApp,
    token: &str,
    name: &str,
    event_date: &str,
    audience: serde_json::Value,
) -> i64 {
    let mut payload = json!({
        "name": name,
        "eventDate": event_date
    });
    let obj = payload.as_object_mut().expect("object");
    for (key, value) in audience.as_object().expect("audience object") {
        obj.insert(key.clone(), value.clone());
    }
    let (status, body) = app.post_json("/api/v1/events", &payload, Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "create event: {body}");
    fixtures::json_id(&body["id"])
}

async fn create_child(
    app: &TestApp,
    token: &str,
    guardian_id: i64,
    firstname: &str,
    lastname: &str,
    login_prefix: &str,
) -> (i64, String) {
    let child_type = fixtures::public_type_id_by_name(app, token, "child").await;
    let login = format!("{login_prefix}_{}", fixtures::unique_suffix());
    let email = format!("{login}@test.local");
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": firstname,
                "lastname": lastname,
                "email": email,
                "accountType": "reader",
                "publicType": child_type.to_string(),
                "sex": "f",
                "birthdate": "2016-04-01",
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create child: {body}");
    (fixtures::json_id(&body["id"]), email)
}

async fn grant_events_consent(app: &TestApp, user_id: i64) {
    sqlx::query(
        "UPDATE users SET events_consent_at = NOW(), events_consent_source = 'desk', events_consent_changed_at = NOW() WHERE id = $1",
    )
    .bind(user_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("grant events consent");
}

async fn patron_email(app: &TestApp, token: &str, user_id: i64) -> String {
    let (status, user) = app
        .get_json_with_auth(&format!("/api/v1/users/{user_id}"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "read patron: {user}");
    user["email"].as_str().expect("patron email").to_string()
}

async fn send_template_announcement(
    app: &TestApp,
    token: &str,
    event_id: i64,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        &format!("/api/v1/events/{event_id}/send-announcement"),
        &json!({}),
        Some(token),
    )
    .await
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

async fn outbox_bodies_for(app: &TestApp, event_id: i64, email: &str) -> Vec<String> {
    sqlx::query_scalar(
        r#"
        SELECT o.body
        FROM email_outbox o
        JOIN email_outbox_event_announcements ea ON ea.outbox_id = o.id
        WHERE ea.event_id = $1 AND o.to_addr = $2
        ORDER BY o.id
        "#,
    )
    .bind(event_id)
    .bind(email)
    .fetch_all(app.state.services.repository.pool())
    .await
    .expect("outbox bodies")
}

async fn outbox_rows_for(app: &TestApp, event_id: i64, email: &str) -> i64 {
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
    .expect("outbox count")
}
