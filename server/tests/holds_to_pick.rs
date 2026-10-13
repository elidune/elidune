//! Title-hold shelf capture (`to_pick`) — B-01 / #87.

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

struct Copy {
    id: i64,
}

async fn create_title(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    copies: Vec<serde_json::Value>,
) -> (i64, Vec<Copy>) {
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": copies
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let copies = body["biblio"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| Copy {
            id: fixtures::json_id(&item["id"]),
        })
        .collect();
    (biblio_id, copies)
}

fn shelf_copy(barcode: &str) -> serde_json::Value {
    json!({
        "barcode": barcode,
        "borrowable": true,
        "callNumber": "843 MIS",
        "place": 12
    })
}

async fn place_title(
    app: &TestApp,
    token: &str,
    user_id: i64,
    biblio_id: i64,
    pickup_site_id: Option<i64>,
) -> serde_json::Value {
    let mut payload = json!({
        "userId": user_id.to_string(),
        "biblioId": biblio_id.to_string()
    });
    if let Some(site) = pickup_site_id {
        payload["pickupSiteId"] = json!(site.to_string());
    }
    let (status, body) = app.post_json("/api/v1/holds", &payload, Some(token)).await;
    assert_eq!(status, StatusCode::CREATED, "place title hold: {body}");
    body
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(admin_token),
    )
    .await
}

async fn user_email(app: &TestApp, user_id: i64) -> String {
    sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("email")
}

async fn outbox_count(app: &TestApp, email: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM email_outbox WHERE to_addr = $1")
        .bind(email)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("outbox count")
}

async fn hold_row(app: &TestApp, hold_id: i64) -> elidune_server::models::hold::Hold {
    app.state
        .services
        .repository
        .holds_get_by_id(hold_id)
        .await
        .expect("hold")
}

#[tokio::test]
async fn shelf_copy_is_to_pick_until_scan_then_ready_with_notice() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "picka").await;
    let email = user_email(&app, reader_id).await;
    let before = outbox_count(&app, &email).await;
    let barcode = format!("PICK-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Pick On Shelf",
        vec![shelf_copy(&barcode)],
    )
    .await;

    let hold = place_title(&app, &reader_token, reader_id, biblio_id, None).await;
    assert_eq!(hold["status"], "to_pick");
    assert_eq!(fixtures::json_id(&hold["itemId"]), copies[0].id);
    assert!(hold["notifiedAt"].is_null());
    assert_eq!(
        outbox_count(&app, &email).await,
        before,
        "placement must not notify"
    );

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "scan: {body}");
    assert_eq!(body["status"], "ready");
    assert!(body["loan"].is_null());
    assert_eq!(body["hold"]["status"], "ready");
    assert!(body["hold"]["notifiedAt"].is_string());
    assert!(body["hold"]["expiresAt"].is_string());

    let stored = hold_row(&app, fixtures::json_id(&hold["id"])).await;
    assert_eq!(
        stored.status,
        elidune_server::models::hold::HoldStatus::Ready
    );
    assert!(stored.notified_at.is_some());
    assert!(
        outbox_count(&app, &email).await > before,
        "check-in must queue the same hold-ready notice as a return"
    );
}

