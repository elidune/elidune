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

/// Child patrons are not emailed until guardian routing (#59) exists.
#[tokio::test]
async fn child_only_event_enqueues_no_announcement() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (guardian_id, _) = fixtures::create_reader(&app, &token, "announce_guard").await;
    let child_type = fixtures::public_type_id_by_name(&app, &token, "child").await;
    let child_email = format!("child_{suffix}@test.local");
    let login = format!("child_{suffix}");
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Minor",
                "lastname": login,
                "email": child_email,
                "accountType": "reader",
                "publicType": child_type.to_string(),
                "sex": "f",
                "birthdate": "2016-04-01",
                "addrCity": "Paris",
                "guardianId": guardian_id.to_string()
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create child: {body}");
    let child_id = fixtures::json_id(&body["id"]);

    let opted_in: bool = sqlx::query_scalar(
        r#"
        SELECT u.receive_reminders
        FROM users u
        JOIN public_types pt ON pt.id = u.public_type
        WHERE u.id = $1 AND pt.name = 'child' AND u.email = $2
        "#,
    )
    .bind(child_id)
    .bind(&child_email)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("child opted in");
    assert!(
        opted_in,
        "child fixture must match the pre-exclusion recipient query"
    );

    let event_id = create_event(
        &app,
        &token,
        &format!("Child only {suffix}"),
        "2026-11-01",
        json!({ "publicTypes": ["child"] }),
    )
    .await;

    let (status, body) = send_announcement(&app, &token, event_id).await;
    assert_eq!(status, StatusCode::OK, "send child announcement: {body}");
    assert_eq!(
        body["emailsSent"], 0,
        "child audience must enqueue nothing: {body}"
    );
    assert_eq!(pending_announcement_count(&app, event_id).await, 0);
    assert_eq!(
        outbox_rows_for(&app, event_id, &child_email).await,
        0,
        "child address must not be queued"
    );
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

    // Put the address back so a dropped status/consent filter would enqueue this patron.
    sqlx::query("UPDATE users SET email = $1 WHERE id = $2")
        .bind(&email)
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("restore email");

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

async fn pending_announcement_count(app: &TestApp, event_id: i64) -> i64 {
    app.state
        .services
        .repository
        .email_outbox_pending_event_announcement_count(event_id)
        .await
        .expect("pending announcement count")
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
