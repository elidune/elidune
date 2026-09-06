//! Inter-site hold transit: ship / receive / checkout block / cancel-in-transit.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

async fn create_site(app: &TestApp, admin_token: &str, name: &str) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/sources",
            &json!({ "name": format!("{name}-{}", fixtures::unique_suffix()) }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create site: {body}");
    fixtures::json_id(&body["id"])
}

async fn create_copy_at_site(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    site_id: i64,
) -> (i64, i64) {
    let payload = json!({
        "title": title,
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": format!("TRN-{}", fixtures::unique_suffix()),
            "borrowable": true,
            "sourceId": site_id.to_string()
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (biblio_id, item_id)
}

async fn place_title_hold(
    app: &TestApp,
    token: &str,
    user_id: i64,
    biblio_id: i64,
    pickup_site_id: i64,
) -> serde_json::Value {
    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": user_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "pickupSiteId": pickup_site_id.to_string()
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "place hold: {body}");
    body
}

async fn ship_hold(
    app: &TestApp,
    admin_token: &str,
    hold_id: i64,
    item_id: i64,
) -> serde_json::Value {
    let (status, body) = app
        .post_json(
            &format!("/api/v1/holds/{hold_id}/transits"),
            &json!({
                "itemId": item_id.to_string(),
                "ship": true
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "ship: {body}");
    body
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

async fn audit_has(app: &TestApp, admin_token: &str, event_type: &str) -> bool {
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/audit?eventType={event_type}&perPage=50"),
            admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "audit {event_type}: {body}");
    body["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|row| row["eventType"] == event_type)
}

#[tokio::test]
async fn pickup_site_is_stored_on_title_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let pickup = create_site(&app, &admin_token, "Pickup").await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "trnpickup").await;
    let (biblio_id, _) = create_copy_at_site(&app, &admin_token, "Transit Pickup", pickup).await;

    let hold = place_title_hold(&app, &reader_token, reader_id, biblio_id, pickup).await;
    assert_eq!(fixtures::json_id(&hold["pickupSiteId"]), pickup);
    assert!(hold["itemId"].is_null());
}

#[tokio::test]
async fn happy_path_ship_receive_marks_hold_ready() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "Origin").await;
    let pickup = create_site(&app, &admin_token, "Dest").await;
    let (patron_id, patron_token) = fixtures::create_reader(&app, &admin_token, "trnhappy").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "trnwrong").await;
    let (biblio_id, item_id) =
        create_copy_at_site(&app, &admin_token, "Transit Happy Path", origin).await;

    let hold = place_title_hold(&app, &patron_token, patron_id, biblio_id, pickup).await;
    let hold_id = fixtures::json_id(&hold["id"]);

    let shipped = ship_hold(&app, &admin_token, hold_id, item_id).await;
    assert_eq!(shipped["transit"]["status"], "inTransit");
    assert_eq!(
        fixtures::json_id(&shipped["transit"]["fromSourceId"]),
        origin
    );
    assert_eq!(fixtures::json_id(&shipped["transit"]["toSourceId"]), pickup);
    assert_eq!(shipped["hold"]["status"], "pending");
    assert_eq!(fixtures::json_id(&shipped["hold"]["itemId"]), item_id);
    let transit_id = fixtures::json_id(&shipped["transit"]["id"]);

    let (status, body) = checkout(&app, &admin_token, other_id, item_id).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blocked: {body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("transit"),
        "expected transit block: {body}"
    );

    let (status, received) = app
        .post_json(
            &format!("/api/v1/transits/{transit_id}/receive"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "receive: {received}");
    assert_eq!(received["transit"]["status"], "received");
    assert_eq!(received["hold"]["status"], "ready");
    assert!(received["hold"]["expiresAt"].is_string());

    let (status, items) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "items: {items}");
    let moved = items
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| fixtures::json_id(&i["id"]) == item_id)
        .expect("item");
    assert_eq!(fixtures::json_id(&moved["sourceId"]), pickup);

    let (status, body) = checkout(&app, &admin_token, patron_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "pickup checkout: {body}");

    assert!(audit_has(&app, &admin_token, "transit.shipped").await);
    assert!(audit_has(&app, &admin_token, "transit.received").await);
    assert!(audit_has(&app, &admin_token, "hold.ready").await);
}