#[tokio::test]
async fn no_shelf_copy_stays_pending_until_return_readies_oldest_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (borrower, _) = fixtures::create_reader(&app, &admin_token, "pickbor").await;
    let (first_id, first_token) = fixtures::create_reader(&app, &admin_token, "pick1").await;
    let (second_id, second_token) = fixtures::create_reader(&app, &admin_token, "pick2").await;
    let barcode = format!("WAIT-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Nothing On Shelf",
        vec![shelf_copy(&barcode)],
    )
    .await;

    let (status, loan) = checkout(&app, &admin_token, borrower, copies[0].id, false).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

    let older = place_title(&app, &first_token, first_id, biblio_id, None).await;
    let newer = place_title(&app, &second_token, second_id, biblio_id, None).await;
    assert_eq!(older["status"], "pending");
    assert!(older["itemId"].is_null());
    assert_eq!(newer["status"], "pending");
    assert!(newer["itemId"].is_null());
    assert!(older["position"].as_i64().unwrap() < newer["position"].as_i64().unwrap());

    let first_email = user_email(&app, first_id).await;
    let second_email = user_email(&app, second_id).await;
    let first_before = outbox_count(&app, &first_email).await;
    let second_before = outbox_count(&app, &second_email).await;

    let (status, ret) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {ret}");

    let older_row = hold_row(&app, fixtures::json_id(&older["id"])).await;
    let newer_row = hold_row(&app, fixtures::json_id(&newer["id"])).await;
    assert_eq!(
        older_row.status,
        elidune_server::models::hold::HoldStatus::Ready
    );
    assert_eq!(older_row.item_id, Some(copies[0].id));
    assert!(older_row.notified_at.is_some());
    assert_eq!(
        newer_row.status,
        elidune_server::models::hold::HoldStatus::Pending
    );
    assert!(newer_row.item_id.is_none());
    assert!(newer_row.notified_at.is_none());
    assert!(outbox_count(&app, &first_email).await > first_before);
    assert_eq!(outbox_count(&app, &second_email).await, second_before);
}

#[tokio::test]
async fn unassigned_copies_stay_lendable_and_to_pick_copy_is_reserved() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "pickh").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "picko").await;
    let b1 = format!("FREE-{}", fixtures::unique_suffix());
    let b2 = format!("HELD-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Two Shelf Copies",
        vec![shelf_copy(&b1), shelf_copy(&b2)],
    )
    .await;

    let hold = place_title(&app, &holder_token, holder_id, biblio_id, None).await;
    assert_eq!(hold["status"], "to_pick");
    let reserved = fixtures::json_id(&hold["itemId"]);
    let free = copies
        .iter()
        .find(|c| c.id != reserved)
        .expect("other copy");

    let (status, loan) = checkout(&app, &admin_token, other_id, free.id, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unassigned copy stays lendable: {loan}"
    );

    let (status, blocked) = checkout(&app, &admin_token, other_id, reserved, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "reserved: {blocked}"
    );
    assert_eq!(blocked["code"], "item_to_pick_for_other", "{blocked}");
    assert_eq!(blocked["forceable"], false, "{blocked}");
    assert_eq!(
        fixtures::json_id(&blocked["details"]["holdId"]),
        fixtures::json_id(&hold["id"])
    );
    assert_eq!(fixtures::json_id(&blocked["details"]["userId"]), holder_id);
    assert!(blocked["details"].get("userName").is_none(), "{blocked}");
    assert!(blocked["details"]["cardNumber"].is_null(), "{blocked}");
    let initials = blocked["details"]["initials"].as_str().unwrap_or("");
    assert_eq!(initials.chars().count(), 2, "{blocked}");
    assert!(initials.starts_with('R'), "{blocked}");
    assert!(!blocked.to_string().contains("Reader"), "{blocked}");
}

