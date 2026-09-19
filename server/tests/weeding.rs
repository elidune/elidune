//! Weeding lifecycle on the item: candidate circulates, withdrawn does not.
//! Part of #56, #63, #64.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::item::WeedingStatus;
use serde_json::json;

async fn unlock_admin(app: &TestApp, admin_token: &str) {
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", admin_token).await;
    if status != StatusCode::OK {
        return;
    }
    let admin_id = fixtures::json_id(&me["id"]);
    let _ = app
        .state
        .services
        .users
        .set_must_change_password(admin_id, false)
        .await;
}

async fn create_title_with_copies(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    copy_count: usize,
) -> (i64, Vec<i64>) {
    let items: Vec<serde_json::Value> = (0..copy_count)
        .map(|i| {
            json!({
                "barcode": format!("WD-{}-{i}", fixtures::unique_suffix()),
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

async fn set_weeding(
    app: &TestApp,
    admin_token: &str,
    item_id: i64,
    status: &str,
    reason: &str,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        &format!("/api/v1/items/{item_id}/weeding"),
        &json!({ "status": status, "reason": reason }),
        Some(admin_token),
    )
    .await
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string()
        }),
        Some(admin_token),
    )
    .await
}

async fn place_copy_hold(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/holds",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string()
        }),
        Some(token),
    )
    .await
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

async fn item_copy(app: &TestApp, admin_token: &str, item_id: i64) -> serde_json::Value {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "get item: {body}");
    body["items"][0].clone()
}

#[tokio::test]
async fn candidate_stays_circulable_for_checkout_renew_and_holds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "wdcand").await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "wdcandh").await;
    let (_, item_ids) =
        create_title_with_copies(&app, &admin_token, "Candidate Circulates", 1).await;
    let item_id = item_ids[0];

    let (status, body) =
        set_weeding(&app, &admin_token, item_id, "candidate", "worn but usable").await;
    assert_eq!(status, StatusCode::OK, "mark candidate: {body}");
    assert_eq!(body["weedingStatus"], "candidate");
    assert_eq!(body["weedingReason"], "worn but usable");
    assert!(
        body["archivedAt"].is_null(),
        "candidate must not set archived_at: {body}"
    );
    assert_eq!(body["borrowable"], true);

    let (status, loan_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "candidate checkout: {loan_body}"
    );
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, renew_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/renew"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "candidate renew: {renew_body}");

    let (status, hold_body) = place_copy_hold(&app, &holder_token, holder_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "candidate hold: {hold_body}");
}

#[tokio::test]
async fn withdrawn_blocks_checkout_renew_and_new_holds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "wdout").await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "wdouth").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Withdrawn Blocks", 1).await;
    let item_id = item_ids[0];

    let (status, loan_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "pre-withdraw checkout: {loan_body}"
    );
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, body) = set_weeding(
        &app,
        &admin_token,
        item_id,
        "withdrawn",
        "last copy discarded",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "withdraw: {body}");
    assert_eq!(body["weedingStatus"], "withdrawn");
    assert!(
        body["archivedAt"].is_null(),
        "withdraw must not reuse archived_at: {body}"
    );

    let (status, renew_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/renew"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "withdrawn renew: {renew_body}"
    );
    assert!(
        renew_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains(WeedingStatus::RENEW_BLOCKED),
        "renew error: {renew_body}"
    );

    let (status, return_body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "return withdrawn loan: {return_body}"
    );

    let (status, checkout_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "withdrawn checkout: {checkout_body}"
    );
    assert!(
        checkout_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains(WeedingStatus::CHECKOUT_BLOCKED),
        "checkout error: {checkout_body}"
    );

    let (status, hold_body) = place_copy_hold(&app, &holder_token, holder_id, item_id).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "withdrawn copy hold: {hold_body}"
    );
    assert!(
        hold_body["message"]
            .as_str()
            .unwrap_or_default()
            .contains(WeedingStatus::HOLD_BLOCKED),
        "hold error: {hold_body}"
    );

    let (status, title_hold) = place_title_hold(&app, &holder_token, holder_id, biblio_id).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "title hold on fully withdrawn biblio: {title_hold}"
    );
    assert!(
        title_hold["message"]
            .as_str()
            .unwrap_or_default()
            .contains(WeedingStatus::TITLE_HOLD_BLOCKED),
        "title hold error: {title_hold}"
    );
}

