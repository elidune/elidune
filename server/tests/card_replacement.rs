//! Lost-card report and card replacement.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::{json, Value};
use sqlx::Row;

fn retarget(token: &str, secret: &str, users: Rights, loans: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.users_rights = users;
    claims.rights.loans_rights = loans;
    claims.create_token(secret).expect("mint test token")
}

fn assert_error_shape(body: &Value) {
    assert!(
        body["code"].as_str().is_some_and(|code| !code.is_empty()),
        "{body}"
    );
    assert!(
        body["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty()),
        "{body}"
    );
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{body}"
    );
}

async fn create_patron(app: &TestApp, admin_token: &str, prefix: &str, card: &str) -> i64 {
    let suffix = fixtures::unique_suffix();
    let login = format!("{prefix}_{suffix}");
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Isolde",
                "lastname": "Moreau",
                "email": format!("{login}@secret.test"),
                "addrCity": "Paris",
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "barcode": card,
                "sex": "f",
                "birthdate": "1990-06-01"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create patron: {body}");
    fixtures::json_id(&body["id"])
}

async fn borrowable_item(app: &TestApp, admin_token: &str) -> i64 {
    let suffix = fixtures::unique_suffix();
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Card replace {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": format!("COPY-{suffix}"), "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {biblio}");
    fixtures::json_id(&biblio["biblio"]["items"][0]["id"])
}

async fn checkout(app: &TestApp, token: &str, user_id: i64, item_id: i64) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string()
        }),
        Some(token),
    )
    .await
}

async fn report(app: &TestApp, token: &str, user_id: i64, body: &Value) -> (StatusCode, Value) {
    app.post_json(
        &format!("/api/v1/users/{user_id}/card-lost"),
        body,
        Some(token),
    )
    .await
}

async fn replace(app: &TestApp, token: &str, user_id: i64, barcode: &str) -> (StatusCode, Value) {
    app.post_json(
        &format!("/api/v1/users/{user_id}/replace-card"),
        &json!({ "barcode": barcode }),
        Some(token),
    )
    .await
}

fn audit_entry<'a>(body: &'a Value, event_type: &str) -> Option<&'a Value> {
    body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["eventType"] == event_type)
}

async fn wait_audit(app: &TestApp, token: &str, event_type: &str, entity_id: i64) -> Value {
    let path = format!("/api/v1/audit?eventType={event_type}&entityId={entity_id}&perPage=20");
    for _ in 0..25 {
        let (status, body) = app.get_json_with_auth(&path, token).await;
        assert_eq!(status, StatusCode::OK, "audit {event_type}: {body}");
        if audit_entry(&body, event_type).is_some() {
            return body;
        }
        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    }
    panic!("missing audit {event_type} for {entity_id}");
}