#[tokio::test]
async fn cancel_while_in_transit_frees_hold_and_reverses() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "OriginC").await;
    let pickup = create_site(&app, &admin_token, "DestC").await;
    let (patron_id, patron_token) = fixtures::create_reader(&app, &admin_token, "trncan").await;
    let (biblio_id, item_id) =
        create_copy_at_site(&app, &admin_token, "Transit Cancel", origin).await;

    let hold = place_title_hold(&app, &patron_token, patron_id, biblio_id, pickup).await;
    let hold_id = fixtures::json_id(&hold["id"]);
    let shipped = ship_hold(&app, &admin_token, hold_id, item_id).await;
    let transit_id = fixtures::json_id(&shipped["transit"]["id"]);

    let (status, cancelled) = app
        .delete_with_auth(&format!("/api/v1/holds/{hold_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "cancel hold: {cancelled}");
    assert_eq!(cancelled["status"], "cancelled");

    let (status, original) = app
        .get_json_with_auth(&format!("/api/v1/transits/{transit_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "original transit: {original}");
    assert_eq!(original["transit"]["status"], "cancelled");

    let (status, list) = app
        .get_json_with_auth(
            &format!("/api/v1/transits?itemId={item_id}&perPage=20"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "list transits: {list}");
    let reverse = list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["status"] == "requested" && t["reversedFromId"].is_string())
        .expect("reverse transit");
    assert_eq!(fixtures::json_id(&reverse["fromSourceId"]), pickup);
    assert_eq!(fixtures::json_id(&reverse["toSourceId"]), origin);
    assert!(reverse["holdId"].is_null());

    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "trnfree").await;
    let (status, body) = checkout(&app, &admin_token, other_id, item_id).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "reverse requested still blocks: {body}"
    );
}

#[tokio::test]
async fn loan_return_requests_transit_instead_of_ready() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "OriginR").await;
    let pickup = create_site(&app, &admin_token, "DestR").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "trnbor").await;
    let (patron_id, patron_token) = fixtures::create_reader(&app, &admin_token, "trnwait").await;
    let (biblio_id, item_id) =
        create_copy_at_site(&app, &admin_token, "Transit On Return", origin).await;

    let (status, loan) = checkout(&app, &admin_token, borrower_id, item_id).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

    let hold = place_title_hold(&app, &patron_token, patron_id, biblio_id, pickup).await;
    let hold_id = fixtures::json_id(&hold["id"]);

    let (status, ret) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {ret}");

    let (status, holds) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "holds: {holds}");
    let hold_row = holds
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| fixtures::json_id(&h["id"]) == hold_id)
        .expect("hold");
    assert_eq!(
        hold_row["status"], "pending",
        "must not be ready yet: {hold_row}"
    );
    assert_eq!(fixtures::json_id(&hold_row["itemId"]), item_id);

    let (status, transit) = app
        .get_json_with_auth(&format!("/api/v1/holds/{hold_id}/transits"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "active transit: {transit}");
    assert_eq!(transit["status"], "requested");
    assert_eq!(fixtures::json_id(&transit["toSourceId"]), pickup);
}

#[tokio::test]
async fn transit_destination_must_match_hold_pickup_site() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "OriginL").await;
    let pickup = create_site(&app, &admin_token, "PickupL").await;
    let other = create_site(&app, &admin_token, "OtherL").await;
    let (patron_id, patron_token) = fixtures::create_reader(&app, &admin_token, "trnlock").await;
    let (biblio_id, item_id) =
        create_copy_at_site(&app, &admin_token, "Transit Pickup Lock", origin).await;

    let hold = place_title_hold(&app, &patron_token, patron_id, biblio_id, pickup).await;
    let hold_id = fixtures::json_id(&hold["id"]);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/holds/{hold_id}/transits"),
            &json!({
                "itemId": item_id.to_string(),
                "toSourceId": other.to_string(),
                "ship": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "mismatch: {body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("pickupSiteId"),
        "expected pickup lock: {body}"
    );
}