#[tokio::test]
async fn force_does_not_check_out_a_copy_to_pick_for_another_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "relh").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "relo").await;

    let only = format!("ONLY-{}", fixtures::unique_suffix());
    let (biblio_only, copies) =
        create_title(&app, &admin_token, "Only Copy", vec![shelf_copy(&only)]).await;
    let hold = place_title(&app, &holder_token, holder_id, biblio_only, None).await;
    let hold_id = fixtures::json_id(&hold["id"]);
    let (status, forced) = checkout(&app, &admin_token, other_id, copies[0].id, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{forced}");
    assert_eq!(forced["code"], "item_to_pick_for_other", "{forced}");
    assert_eq!(forced["forceable"], false, "{forced}");
    let unchanged = hold_row(&app, hold_id).await;
    assert_eq!(
        unchanged.status,
        elidune_server::models::hold::HoldStatus::ToPick
    );
    assert_eq!(unchanged.item_id, Some(copies[0].id));

    let (holder2, token2) = fixtures::create_reader(&app, &admin_token, "relh2").await;
    let (other2, _) = fixtures::create_reader(&app, &admin_token, "relo2").await;
    let a = format!("REA-{}", fixtures::unique_suffix());
    let b = format!("REB-{}", fixtures::unique_suffix());
    let (biblio_two, _) = create_title(
        &app,
        &admin_token,
        "Reassign Copy",
        vec![shelf_copy(&a), shelf_copy(&b)],
    )
    .await;
    let hold = place_title(&app, &token2, holder2, biblio_two, None).await;
    let hold_id = fixtures::json_id(&hold["id"]);
    let reserved = fixtures::json_id(&hold["itemId"]);
    let (status, forced) = checkout(&app, &admin_token, other2, reserved, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{forced}");
    assert_eq!(forced["code"], "item_to_pick_for_other", "{forced}");
    assert_eq!(forced["forceable"], false, "{forced}");
    let still = hold_row(&app, hold_id).await;
    assert_eq!(
        still.status,
        elidune_server::models::hold::HoldStatus::ToPick
    );
    assert_eq!(still.item_id, Some(reserved));
}

#[tokio::test]
async fn pick_list_lists_to_pick_rows_and_filters_by_copy_site() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let here = create_site(&app, &admin_token, "Here").await;
    let elsewhere = create_site(&app, &admin_token, "Elsewhere").await;
    let pickup = create_site(&app, &admin_token, "Pickup").await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "listp").await;
    let barcode = format!("LIST-{}", fixtures::unique_suffix());
    let (biblio_id, _) = create_title(
        &app,
        &admin_token,
        "Pick List Title",
        vec![json!({
            "barcode": barcode,
            "borrowable": true,
            "callNumber": "843 MIS",
            "place": 12,
            "sourceId": here.to_string()
        })],
    )
    .await;

    let hold = place_title(&app, &reader_token, reader_id, biblio_id, Some(pickup)).await;
    assert_eq!(hold["status"], "to_pick");
    let hold_id = fixtures::json_id(&hold["id"]);

    let (status, denied) = app
        .get_json_with_auth("/api/v1/holds/pick-list", &reader_token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader: {denied}");

    let (status, list) = app
        .get_json_with_auth(
            &format!("/api/v1/holds/pick-list?siteId={here}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pick list: {list}");
    let rows = list.as_array().expect("array");
    let row = rows
        .iter()
        .find(|r| fixtures::json_id(&r["holdId"]) == hold_id)
        .expect("row");
    let mut keys: Vec<_> = row.as_object().expect("object").keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "barcode",
            "callNumber",
            "holdFor",
            "holdId",
            "itemId",
            "overdue",
            "pickDueAt",
            "pickStartedAt",
            "pickupDeadline",
            "pickupSiteId",
            "pickupSiteName",
            "title",
            "userId",
        ]
    );
    assert_eq!(row["barcode"], barcode);
    assert_eq!(row["callNumber"], "843 MIS");
    assert_eq!(row["title"], "Pick List Title");
    assert_eq!(fixtures::json_id(&row["userId"]), reader_id);
    assert_eq!(fixtures::json_id(&row["holdFor"]["id"]), reader_id);
    assert!(row["holdFor"]["cardNumber"].is_null(), "{row}");
    let initials = row["holdFor"]["initials"].as_str().unwrap_or("");
    assert_eq!(initials.chars().count(), 2, "{row}");
    assert!(initials.starts_with('R'), "{row}");
    assert!(row["holdFor"].get("firstname").is_none(), "{row}");
    assert!(row.get("userName").is_none(), "{row}");
    assert!(!row.to_string().contains("Reader"), "{row}");
    assert!(row["pickupDeadline"].is_null(), "{row}");
    assert_eq!(fixtures::json_id(&row["pickupSiteId"]), pickup);
    assert!(row["pickupSiteName"].is_string());
    assert!(row["pickStartedAt"].is_string());
    assert!(row["pickDueAt"].is_string());
    assert_eq!(row["overdue"], false);

    let (status, other) = app
        .get_json_with_auth(
            &format!("/api/v1/holds/pick-list?siteId={elsewhere}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "other site: {other}");
    let other_rows = other.as_array().expect("array");
    assert!(other_rows
        .iter()
        .all(|r| fixtures::json_id(&r["holdId"]) != hold_id));
}

#[tokio::test]
async fn scan_at_other_pickup_site_starts_transit_without_notice() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "OriginP").await;
    let pickup = create_site(&app, &admin_token, "DestP").await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "trnpk").await;
    let email = user_email(&app, reader_id).await;
    let barcode = format!("TRP-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Transit On Pick",
        vec![json!({
            "barcode": barcode,
            "borrowable": true,
            "sourceId": origin.to_string()
        })],
    )
    .await;

    let before = outbox_count(&app, &email).await;
    let hold = place_title(&app, &reader_token, reader_id, biblio_id, Some(pickup)).await;
    assert_eq!(hold["status"], "to_pick");
    assert_eq!(fixtures::json_id(&hold["itemId"]), copies[0].id);
    assert_eq!(outbox_count(&app, &email).await, before);

    let hold_id = fixtures::json_id(&hold["id"]);
    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "scan: {scan}");
    assert_eq!(scan["status"], "in_transit");
    assert_eq!(fixtures::json_id(&scan["destinationSiteId"]), pickup);
    assert!(scan["destinationSiteName"].is_string());
    assert_eq!(scan["hold"]["status"], "pending");
    assert!(scan["hold"]["notifiedAt"].is_null());
    assert_eq!(outbox_count(&app, &email).await, before);

    let (status, transit) = app
        .get_json_with_auth(&format!("/api/v1/holds/{hold_id}/transits"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "transit: {transit}");
    assert_eq!(transit["status"], "requested");
    assert_eq!(fixtures::json_id(&transit["fromSourceId"]), origin);
    assert_eq!(fixtures::json_id(&transit["toSourceId"]), pickup);
}