#[tokio::test]
async fn withdraw_advances_hold_queue_and_keeps_title_holds_on_siblings() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_a, _) = fixtures::create_reader(&app, &admin_token, "wdqa").await;
    let (reader_b, token_b) = fixtures::create_reader(&app, &admin_token, "wdqb").await;
    let (reader_c, token_c) = fixtures::create_reader(&app, &admin_token, "wdqc").await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Withdraw Advances Queue", 2).await;
    let item_a = item_ids[0];
    let item_b = item_ids[1];

    let (status, loan_body) = checkout(&app, &admin_token, reader_a, item_a).await;
    assert_eq!(status, StatusCode::CREATED, "checkout A: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, hold_b) = place_title_hold(&app, &token_b, reader_b, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "title hold B: {hold_b}");
    let hold_b_id = fixtures::json_id(&hold_b["id"]);
    assert!(hold_b["itemId"].is_null());

    let (status, hold_c) = place_title_hold(&app, &token_c, reader_c, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "title hold C: {hold_c}");
    let hold_c_id = fixtures::json_id(&hold_c["id"]);
    assert!(hold_c["itemId"].is_null());

    let (status, ret) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return A: {ret}");

    let (status, queue) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "queue after return: {queue}");
    let rows = queue.as_array().expect("queue");
    let ready = rows
        .iter()
        .find(|h| fixtures::json_id(&h["id"]) == hold_b_id)
        .expect("B still in queue");
    assert_eq!(ready["status"], "ready");
    assert_eq!(fixtures::json_id(&ready["itemId"]), item_a);

    let (status, wd) = set_weeding(
        &app,
        &admin_token,
        item_a,
        "withdrawn",
        "withdraw ready copy",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "withdraw A: {wd}");
    assert!(wd["archivedAt"].is_null());

    let (status, after) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "queue after withdraw: {after}");
    let rows = after.as_array().expect("queue");
    assert!(
        rows.iter()
            .all(|h| fixtures::json_id(&h["id"]) != hold_b_id),
        "ready hold on withdrawn copy must leave the active queue: {after}"
    );
    assert!(
        rows.iter()
            .all(|h| h["itemId"].is_null() || fixtures::json_id(&h["itemId"]) != item_a),
        "no active hold may stay pinned to the withdrawn copy: {after}"
    );
    let promoted = rows
        .iter()
        .find(|h| fixtures::json_id(&h["id"]) == hold_c_id)
        .expect("title-level hold C must remain (sibling can fulfill)");
    assert_eq!(
        promoted["status"], "ready",
        "queue must advance onto the sibling: {after}"
    );
    assert_eq!(
        fixtures::json_id(&promoted["itemId"]),
        item_b,
        "next patron must be offered the remaining copy: {after}"
    );

    let (status, all_holds) = app
        .get_json_with_auth("/api/v1/holds?activeOnly=false&perPage=50", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "all holds: {all_holds}");
    let entries = all_holds["items"].as_array().expect("paginated items");
    let cancelled_b = entries
        .iter()
        .find(|h| fixtures::json_id(&h["id"]) == hold_b_id)
        .expect("cancelled hold B");
    assert_eq!(
        cancelled_b["status"], "cancelled",
        "B ready hold cancelled: {cancelled_b}"
    );
}

#[tokio::test]
async fn last_item_withdrawn_keeps_biblio_findable_and_unarchived() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (biblio_id, item_ids) =
        create_title_with_copies(&app, &admin_token, "Last Copy Withdrawn", 1).await;

    let (status, body) = set_weeding(
        &app,
        &admin_token,
        item_ids[0],
        "withdrawn",
        "collection weeding",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "withdraw last: {body}");
    assert!(body["archivedAt"].is_null());

    let (status, biblio) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "biblio after last withdraw: {biblio}"
    );
    assert!(
        biblio["archivedAt"].is_null(),
        "biblio must stay unarchived: {biblio}"
    );
    assert_eq!(biblio["weedingStatus"], "withdrawn");
    assert_eq!(biblio["items"][0]["weedingStatus"], "withdrawn");
    assert!(biblio["items"][0]["archivedAt"].is_null());
}

#[tokio::test]
async fn weeding_is_orthogonal_to_circulation_exceptions_and_archived_at() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "wdorth").await;
    let (_, item_ids) = create_title_with_copies(&app, &admin_token, "Orthogonal Weeding", 1).await;
    let item_id = item_ids[0];

    let (status, loan_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, lost_body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": false }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "mark lost: {lost_body}");

    let (status, wd) = set_weeding(
        &app,
        &admin_token,
        item_id,
        "candidate",
        "review after loss",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "candidate after lost: {wd}");
    assert_eq!(wd["weedingStatus"], "candidate");
    assert!(wd["archivedAt"].is_null());

    let copy = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(copy["weedingStatus"], "candidate");
    assert!(
        copy["archivedAt"].is_null(),
        "weeding must not set archived_at: {copy}"
    );
    assert_ne!(
        copy["circulationStatus"],
        json!(null),
        "circulation exception must remain: {copy}"
    );

    let (status, wd2) =
        set_weeding(&app, &admin_token, item_id, "withdrawn", "do not replace").await;
    assert_eq!(status, StatusCode::OK, "withdraw after lost: {wd2}");
    let copy = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(copy["weedingStatus"], "withdrawn");
    assert!(copy["archivedAt"].is_null());
}
