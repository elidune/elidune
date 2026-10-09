//! Event audiences: API create/update/read and legacy column migration.

mod common;

use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

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
    let token = fixtures::ensure_first_setup(&app).await;

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
                "name": "No audience",
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
                "name": "Legacy single",
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
                "name": "Two audiences",
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
        .get_json_with_auth("/api/v1/events?perPage=100", &token)
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