#[tokio::test]
async fn copy_not_yet_borrowable_is_not_captured() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "incp").await;
    let barcode = format!("INC-{}", fixtures::unique_suffix());
    let (biblio_id, _) = create_title(
        &app,
        &admin_token,
        "Incomplete Reception",
        vec![json!({ "barcode": barcode, "borrowable": false })],
    )
    .await;
    let hold = place_title(&app, &reader_token, reader_id, biblio_id, None).await;
    assert_eq!(hold["status"], "pending");
    assert!(hold["itemId"].is_null());
}

async fn item_flags(app: &TestApp, item_id: i64) -> (i16, bool) {
    let row: (Option<i16>, bool) =
        sqlx::query_as("SELECT circulation_status, borrowable FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("item");
    (row.0.unwrap_or(0), row.1)
}

#[tokio::test]
async fn concurrent_title_holds_assign_the_single_shelf_copy_once() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (first_id, _) = fixtures::create_reader(&app, &admin_token, "racep1").await;
    let (second_id, _) = fixtures::create_reader(&app, &admin_token, "racep2").await;
    let barcode = format!("RACE-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "One Shelf Race",
        vec![shelf_copy(&barcode)],
    )
    .await;
    let repo = app.state.services.repository.clone();
    let first = elidune_server::models::hold::CreateHold::for_biblio(first_id, biblio_id);
    let second = elidune_server::models::hold::CreateHold::for_biblio(second_id, biblio_id);
    let (r1, r2) = tokio::join!(repo.holds_create(&first), repo.holds_create(&second));
    let h1 = r1.expect("first place");
    let h2 = r2.expect("second place");
    let picks = [&h1, &h2]
        .iter()
        .filter(|h| h.status == elidune_server::models::hold::HoldStatus::ToPick)
        .count();
    let waiting = [&h1, &h2]
        .iter()
        .filter(|h| h.status == elidune_server::models::hold::HoldStatus::Pending)
        .count();
    assert_eq!(picks, 1, "exactly one to_pick: {h1:?} {h2:?}");
    assert_eq!(waiting, 1, "the other stays pending: {h1:?} {h2:?}");
    let assigned_item = h1.item_id.or(h2.item_id);
    assert_eq!(assigned_item, Some(copies[0].id));
    assert!(h1.item_id.is_none() || h2.item_id.is_none());
}

