//! Title-level (biblio) holds: place / promote / cancel / expire / concurrency.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::hold::{CreateHold, HoldStatus};
use serde_json::json;

async fn create_title_with_copies(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    copy_count: usize,
) -> (i64, Vec<i64>) {
    let items: Vec<serde_json::Value> = (0..copy_count)
        .map(|i| {
            json!({
                "barcode": format!("TTL-{}-{i}", fixtures::unique_suffix()),
                "borrowable": true
            })
        })
        .collect();
    let payload = json!({
        "title": title,
        "mediaType": "printedText",
        "lang": "french",
        "items": items
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_ids: Vec<i64> = body["biblio"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| fixtures::json_id(&item["id"]))
        .collect();
    (biblio_id, item_ids)
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": user_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    fixtures::json_id(&body["id"])
}

async fn return_loan(app: &TestApp, admin_token: &str, loan_id: i64) -> serde_json::Value {
    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");
    body
}

async fn place_title_hold(
    app: &TestApp,
    token: &str,
    user_id: i64,
    biblio_id: i64,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/holds",
        &json!({
            "userId": user_id.to_string(),
            "biblioId": biblio_id.to_string()
        }),
        Some(token),
    )
    .await
}

async fn create_capped_public_type(app: &TestApp, admin_token: &str, cap: i16) -> i64 {
    let name = format!("ttlcap_{}", fixtures::unique_suffix());
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
    fixtures::json_id(&body["id"])
}

#[tokio::test]
async fn place_title_level_hold_leaves_item_unassigned() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ttlplace").await;
    let (biblio_id, _) = create_title_with_copies(&app, &admin_token, "Title Hold Place", 2).await;

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "place title hold: {body}");
    assert!(
        body["itemId"].is_null(),
        "title hold must not assign a copy: {body}"
    );
    assert_eq!(fixtures::json_id(&body["biblioId"]), biblio_id);
    assert_eq!(body["status"], "pending");

    let (status, queue) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "title queue: {queue}");
    let rows = queue.as_array().expect("queue array");
    assert_eq!(rows.len(), 1);
    assert!(rows[0]["itemId"].is_null());
    assert_eq!(
        fixtures::json_id(&rows[0]["id"]),
        fixtures::json_id(&body["id"])
    );
}

#[tokio::test]
async fn promote_title_hold_assigns_returned_copy_fifo() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "ttlprom_a").await;
    let (reader_b, token_b) = fixtures::create_reader(&app, &admin_token, "ttlprom_b").await;
    let (reader_c, token_c) = fixtures::create_reader(&app, &admin_token, "ttlprom_c").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Title Hold Promote", 2).await;
    let item_a = item_ids[0];

    let loan_id = checkout(&app, &admin_token, reader_a, item_a).await;

    let (status, hold_b) = place_title_hold(&app, &token_b, reader_b, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold B: {hold_b}");
    let (status, hold_c) = place_title_hold(&app, &token_c, reader_c, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold C: {hold_c}");
    assert!(hold_b["position"].as_i64().unwrap() < hold_c["position"].as_i64().unwrap());

    return_loan(&app, &admin_token, loan_id).await;
    let repo = app.state.services.repository.as_ref();
    let ready = repo
        .holds_get_by_id(fixtures::json_id(&hold_b["id"]))
        .await
        .expect("hold B after return");
    assert_eq!(
        ready.status,
        HoldStatus::Ready,
        "FIFO must promote first title hold"
    );
    assert_eq!(ready.item_id, Some(item_a));
    assert!(ready.expires_at.is_some());

    let after = repo
        .holds_get_by_id(fixtures::json_id(&hold_c["id"]))
        .await
        .expect("hold C still pending");
    assert_eq!(after.status, HoldStatus::Pending);
    assert!(after.item_id.is_none());
}

#[tokio::test]
async fn copy_level_hold_precedes_title_hold_on_that_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "ttlprec_a").await;
    let (reader_b, token_b) = fixtures::create_reader(&app, &admin_token, "ttlprec_b").await;
    let (reader_c, token_c) = fixtures::create_reader(&app, &admin_token, "ttlprec_c").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Title Hold Precedence", 2).await;
    let item_a = item_ids[0];
    let item_b = item_ids[1];

    let loan_a = checkout(&app, &admin_token, reader_a, item_a).await;
    let loan_b = checkout(&app, &admin_token, reader_a, item_b).await;

    let (status, title_hold) = place_title_hold(&app, &token_b, reader_b, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "title hold: {title_hold}");

    let (status, copy_hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_c.to_string(),
                "itemId": item_a.to_string()
            }),
            Some(&token_c),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "copy hold: {copy_hold}");

    return_loan(&app, &admin_token, loan_a).await;
    let repo = app.state.services.repository.as_ref();
    let copy_after = repo
        .holds_get_by_id(fixtures::json_id(&copy_hold["id"]))
        .await
        .expect("copy hold after first return");
    assert_eq!(
        copy_after.status,
        HoldStatus::Ready,
        "copy-level hold on the returned item wins"
    );

    return_loan(&app, &admin_token, loan_b).await;
    let title_after = repo
        .holds_get_by_id(fixtures::json_id(&title_hold["id"]))
        .await
        .expect("title hold after second return");
    assert_eq!(title_after.status, HoldStatus::Ready);
    assert_eq!(title_after.item_id, Some(item_b));
}

