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
    let _ = app.state.services.users.set_must_change_password(admin_id, false).await;
}

async fn create_title_with_copies(app: &TestApp, admin_token: &str, title: &str, copy_count: usize) -> (i64, Vec<i64>) {
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
    let (status, body) = app.post_json("/api/v1/biblios", &payload, Some(admin_token)).await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_ids: Vec<i64> = body["biblio"]["items"].as_array().expect("items").iter().map(|item| fixtures::json_id(&item["id"])).collect();
    (biblio_id, item_ids)
}

async fn set_weeding(app: &TestApp, admin_token: &str, item_id: i64, status: &str, reason: &str) -> (StatusCode, serde_json::Value) {
    app.post_json(&format!("/api/v1/items/{item_id}/weeding"), &json!({ "status": status, "reason": reason }), Some(admin_token))
        .await
}

async fn withdraw_copy(app: &TestApp, admin_token: &str, item_id: i64, reason: &str) -> (StatusCode, serde_json::Value) {
    app.post_json(&format!("/api/v1/items/{item_id}/withdrawals"), &json!({ "reason": reason, "destination": "pulp" }), Some(admin_token))
        .await
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> (StatusCode, serde_json::Value) {
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

async fn place_copy_hold(app: &TestApp, token: &str, user_id: i64, item_id: i64) -> (StatusCode, serde_json::Value) {
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

async fn place_title_hold(app: &TestApp, token: &str, user_id: i64, biblio_id: i64) -> (StatusCode, serde_json::Value) {
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
    let (status, body) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}"), admin_token).await;
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
    let (_, item_ids) = create_title_with_copies(&app, &admin_token, "Candidate Circulates", 1).await;
    let item_id = item_ids[0];

    let (status, body) = set_weeding(&app, &admin_token, item_id, "candidate", "worn but usable").await;
    assert_eq!(status, StatusCode::OK, "mark candidate: {body}");
    assert_eq!(body["weedingStatus"], "candidate");
    assert_eq!(body["weedingReason"], "worn but usable");
    assert!(body["archivedAt"].is_null(), "candidate must not set archived_at: {body}");
    assert_eq!(body["borrowable"], true);

    let (status, loan_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "candidate checkout: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);
    fixtures::pull_loan_into_renewal_window(&app, loan_id).await;

    let (status, renew_body) = app.post_json(&format!("/api/v1/loans/{loan_id}/renew"), &json!({}), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "candidate renew: {renew_body}");

    let (status, hold_body) = place_copy_hold(&app, &holder_token, holder_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "candidate hold: {hold_body}");
}

#[tokio::test]
async fn weeding_candidates_list_returns_marked_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, item_ids) = create_title_with_copies(&app, &admin_token, "List Candidates", 2).await;
    let item_id = item_ids[0];

    let (status, empty) = app.get_json_with_auth("/api/v1/weeding-candidates", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "list before candidate: {empty}");

    let (status, body) = set_weeding(&app, &admin_token, item_id, "candidate", "review shelf").await;
    assert_eq!(status, StatusCode::OK, "mark candidate: {body}");

    let (status, list) = app.get_json_with_auth("/api/v1/weeding-candidates", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "list candidates: {list}");
    assert_eq!(list["total"], 1);
    let row = &list["items"][0];
    assert_eq!(row["itemId"], item_id.to_string());
    assert_eq!(row["weedingReason"], "review shelf");
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
    let (biblio_id, item_ids) = create_title_with_copies(&app, &admin_token, "Withdrawn Blocks", 1).await;
    let item_id = item_ids[0];

    let (status, loan_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "pre-withdraw checkout: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, refused) = set_weeding(&app, &admin_token, item_id, "withdrawn", "last copy discarded").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "old withdraw: {refused}");
    assert_eq!(refused["code"], "withdrawn_via_withdrawal");

    let (status, on_loan) = withdraw_copy(&app, &admin_token, item_id, "worn").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "on loan: {on_loan}");
    assert_eq!(on_loan["code"], "item_on_loan");

    let (status, return_body) = app.post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "return before withdraw: {return_body}");

    let (status, body) = withdraw_copy(&app, &admin_token, item_id, "worn").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {body}");
    let copy = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(copy["weedingStatus"], "withdrawn");
    assert!(copy["archivedAt"].is_null(), "withdraw must not reuse archived_at: {copy}");
    assert!(copy["weedingReason"].is_null());

    let (status, checkout_body) = checkout(&app, &admin_token, reader_id, item_id).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "withdrawn checkout: {checkout_body}");
    assert!(
        checkout_body["message"].as_str().unwrap_or_default().contains(WeedingStatus::CHECKOUT_BLOCKED),
        "checkout error: {checkout_body}"
    );

    let (status, hold_body) = place_copy_hold(&app, &holder_token, holder_id, item_id).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "withdrawn copy hold: {hold_body}");
    assert!(hold_body["message"].as_str().unwrap_or_default().contains(WeedingStatus::HOLD_BLOCKED), "hold error: {hold_body}");

    let (status, title_hold) = place_title_hold(&app, &holder_token, holder_id, biblio_id).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "title hold on fully withdrawn biblio: {title_hold}");
    assert!(
        title_hold["message"].as_str().unwrap_or_default().contains(WeedingStatus::TITLE_HOLD_BLOCKED),
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
    let (biblio_id, item_ids) = create_title_with_copies(&app, &admin_token, "Withdraw Advances Queue", 2).await;
    let item_a = item_ids[0];
    let item_b = item_ids[1];

    let (status, loan_body) = checkout(&app, &admin_token, reader_a, item_a).await;
    assert_eq!(status, StatusCode::CREATED, "checkout A: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);

    let (status, pinned) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_b.to_string(),
                "itemId": item_a.to_string()
            }),
            Some(&token_b),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "pinned hold: {pinned}");
    let pinned_id = fixtures::json_id(&pinned["id"]);

    let (status, title_hold) = place_title_hold(&app, &token_c, reader_c, biblio_id).await;
    assert_eq!(status, StatusCode::CREATED, "title hold: {title_hold}");
    let title_id = fixtures::json_id(&title_hold["id"]);
    assert_eq!(title_hold["status"], "to_pick");
    assert_eq!(fixtures::json_id(&title_hold["itemId"]), item_b, "title hold captures the shelf sibling");

    let (status, ret) = app.post_empty(&format!("/api/v1/loans/{loan_id}/return"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "return A: {ret}");

    let (status, blocked) = withdraw_copy(&app, &admin_token, item_a, "surplus").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ready hold blocks withdraw: {blocked}");
    assert_eq!(blocked["code"], "item_on_hold");

    sqlx::query("UPDATE holds SET status = 'pending' WHERE id = $1")
        .bind(pinned_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("pending hold still points at the copy");

    let (status, wd) = withdraw_copy(&app, &admin_token, item_a, "surplus").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw A: {wd}");
    let copy = item_copy(&app, &admin_token, item_a).await;
    assert!(copy["archivedAt"].is_null());

    let (status, after) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "queue after withdraw: {after}");
    let rows = after.as_array().expect("queue");
    assert!(
        rows.iter().all(|h| fixtures::json_id(&h["id"]) != pinned_id),
        "ready hold on withdrawn copy must leave the active queue: {after}"
    );
    assert!(
        rows.iter().all(|h| h["itemId"].is_null() || fixtures::json_id(&h["itemId"]) != item_a),
        "no active hold may stay pinned to the withdrawn copy: {after}"
    );
    let kept = rows.iter().find(|h| fixtures::json_id(&h["id"]) == title_id).expect("title hold on the sibling must remain");
    assert_eq!(kept["status"], "to_pick");
    assert_eq!(fixtures::json_id(&kept["itemId"]), item_b);

    let cancelled = app.state.services.repository.holds_get_by_id(pinned_id).await.expect("cancelled pinned hold");
    assert_eq!(cancelled.status, elidune_server::models::hold::HoldStatus::Cancelled);
}