#[tokio::test]
async fn pick_not_found_reassigns_and_hides_the_missing_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "miss2").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "miss2o").await;
    let suffix = fixtures::unique_suffix();
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Two Copies Missing",
        vec![
            shelf_copy(&format!("MSA-{suffix}")),
            shelf_copy(&format!("MSB-{suffix}")),
        ],
    )
    .await;
    let hold = place_title(&app, &holder_token, holder_id, biblio_id, None).await;
    let hold_id = fixtures::json_id(&hold["id"]);
    let assigned = fixtures::json_id(&hold["itemId"]);
    let other = copies
        .iter()
        .find(|c| c.id != assigned)
        .expect("sibling")
        .id;

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/holds/{hold_id}/pick-not-found"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pick-not-found: {body}");
    assert_eq!(body["status"], "to_pick");
    assert_eq!(fixtures::json_id(&body["itemId"]), other);
    assert_ne!(fixtures::json_id(&body["itemId"]), assigned);

    let (circulation, borrowable) = item_flags(&app, assigned).await;
    assert_eq!(circulation, 4, "to_search");
    assert!(!borrowable);
    let (status, blocked) = checkout(&app, &admin_token, other_id, assigned, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "missing not lendable: {blocked}"
    );
    let (status, sibling) = checkout(&app, &admin_token, other_id, other, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "reassigned copy stays reserved: {sibling}"
    );
    assert_eq!(sibling["code"], "item_to_pick_for_other");

    let (only_id, only_token) = fixtures::create_reader(&app, &admin_token, "miss1").await;
    let (stranger, _) = fixtures::create_reader(&app, &admin_token, "miss1o").await;
    let only_barcode = format!("ONLYM-{suffix}");
    let (biblio_only, only_copies) = create_title(
        &app,
        &admin_token,
        "Only Copy Missing",
        vec![shelf_copy(&only_barcode)],
    )
    .await;
    let hold = place_title(&app, &only_token, only_id, biblio_only, None).await;
    let hold_id = fixtures::json_id(&hold["id"]);
    let item_id = only_copies[0].id;
    let (status, body) = app
        .post_empty(
            &format!("/api/v1/holds/{hold_id}/pick-not-found"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "only copy: {body}");
    assert_eq!(body["status"], "pending");
    assert!(body["itemId"].is_null());
    let (status, blocked) = checkout(&app, &admin_token, stranger, item_id, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "still not lendable: {blocked}"
    );

    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{only_barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "found scan: {scan}");
    assert_eq!(scan["status"], "unchanged", "{scan}");
    assert_eq!(
        scan["notice"]["code"], "item_not_available_status",
        "{scan}"
    );
    assert_eq!(scan["notice"]["level"], "warning", "{scan}");
    assert_eq!(scan["circulationStatus"], "toSearch", "{scan}");
    assert!(scan["hold"].is_null(), "{scan}");
    let (circulation, borrowable) = item_flags(&app, item_id).await;
    assert_eq!(circulation, 4, "return scan must not clear to_search");
    assert!(!borrowable);
    let stored = hold_row(&app, hold_id).await;
    assert_eq!(
        stored.status,
        elidune_server::models::hold::HoldStatus::Pending
    );
    assert_eq!(stored.item_id, None);
}