#[tokio::test]
async fn concurrent_return_of_two_copies_does_not_double_notify() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "ttlrace_a").await;
    let (reader_b, token_b) = fixtures::create_reader(&app, &admin_token, "ttlrace_b").await;
    let (reader_c, token_c) = fixtures::create_reader(&app, &admin_token, "ttlrace_c").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Title Hold Race", 2).await;

    let loan_1 = checkout(&app, &admin_token, reader_a, item_ids[0]).await;
    let loan_2 = checkout(&app, &admin_token, reader_a, item_ids[1]).await;

    let (status, hold_b) = place_title_hold(&app, &token_b, reader_b, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold B: {hold_b}");
    let (status, hold_c) = place_title_hold(&app, &token_c, reader_c, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold C: {hold_c}");

    let repo = app.state.services.repository.as_ref().clone();
    let (r1, r2) = tokio::join!(repo.loans_return(loan_1), repo.loans_return(loan_2));
    let o1 = r1.expect("return 1");
    let o2 = r2.expect("return 2");

    let ready_ids: Vec<i64> = [o1.readied_hold, o2.readied_hold]
        .into_iter()
        .flatten()
        .map(|h| h.id)
        .collect();
    assert_eq!(
        ready_ids.len(),
        2,
        "each returned copy should notify a different patron: {ready_ids:?}"
    );
    let unique: std::collections::HashSet<i64> = ready_ids.iter().copied().collect();
    assert_eq!(unique.len(), 2, "same patron must not be notified twice");

    let hold_b_row = repo
        .holds_get_by_id(fixtures::json_id(&hold_b["id"]))
        .await
        .expect("hold B");
    let hold_c_row = repo
        .holds_get_by_id(fixtures::json_id(&hold_c["id"]))
        .await
        .expect("hold C");
    assert_eq!(hold_b_row.status, HoldStatus::Ready);
    assert_eq!(hold_c_row.status, HoldStatus::Ready);
    assert_ne!(hold_b_row.item_id, hold_c_row.item_id);
}

#[tokio::test]
async fn cancel_pending_title_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ttlcan").await;
    let (biblio_id, _) = create_title_with_copies(&app, &admin_token, "Title Hold Cancel", 1).await;

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "place: {body}");
    let hold_id = fixtures::json_id(&body["id"]);

    let (status, cancelled) = app
        .delete_with_auth(&format!("/api/v1/holds/{hold_id}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "cancel: {cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
    assert!(cancelled["itemId"].is_null());

    let (status, queue) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "queue after cancel: {queue}");
    let rows = queue.as_array().expect("queue");
    assert!(rows.is_empty(), "cancelled title hold must leave the queue");
}

#[tokio::test]
async fn expire_ready_title_hold_promotes_next_title_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "ttlexp_a").await;
    let (reader_b, token_b) = fixtures::create_reader(&app, &admin_token, "ttlexp_b").await;
    let (reader_c, token_c) = fixtures::create_reader(&app, &admin_token, "ttlexp_c").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Title Hold Expire", 1).await;

    let loan_id = checkout(&app, &admin_token, reader_a, item_ids[0]).await;
    let (status, hold_b) = place_title_hold(&app, &token_b, reader_b, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold B: {hold_b}");
    let (status, hold_c) = place_title_hold(&app, &token_c, reader_c, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "hold C: {hold_c}");

    return_loan(&app, &admin_token, loan_id).await;
    let hold_b_id = fixtures::json_id(&hold_b["id"]);
    sqlx::query("UPDATE holds SET expires_at = NOW() - INTERVAL '1 minute' WHERE id = $1")
        .bind(hold_b_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire hold B");

    let expired = app
        .state
        .services
        .holds
        .expire_overdue()
        .await
        .expect("expire");
    assert!(expired.contains(&hold_b_id), "B should expire: {expired:?}");

    let hold_c_row = app
        .state
        .services
        .repository
        .holds_get_by_id(fixtures::json_id(&hold_c["id"]))
        .await
        .expect("hold C");
    assert_eq!(hold_c_row.status, HoldStatus::Ready);
    assert_eq!(hold_c_row.item_id, Some(item_ids[0]));
}

#[tokio::test]
async fn title_level_hold_counts_toward_max_active() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let public_type_id = create_capped_public_type(&app, &admin_token, 1).await;
    let (reader_id, reader_token) = fixtures::create_reader_with_public_type(
        &app,
        &admin_token,
        "ttlcap",
        Some(public_type_id),
    )
    .await;
    let (biblio_a, _) = create_title_with_copies(&app, &admin_token, "Title Cap A", 1).await;
    let (biblio_b, item_b) = create_title_with_copies(&app, &admin_token, "Title Cap B", 1).await;

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_a).await;
    assert_eq!(status, StatusCode::CREATED, "first title hold: {body}");

    let (status, quota) = app
        .get_json_with_auth("/api/v1/holds/quota", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "quota: {quota}");
    assert_eq!(quota["activeHolds"], 1);
    assert_eq!(quota["remaining"], 0);

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_b).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "second title hold blocked: {body}"
    );

    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_b[0].to_string()
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "copy hold also blocked by title hold slot: {body}"
    );
}

#[tokio::test]
async fn cannot_combine_title_and_copy_hold_on_same_biblio() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ttldup").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Title Hold Dup", 2).await;

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "title: {body}");

    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_ids[0].to_string()
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "copy after title: {body}");

    let (status, body) = place_title_hold(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(status, StatusCode::CONFLICT, "duplicate title: {body}");
}

#[tokio::test]
async fn reject_create_hold_without_item_or_biblio() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ttlnone").await;

    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({ "userId": reader_id.to_string() }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "missing target: {body}");
}

#[tokio::test]
async fn repository_title_hold_create_helper_matches_api() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "ttlrepo").await;
    let (biblio_id, _) = create_title_with_copies(&app, &admin_token, "Title Hold Repo", 1).await;

    let hold = app
        .state
        .services
        .repository
        .holds_create(&CreateHold::for_biblio(reader_id, biblio_id))
        .await
        .expect("repo title hold");
    assert_eq!(hold.biblio_id, biblio_id);
    assert!(hold.item_id.is_none());
    assert_eq!(hold.status, HoldStatus::Pending);
}