#[tokio::test]
async fn last_item_withdrawn_keeps_biblio_findable_and_unarchived() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "wdlast").await;
    let title = format!("Last Copy Withdrawn {}", fixtures::unique_suffix());
    let (biblio_id, item_ids) = create_title_with_copies(&app, &admin_token, &title, 1).await;

    let (status, body) = withdraw_copy(&app, &admin_token, item_ids[0], "obsolete").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw last: {body}");
    let copy = item_copy(&app, &admin_token, item_ids[0]).await;
    assert!(copy["archivedAt"].is_null());

    let (status, biblio) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "biblio after last withdraw: {biblio}");
    assert!(biblio["archivedAt"].is_null(), "biblio must stay unarchived: {biblio}");
    assert_eq!(biblio["weedingStatus"], "withdrawn");
    assert_eq!(biblio["items"][0]["weedingStatus"], "withdrawn");
    assert!(biblio["items"][0]["archivedAt"].is_null());

    let (status, opac) = app.get_json(&format!("/api/v1/opac/biblios/{biblio_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "opac hides a fully withdrawn record: {opac}");
    assert_eq!(opac["code"], "not_found");

    let (status, reader) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &reader_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "reader catalog: {reader}");

    let encoded = title.replace(' ', "%20");
    let (status, staff_search) = app.get_json_with_auth(&format!("/api/v1/biblios?title={encoded}&perPage=5"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "staff search: {staff_search}");
    assert!(
        staff_search["items"].as_array().expect("hits").iter().any(|row| fixtures::json_id(&row["id"]) == biblio_id),
        "staff still find a fully withdrawn record: {staff_search}"
    );
    let (status, opac_search) = app.get_json(&format!("/api/v1/opac/biblios?title={encoded}&perPage=5")).await;
    assert_eq!(status, StatusCode::OK, "opac search: {opac_search}");
    assert!(
        opac_search["items"].as_array().expect("opac hits").iter().all(|row| fixtures::json_id(&row["id"]) != biblio_id),
        "opac search omits a fully withdrawn record: {opac_search}"
    );
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

    let (status, lost_body) = app.post_json(&format!("/api/v1/loans/{loan_id}/lost"), &json!({ "bill": false }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "mark lost: {lost_body}");

    let (status, wd) = set_weeding(&app, &admin_token, item_id, "candidate", "review after loss").await;
    assert_eq!(status, StatusCode::OK, "candidate after lost: {wd}");
    assert_eq!(wd["weedingStatus"], "candidate");
    assert!(wd["archivedAt"].is_null());

    let copy = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(copy["weedingStatus"], "candidate");
    assert!(copy["archivedAt"].is_null(), "weeding must not set archived_at: {copy}");
    assert_ne!(copy["circulationStatus"], json!(null), "circulation exception must remain: {copy}");

    let (status, wd2) = withdraw_copy(&app, &admin_token, item_id, "lostNotReplaced").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw after lost: {wd2}");
    assert_eq!(wd2["reason"], "lostNotReplaced");
    let copy = item_copy(&app, &admin_token, item_id).await;
    assert_eq!(copy["weedingStatus"], "withdrawn");
    assert!(copy["archivedAt"].is_null());
    assert_ne!(copy["circulationStatus"], json!(null));
}

#[tokio::test]
async fn readers_and_opac_do_not_see_weeding_reason() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "wdhide").await;
    let (biblio_id, item_ids) = create_title_with_copies(&app, &admin_token, "Hidden Weeding", 1).await;
    let item_id = item_ids[0];

    let (status, body) = set_weeding(&app, &admin_token, item_id, "candidate", "internal discard note").await;
    assert_eq!(status, StatusCode::OK, "mark candidate: {body}");
    sqlx::query("UPDATE items SET internal_note = 'staff copy note' WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set copy notes");

    let (status, staff) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "staff biblio: {staff}");
    assert_eq!(staff["items"][0]["weedingReason"], "internal discard note");
    assert_eq!(staff["items"][0]["weedingStatus"], "candidate");
    assert_eq!(staff["items"][0]["internalNote"], "staff copy note");
    assert!(staff["items"][0].get("notes").is_none(), "{staff}");
    assert!(staff.get("weedingStatus").is_some());

    let (status, reader) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "reader biblio: {reader}");
    assert!(
        reader["items"].as_array().expect("items").is_empty(),
        "a weeding candidate is omitted from the patron catalog: {reader}"
    );
    assert!(!reader.to_string().contains("internal discard note"));
    assert!(!reader.to_string().contains("staff copy note"));
    assert!(reader.get("weedingStatus").is_none());
    assert!(reader.get("weedingReason").is_none());

    let (status, opac) = app.get_json(&format!("/api/v1/opac/biblios/{biblio_id}")).await;
    assert_eq!(status, StatusCode::OK, "opac biblio: {opac}");
    assert!(opac["items"].as_array().expect("items").is_empty(), "a weeding candidate is omitted from the OPAC: {opac}");
    assert!(!opac.to_string().contains("internal discard note"));
    assert!(!opac.to_string().contains("staff copy note"));
    assert!(opac.get("weedingStatus").is_none());
}