#[tokio::test]
async fn report_card_lost_requires_reason_and_is_audited() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let actor_id = fixtures::json_id(&me["id"]);

    let suffix = fixtures::unique_suffix();
    let card = format!("LOST-{suffix}");
    let user_id = create_patron(&app, &token, "lostreq", &card).await;

    let (status, missing) = report(&app, &token, user_id, &json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing}");
    assert_error_shape(&missing);
    assert_eq!(missing["code"], "validation_error", "{missing}");

    let (status, unknown) = report(
        &app,
        &token,
        user_id,
        &json!({ "reason": "misplaced", "detail": "desk note" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{unknown}");
    assert_eq!(unknown["code"], "validation_error", "{unknown}");

    let lost: bool = sqlx::query_scalar("SELECT card_lost FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("card_lost");
    assert!(!lost, "a rejected report must not flag the card");

    let (status, body) = report(
        &app,
        &token,
        user_id,
        &json!({ "reason": "stolen", "detail": "  picked from a bag  " }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["userId"], user_id.to_string(), "{body}");
    assert_eq!(body["cardLost"], true, "{body}");
    assert_eq!(body["reason"], "stolen", "{body}");
    assert_eq!(body["detail"], "picked from a bag", "{body}");

    let lost: bool = sqlx::query_scalar("SELECT card_lost FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("card_lost after report");
    assert!(lost);

    let audit = wait_audit(&app, &token, "user.card_lost", user_id).await;
    let entry = audit_entry(&audit, "user.card_lost").expect("entry");
    assert_eq!(entry["outcome"], "success", "{entry}");
    assert_eq!(fixtures::json_id(&entry["userId"]), actor_id, "{entry}");
    assert_eq!(entry["payload"]["reason"], "stolen", "{entry}");
    assert_eq!(entry["payload"]["detail"], "picked from a bag", "{entry}");
    assert_eq!(
        fixtures::json_id(&entry["payload"]["actorId"]),
        actor_id,
        "{entry}"
    );
}

#[tokio::test]
async fn replace_card_assigns_barcode_unblocks_checkout_and_is_audited() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    let actor_id = fixtures::json_id(&me["id"]);

    let suffix = fixtures::unique_suffix();
    let old_card = format!("OLD-{suffix}");
    let new_card = format!("NEW-{suffix}");
    let user_id = create_patron(&app, &token, "replace", &old_card).await;
    let blocked_item = borrowable_item(&app, &token).await;

    let (status, reported) = report(
        &app,
        &token,
        user_id,
        &json!({ "reason": "lost", "detail": "left on the bus" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reported}");

    let (status, refused) = checkout(&app, &token, user_id, blocked_item).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["code"], "card_lost", "{refused}");

    let (status, replaced) = replace(&app, &token, user_id, &new_card).await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["barcode"], new_card, "{replaced}");
    assert_eq!(replaced["previousBarcode"], old_card, "{replaced}");
    assert_eq!(replaced["cardLost"], false, "{replaced}");
    assert_eq!(replaced["user"]["barcode"], new_card, "{replaced}");

    let row = sqlx::query("SELECT barcode, card_lost FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("user after replace");
    let barcode: Option<String> = row.get("barcode");
    let card_lost: bool = row.get("card_lost");
    assert_eq!(barcode.as_deref(), Some(new_card.as_str()));
    assert!(!card_lost);

    let retired: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM retired_card_numbers WHERE card_number = $1 AND retired_by = $2",
    )
    .bind(&old_card)
    .bind(actor_id)
    .fetch_optional(app.state.services.repository.pool())
    .await
    .expect("retired row");
    assert_eq!(retired, Some(user_id));

    let next_item = borrowable_item(&app, &token).await;
    let (status, loan) = checkout(&app, &token, user_id, next_item).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "checkout after replace: {loan}"
    );

    let audit = wait_audit(&app, &token, "user.card_replaced", user_id).await;
    let entry = audit_entry(&audit, "user.card_replaced").expect("entry");
    assert_eq!(entry["outcome"], "success", "{entry}");
    assert_eq!(fixtures::json_id(&entry["userId"]), actor_id, "{entry}");
    assert_eq!(entry["payload"]["previousBarcode"], old_card, "{entry}");
    assert_eq!(entry["payload"]["barcode"], new_card, "{entry}");
    assert_eq!(
        fixtures::json_id(&entry["payload"]["actorId"]),
        actor_id,
        "{entry}"
    );
}

#[tokio::test]
async fn retired_card_number_reuse_is_conflict() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let old_card = format!("RETIRED-{suffix}");
    let new_card = format!("FRESH-{suffix}");
    let other_card = format!("OTHER-{suffix}");
    let user_id = create_patron(&app, &token, "retiree", &old_card).await;
    let (status, replaced) = replace(&app, &token, user_id, &new_card).await;
    assert_eq!(status, StatusCode::OK, "{replaced}");

    let public_type_id = fixtures::public_type_id_by_name(&app, &token, "adult").await;
    let login = format!("reuse_{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Nora",
                "lastname": "Blanc",
                "addrCity": "Lyon",
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "barcode": old_card,
                "sex": "f",
                "birthdate": "1988-02-02"
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{created}");
    assert_error_shape(&created);
    assert_eq!(created["code"], "card_number_retired", "{created}");

    let other_id = create_patron(&app, &token, "reuseother", &other_card).await;
    let (status, updated) = app
        .put_json(
            &format!("/api/v1/users/{other_id}"),
            &json!({ "barcode": old_card }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{updated}");
    assert_eq!(updated["code"], "card_number_retired", "{updated}");
    let still: Option<String> = sqlx::query_scalar("SELECT barcode FROM users WHERE id = $1")
        .bind(other_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("other barcode");
    assert_eq!(still.as_deref(), Some(other_card.as_str()));

    let (status, again) = replace(&app, &token, other_id, &old_card).await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    assert_eq!(again["code"], "card_number_retired", "{again}");

    let (status, same_patron) = replace(&app, &token, user_id, &old_card).await;
    assert_eq!(status, StatusCode::CONFLICT, "{same_patron}");
    assert_eq!(same_patron["code"], "card_number_retired", "{same_patron}");
    let current: Option<String> = sqlx::query_scalar("SELECT barcode FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("original barcode");
    assert_eq!(current.as_deref(), Some(new_card.as_str()));
}

#[tokio::test]
async fn loans_only_staff_cannot_report_or_replace_card() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let suffix = fixtures::unique_suffix();
    let user_id = create_patron(&app, &admin_token, "loansonly", &format!("DESK-{suffix}")).await;

    let (status, reported) = report(&app, &loans_only, user_id, &json!({ "reason": "lost" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{reported}");
    assert_error_shape(&reported);
    assert_eq!(reported["code"], "authorization_failed", "{reported}");

    let (status, replaced) = replace(&app, &loans_only, user_id, &format!("NEXT-{suffix}")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{replaced}");
    assert_error_shape(&replaced);
    assert_eq!(replaced["code"], "authorization_failed", "{replaced}");

    let lost: bool = sqlx::query_scalar("SELECT card_lost FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("card_lost unchanged");
    assert!(!lost);
}