#[tokio::test]
async fn pick_list_overdue_follows_pick_delay_without_expiring() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "delayp").await;
    let suffix = fixtures::unique_suffix();
    let early = format!("AAA-{suffix}");
    let late = format!("ZZZ-{suffix}");
    let (biblio_a, _) = create_title(
        &app,
        &admin_token,
        "Delay A",
        vec![json!({ "barcode": early, "borrowable": true, "callNumber": early })],
    )
    .await;
    let (biblio_b, _) = create_title(
        &app,
        &admin_token,
        "Delay B",
        vec![json!({ "barcode": late, "borrowable": true, "callNumber": late })],
    )
    .await;
    let (other_id, other_token) = fixtures::create_reader(&app, &admin_token, "delayq").await;
    let older = place_title(&app, &reader_token, reader_id, biblio_a, None).await;
    let newer = place_title(&app, &other_token, other_id, biblio_b, None).await;
    let older_id = fixtures::json_id(&older["id"]);
    let newer_id = fixtures::json_id(&newer["id"]);
    sqlx::query("UPDATE holds SET pick_started_at = NOW() - INTERVAL '3 days' WHERE id = $1")
        .bind(older_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backdate");

    let (status, policy) = app
        .get_json_with_auth("/api/v1/holds/policy", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "policy: {policy}");
    assert_eq!(policy["pickDelayDays"], 2);

    let (status, list) = app
        .get_json_with_auth("/api/v1/holds/pick-list", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "list: {list}");
    let rows = list.as_array().expect("array");
    let ours: Vec<_> = rows
        .iter()
        .filter(|r| {
            let id = fixtures::json_id(&r["holdId"]);
            id == older_id || id == newer_id
        })
        .collect();
    assert_eq!(ours.len(), 2);
    assert_eq!(ours[0]["callNumber"], early);
    assert_eq!(ours[1]["callNumber"], late);
    let older_row = ours
        .iter()
        .find(|r| fixtures::json_id(&r["holdId"]) == older_id)
        .unwrap();
    let newer_row = ours
        .iter()
        .find(|r| fixtures::json_id(&r["holdId"]) == newer_id)
        .unwrap();
    assert_eq!(older_row["overdue"], true);
    assert_eq!(newer_row["overdue"], false);
    let stored = hold_row(&app, older_id).await;
    assert_eq!(
        stored.status,
        elidune_server::models::hold::HoldStatus::ToPick
    );
}

async fn mark_to_search(app: &TestApp, item_id: i64) {
    sqlx::query(
        "UPDATE items SET circulation_status = $2, borrowable = FALSE, updated_at = NOW() WHERE id = $1",
    )
    .bind(item_id)
    .bind(elidune_server::models::circulation::CirculationStatus::TO_SEARCH)
    .execute(app.state.services.repository.pool())
    .await
    .expect("mark to_search");
}

