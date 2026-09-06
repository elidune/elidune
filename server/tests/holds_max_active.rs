//! Max concurrent active holds: policy, place_hold cap, staff force.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::hold::CreateHold;
use serde_json::json;

async fn create_borrowable_item(app: &TestApp, admin_token: &str, prefix: &str) -> i64 {
    let payload = json!({
        "title": format!("Hold cap {prefix}"),
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
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn create_capped_public_type(app: &TestApp, admin_token: &str, cap: i16) -> i64 {
    let name = format!("holdcap_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "maxActiveHolds": cap
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create public type: {body}");
    assert_eq!(body["maxActiveHolds"], cap);
    fixtures::json_id(&body["id"])
}

async fn place_hold(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/holds",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(token),
    )
    .await
}

#[tokio::test]
async fn policy_is_readable_by_desk_and_opac() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "holdpol").await;

    let (status, body) = app
        .get_json_with_auth("/api/v1/holds/policy", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "desk policy: {body}");
    assert!(
        body["maxActiveHolds"].as_i64().unwrap_or(0) >= 1,
        "global cap: {body}"
    );

    let (status, body) = app
        .get_json_with_auth("/api/v1/holds/policy", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "opac policy: {body}");

    let (status, types) = app
        .get_json_with_auth("/api/v1/public-types", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "public types: {types}");
    assert!(
        types
            .as_array()
            .is_some_and(|rows| rows.iter().all(|row| row.get("maxActiveHolds").is_some())),
        "each public type exposes maxActiveHolds: {types}"
    );
}

#[tokio::test]
async fn place_hold_under_public_type_cap_succeeds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 2).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdunder",
        Some(public_type_id),
    )
    .await;

    let item_id = create_borrowable_item(&app, &admin_token, "UNDER").await;
    let (status, body) = place_hold(&app, &reader_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "under cap: {body}");

    let (status, quota) = app
        .get_json_with_auth("/api/v1/holds/quota", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "quota: {quota}");
    assert_eq!(quota["maxActiveHolds"], 2);
    assert_eq!(quota["activeHolds"], 1);
    assert_eq!(quota["remaining"], 1);
}

#[tokio::test]
async fn place_hold_at_public_type_cap_is_rejected() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 2).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdat",
        Some(public_type_id),
    )
    .await;

    let first = create_borrowable_item(&app, &admin_token, "AT1").await;
    let second = create_borrowable_item(&app, &admin_token, "AT2").await;
    let third = create_borrowable_item(&app, &admin_token, "AT3").await;

    let (status, body) = place_hold(&app, &reader_token, reader_id, first, false).await;
    assert_eq!(status, StatusCode::CREATED, "first: {body}");
    let (status, body) = place_hold(&app, &reader_token, reader_id, second, false).await;
    assert_eq!(status, StatusCode::CREATED, "at cap after second: {body}");

    let (status, body) = place_hold(&app, &reader_token, reader_id, third, false).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "at cap: {body}");
    let message = body["message"].as_str().unwrap_or_default();
    assert!(message.contains("Maximum of 2"), "{message}");
    assert!(message.contains("force=true"), "{message}");
    assert_eq!(body["code"], "business_rule_violation");
}

#[tokio::test]
async fn place_hold_over_cap_is_rejected() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 2).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdover",
        Some(public_type_id),
    )
    .await;

    let first = create_borrowable_item(&app, &admin_token, "OV1").await;
    let second = create_borrowable_item(&app, &admin_token, "OV2").await;
    let third = create_borrowable_item(&app, &admin_token, "OV3").await;

    app.state
        .services
        .repository
        .holds_create(&CreateHold {
            user_id: reader_id,
            item_id: first,
            notes: None,
            force: false,
        })
        .await
        .expect("seed hold 1");
    app.state
        .services
        .repository
        .holds_create(&CreateHold {
            user_id: reader_id,
            item_id: second,
            notes: None,
            force: false,
        })
        .await
        .expect("seed hold 2");

    let (status, body) = app
        .put_json(
            &format!("/api/v1/public-types/{public_type_id}"),
            &json!({ "maxActiveHolds": 1 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "tighten cap: {body}");

    let (status, body) = place_hold(&app, &reader_token, reader_id, third, false).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "over cap: {body}");
}

#[tokio::test]
async fn staff_force_override_is_audited() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 1).await;
    let (reader_id, _) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdforce",
        Some(public_type_id),
    )
    .await;

    let first = create_borrowable_item(&app, &admin_token, "FRC1").await;
    let second = create_borrowable_item(&app, &admin_token, "FRC2").await;

    let (status, body) = place_hold(&app, &admin_token, reader_id, first, false).await;
    assert_eq!(status, StatusCode::CREATED, "first: {body}");

    let (status, body) = place_hold(&app, &admin_token, reader_id, second, false).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blocked: {body}");

    let (status, body) = place_hold(&app, &admin_token, reader_id, second, true).await;
    assert_eq!(status, StatusCode::CREATED, "force: {body}");

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, audit) = app
        .get_json_with_auth("/api/v1/audit?eventType=hold.max_overridden", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "audit query: {audit}");
    let entries = audit["entries"].as_array().cloned().unwrap_or_default();
    assert!(
        entries
            .iter()
            .any(|row| row["payload"]["operation"] == "place_hold"
                && row["payload"]["max"] == 1
                && row["entityId"] == reader_id),
        "expected override audit, got {audit}"
    );
}

#[tokio::test]
async fn reader_cannot_force_the_holds_cap() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 1).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdnoforce",
        Some(public_type_id),
    )
    .await;

    let first = create_borrowable_item(&app, &admin_token, "NF1").await;
    let second = create_borrowable_item(&app, &admin_token, "NF2").await;
    let (status, body) = place_hold(&app, &reader_token, reader_id, first, false).await;
    assert_eq!(status, StatusCode::CREATED, "first: {body}");

    let (status, body) = place_hold(&app, &reader_token, reader_id, second, true).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader force: {body}");
}

#[tokio::test]
async fn public_type_override_is_tighter_than_global() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 1).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "holdpt",
        Some(public_type_id),
    )
    .await;

    let (status, policy) = app
        .get_json_with_auth("/api/v1/holds/policy", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "policy: {policy}");
    let global = policy["maxActiveHolds"].as_i64().unwrap_or(0);
    assert!(
        global > 1,
        "global default should exceed override: {policy}"
    );

    let first = create_borrowable_item(&app, &admin_token, "PT1").await;
    let second = create_borrowable_item(&app, &admin_token, "PT2").await;
    let (status, body) = place_hold(&app, &reader_token, reader_id, first, false).await;
    assert_eq!(status, StatusCode::CREATED, "first: {body}");
    let (status, body) = place_hold(&app, &reader_token, reader_id, second, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "public-type cap: {body}"
    );
}