/// A `toSearch` copy is not lendable, including with `force`. A loan already
/// open when the copy is marked `toSearch` still clears that status on check-in (#115).
#[tokio::test]
async fn to_search_checkout_is_not_forceable_and_return_restores_lendable_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (borrower, _) = fixtures::create_reader(&app, &admin_token, "srchbor").await;
    let (next_id, _) = fixtures::create_reader(&app, &admin_token, "srchnext").await;
    let barcode = format!("SRCH-{}", fixtures::unique_suffix());
    let (_biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Missing Then Forced",
        vec![shelf_copy(&barcode)],
    )
    .await;
    let item_id = copies[0].id;
    mark_to_search(&app, item_id).await;

    for force in [false, true] {
        let (status, blocked) = checkout(&app, &admin_token, borrower, item_id, force).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "to_search is not lendable: {blocked}"
        );
        assert_eq!(blocked["code"], "item_status_action_required", "{blocked}");
        assert_eq!(blocked["itemStatus"], "toSearch", "{blocked}");
        assert_eq!(blocked["forceable"], false, "{blocked}");
    }

    sqlx::query(
        "UPDATE items SET circulation_status = 0, borrowable = TRUE, updated_at = NOW() WHERE id = $1",
    )
    .bind(item_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("restore shelf copy");
    let (status, loan) = checkout(&app, &admin_token, borrower, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "shelf loan: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    mark_to_search(&app, item_id).await;
    let (circulation, borrowable) = item_flags(&app, item_id).await;
    assert_eq!(circulation, 4);
    assert!(!borrowable);

    let (status, ret) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {ret}");
    assert_eq!(ret["status"], "returned");

    let (circulation, borrowable) = item_flags(&app, item_id).await;
    assert_eq!(circulation, 0, "physical return clears to_search");
    assert!(borrowable, "copy is lendable again");

    let (status, again) = checkout(&app, &admin_token, next_id, item_id, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "returned to_search copy circulates: {again}"
    );
}

/// A barcode return of a `toSearch` copy must not clear it or assign a hold.
/// Checkout of that copy stays refused, including with `force`.
#[tokio::test]
async fn to_search_barcode_return_does_not_assign_hold_and_force_checkout_stays_refused() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "srchh").await;
    let (borrower, _) = fixtures::create_reader(&app, &admin_token, "srchb2").await;
    let barcode = format!("SRCHH-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_title(
        &app,
        &admin_token,
        "Missing With Hold",
        vec![shelf_copy(&barcode)],
    )
    .await;
    let item_id = copies[0].id;

    let hold = place_title(&app, &holder_token, holder_id, biblio_id, None).await;
    assert_eq!(hold["status"], "to_pick");
    let hold_id = fixtures::json_id(&hold["id"]);
    let (status, missing) = app
        .post_empty(
            &format!("/api/v1/holds/{hold_id}/pick-not-found"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pick-not-found: {missing}");
    assert_eq!(missing["status"], "pending");
    assert!(missing["itemId"].is_null());
    let (circulation, borrowable) = item_flags(&app, item_id).await;
    assert_eq!(circulation, 4);
    assert!(!borrowable);

    let (status, refused) = checkout(&app, &admin_token, borrower, item_id, true).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_eq!(refused["code"], "item_status_action_required", "{refused}");
    assert_eq!(refused["itemStatus"], "toSearch", "{refused}");
    assert_eq!(refused["forceable"], false, "{refused}");
    let still = hold_row(&app, hold_id).await;
    assert_eq!(
        still.status,
        elidune_server::models::hold::HoldStatus::Pending,
        "refusing checkout leaves the unassigned hold pending"
    );

    sqlx::query(
        "UPDATE items SET circulation_status = 0, borrowable = TRUE, updated_at = NOW() WHERE id = $1",
    )
    .bind(item_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("restore shelf copy");
    let (status, loan) = checkout(&app, &admin_token, borrower, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "shelf loan: {loan}");
    mark_to_search(&app, item_id).await;
    let during = hold_row(&app, hold_id).await;
    assert_eq!(
        during.status,
        elidune_server::models::hold::HoldStatus::Pending
    );

    let (status, ret) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "check-in: {ret}");
    assert_eq!(ret["status"], "unchanged", "{ret}");
    assert_eq!(ret["notice"]["code"], "item_not_available_status", "{ret}");
    assert_eq!(ret["notice"]["level"], "warning", "{ret}");
    assert_eq!(ret["circulationStatus"], "toSearch", "{ret}");
    assert!(ret["hold"].is_null(), "{ret}");
    assert!(ret["loan"].is_null(), "{ret}");

    let (circulation, borrowable) = item_flags(&app, item_id).await;
    assert_eq!(circulation, 4, "barcode return must not clear to_search");
    assert!(!borrowable);
    let stored = hold_row(&app, hold_id).await;
    assert_eq!(
        stored.status,
        elidune_server::models::hold::HoldStatus::Pending
    );
    assert_eq!(stored.item_id, None);
    assert!(stored.notified_at.is_none());

    let still_out: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM loans WHERE item_id = $1 AND returned_at IS NULL",
    )
    .bind(item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("open loan");
    assert_eq!(still_out, 1, "the scan must not close the loan");
}
