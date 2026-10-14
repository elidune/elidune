//! Pending unassigned title holds block checkout when shelf copies would not cover them.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn next_id() -> i64 {
    static N: AtomicI64 = AtomicI64::new(1);
    let tick = fixtures::unique_suffix() as i64;
    tick.wrapping_add(N.fetch_add(1, Ordering::Relaxed))
}

async fn create_copies(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    barcodes: &[&str],
) -> (i64, Vec<i64>) {
    let items: Vec<Value> = barcodes
        .iter()
        .map(|barcode| json!({ "barcode": barcode, "borrowable": true }))
        .collect();
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": items
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_ids = body["biblio"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| fixtures::json_id(&item["id"]))
        .collect();
    (biblio_id, item_ids)
}

async fn pending_hold(
    app: &TestApp,
    user_id: i64,
    biblio_id: i64,
    position: i32,
    pickup_site_id: Option<i64>,
) -> i64 {
    let hold_id = next_id();
    sqlx::query(
        r#"
        INSERT INTO holds (id, user_id, biblio_id, position, status, pickup_site_id)
        VALUES ($1, $2, $3, $4, 'pending', $5)
        "#,
    )
    .bind(hold_id)
    .bind(user_id)
    .bind(biblio_id)
    .bind(position)
    .bind(pickup_site_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("pending title hold");
    hold_id
}

fn checkout_body(user_id: i64, item_id: i64, force: bool, reason: Option<&str>) -> Value {
    let mut payload = json!({
        "userId": user_id.to_string(),
        "itemId": item_id.to_string(),
        "force": force
    });
    if let Some(reason) = reason {
        payload["overrideReason"] = json!(reason);
    }
    payload
}

async fn checkout(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
    reason: Option<&str>,
) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/loans",
        &checkout_body(user_id, item_id, force, reason),
        Some(token),
    )
    .await
}

fn assert_held_body(body: &Value, hold_id: i64, forceable: bool) {
    assert_eq!(body["code"], "item_held_for_other", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert_eq!(body["forceable"], forceable, "{body}");
    if forceable {
        assert_eq!(body["reasonRequired"], true, "{body}");
    } else {
        assert!(body.get("reasonRequired").is_none(), "{body}");
    }
    let mut keys: Vec<_> = body["details"]
        .as_object()
        .expect("details")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, ["holdId"], "{body}");
    assert_eq!(fixtures::json_id(&body["details"]["holdId"]), hold_id);
    assert!(body.get("itemStatus").is_none(), "{body}");
}

async fn hold_row(app: &TestApp, hold_id: i64) -> (String, i32, Option<i64>) {
    sqlx::query_as("SELECT status, position, item_id FROM holds WHERE id = $1")
        .bind(hold_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("hold row")
}

async fn staff_without_override(app: &TestApp, admin_token: &str) -> String {
    let code = format!("desk{}", fixtures::unique_suffix() % 1_000_000_000);
    let (status, body) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": "Desk no override",
                "loansRights": "w",
                "settingsRights": "n",
                "circulationOverrideRights": "n"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "account type: {body}");
    let login = format!("desk_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Desk",
                "lastname": "Only",
                "email": format!("{login}@test.local"),
                "accountType": code,
                "publicType": public_type_id.to_string(),
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "staff: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "login: {body}");
    body["token"].as_str().expect("token").to_string()
}

async fn create_site(app: &TestApp, admin_token: &str, name: &str) -> i64 {
    let (status, body) = app
        .post_json(
            "/api/v1/sources",
            &json!({ "name": format!("{name}-{}", fixtures::unique_suffix()) }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "site: {body}");
    fixtures::json_id(&body["id"])
}

#[tokio::test]
async fn other_reader_pending_hold_refuses_the_only_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ttlonlyh").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttlonlyb").await;
    let barcode = format!("ONLY-{}", fixtures::unique_suffix());
    let (biblio_id, items) = create_copies(&app, &admin_token, "Only Copy Held", &[&barcode]).await;
    let hold_id = pending_hold(&app, holder_id, biblio_id, 1, None).await;

    let (status, body) = checkout(&app, &admin_token, borrower_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_held_body(&body, hold_id, true);

    let desk = staff_without_override(&app, &admin_token).await;
    let (status, denied) = checkout(
        &app,
        &desk,
        borrower_id,
        items[0],
        true,
        Some("desk cannot force a title hold"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{denied}");
    assert_held_body(&denied, hold_id, false);

    let (status, batch) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": borrower_id.to_string(),
                "barcodes": [barcode],
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    assert_eq!(batch["created"], 0, "{batch}");
    assert_eq!(batch["results"][0]["success"], false, "{batch}");
    assert_eq!(
        batch["results"][0]["code"], "item_held_for_other",
        "{batch}"
    );

    let (status, forced) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": borrower_id.to_string(),
                "barcodes": [barcode],
                "force": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{forced}");
    assert_eq!(forced["created"], 0, "{forced}");
    assert_eq!(forced["results"][0]["success"], false, "{forced}");
    assert_eq!(
        forced["results"][0]["code"], "override_reason_required",
        "{forced}"
    );
    let (hold_status, position, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(hold_status, "pending");
    assert_eq!(position, 1);
    assert_eq!(assigned, None);
}

#[tokio::test]
async fn second_available_copy_lets_the_loan_proceed() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ttlsech").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttlsecb").await;
    let (biblio_id, items) = create_copies(
        &app,
        &admin_token,
        "Two Shelf Copies",
        &[
            &format!("SEC-A-{}", fixtures::unique_suffix()),
            &format!("SEC-B-{}", fixtures::unique_suffix()),
        ],
    )
    .await;
    let hold_id = pending_hold(&app, holder_id, biblio_id, 1, None).await;

    let (status, created) = checkout(&app, &admin_token, borrower_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (hold_status, position, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(hold_status, "pending");
    assert_eq!(position, 1);
    assert_eq!(assigned, None);
}

#[tokio::test]
async fn ineligible_sibling_does_not_cover_the_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ttlnoth").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttlnotb").await;
    let (biblio_id, items) = create_copies(
        &app,
        &admin_token,
        "Not Loanable Sibling",
        &[
            &format!("NL-A-{}", fixtures::unique_suffix()),
            &format!("NL-B-{}", fixtures::unique_suffix()),
        ],
    )
    .await;
    sqlx::query("UPDATE items SET borrowable = FALSE WHERE id = $1")
        .bind(items[1])
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark not loanable");
    let hold_id = pending_hold(&app, holder_id, biblio_id, 1, None).await;

    let (status, body) = checkout(&app, &admin_token, borrower_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_held_body(&body, hold_id, true);

    let home = create_site(&app, &admin_token, "home").await;
    let away = create_site(&app, &admin_token, "away").await;
    let (transit_biblio, transit_items) = create_copies(
        &app,
        &admin_token,
        "In Transit Sibling",
        &[
            &format!("TR-A-{}", fixtures::unique_suffix()),
            &format!("TR-B-{}", fixtures::unique_suffix()),
        ],
    )
    .await;
    sqlx::query("UPDATE items SET source_id = $2 WHERE id = $1")
        .bind(transit_items[1])
        .bind(home)
        .execute(app.state.services.repository.pool())
        .await
        .expect("site");
    sqlx::query(
        r#"
        INSERT INTO item_transits (id, item_id, from_source_id, to_source_id, status)
        VALUES ($1, $2, $3, $4, 'in_transit')
        "#,
    )
    .bind(next_id())
    .bind(transit_items[1])
    .bind(home)
    .bind(away)
    .execute(app.state.services.repository.pool())
    .await
    .expect("transit");
    let transit_hold = pending_hold(&app, holder_id, transit_biblio, 1, None).await;
    let (status, body) = checkout(
        &app,
        &admin_token,
        borrower_id,
        transit_items[0],
        false,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_held_body(&body, transit_hold, true);
}

#[tokio::test]
async fn borrower_who_holds_the_title_fulfils_that_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ttlownh").await;
    let barcode = format!("OWN-{}", fixtures::unique_suffix());
    let (biblio_id, items) =
        create_copies(&app, &admin_token, "Holder Checkout", &[&barcode]).await;
    let hold_id = pending_hold(&app, holder_id, biblio_id, 1, None).await;

    let (status, created) = checkout(&app, &admin_token, holder_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (hold_status, _, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(hold_status, "fulfilled");
    assert_eq!(assigned, Some(items[0]));
}

#[tokio::test]
async fn forced_loan_keeps_the_unassigned_hold_first_and_audits() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (me_status, me) = app
        .get_json_with_auth("/api/v1/auth/me", &admin_token)
        .await;
    assert_eq!(me_status, StatusCode::OK, "{me}");
    let staff_id = fixtures::json_id(&me["id"]);
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "ttlforh").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttlforb").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "ttlforo").await;
    let barcode = format!("FOR-{}", fixtures::unique_suffix());
    let (biblio_id, items) =
        create_copies(&app, &admin_token, "Force Title Hold", &[&barcode]).await;
    let hold_id = pending_hold(&app, holder_id, biblio_id, 1, None).await;
    let behind_id = pending_hold(&app, other_id, biblio_id, 2, None).await;

    let (status, created) = checkout(
        &app,
        &admin_token,
        borrower_id,
        items[0],
        true,
        Some("reader is in the room"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);
    let (hold_status, position, assigned) = hold_row(&app, hold_id).await;
    assert_eq!(hold_status, "pending");
    assert_eq!(position, 1);
    assert_eq!(assigned, None);
    let (behind_status, behind_position, behind_item) = hold_row(&app, behind_id).await;
    assert_eq!(behind_status, "pending");
    assert_eq!(behind_position, 2);
    assert_eq!(behind_item, None);

    let pool = app.state.services.repository.pool();
    let mut payload = None;
    let mut actor = None;
    for _ in 0..40 {
        let row: Option<(Option<i64>, Value)> = sqlx::query_as(
            r#"
            SELECT user_id, payload
            FROM audit_log
            WHERE event_type = 'loan.held_for_other_overridden' AND entity_id = $1
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(loan_id)
        .fetch_optional(pool)
        .await
        .expect("audit");
        if let Some((user_id, found)) = row {
            actor = user_id;
            payload = Some(found);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let payload = payload.expect("hold override audit");
    assert_eq!(actor, Some(staff_id));
    assert_eq!(payload["reason"], "reader is in the room");
    assert_eq!(payload["overrideReason"], "reader is in the room");
    assert_eq!(fixtures::json_id(&payload["patron_id"]), borrower_id);
    assert_eq!(fixtures::json_id(&payload["holdId"]), hold_id);
    assert_eq!(fixtures::json_id(&payload["loanId"]), loan_id);
    assert_eq!(payload["code"], "item_held_for_other");
}

#[tokio::test]
async fn two_holds_and_two_copies_refuse_the_first_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (first_id, _) = fixtures::create_reader(&app, &admin_token, "ttl2a").await;
    let (second_id, _) = fixtures::create_reader(&app, &admin_token, "ttl2b").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttl2c").await;
    let (biblio_id, items) = create_copies(
        &app,
        &admin_token,
        "Two Holds Two Copies",
        &[
            &format!("TWO-A-{}", fixtures::unique_suffix()),
            &format!("TWO-B-{}", fixtures::unique_suffix()),
        ],
    )
    .await;
    let first_hold = pending_hold(&app, first_id, biblio_id, 1, None).await;
    let second_hold = pending_hold(&app, second_id, biblio_id, 2, None).await;

    let (status, body) = checkout(&app, &admin_token, borrower_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_held_body(&body, first_hold, true);
    let (status, other) = checkout(&app, &admin_token, borrower_id, items[1], false, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{other}");
    assert_held_body(&other, first_hold, true);
    let (first_status, first_position, first_item) = hold_row(&app, first_hold).await;
    assert_eq!(first_status, "pending");
    assert_eq!(first_position, 1);
    assert_eq!(first_item, None);
    let (second_status, second_position, _) = hold_row(&app, second_hold).await;
    assert_eq!(second_status, "pending");
    assert_eq!(second_position, 2);
}

async fn set_aside(
    app: &TestApp,
    token: &str,
    barcode: &str,
    hold_id: i64,
    item_id: i64,
) -> (StatusCode, Value) {
    app.post_json(
        &format!("/api/v1/loans/items/{barcode}/return"),
        &json!({
            "holdId": hold_id.to_string(),
            "itemId": item_id.to_string()
        }),
        Some(token),
    )
    .await
}

#[tokio::test]
async fn oldest_queue_hold_is_named_and_set_aside_on_the_same_site() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (later_reader, _) = fixtures::create_reader(&app, &admin_token, "ttlasa").await;
    let (oldest_reader, _) = fixtures::create_reader(&app, &admin_token, "ttlasb").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "ttlasc").await;
    let home = create_site(&app, &admin_token, "aside-home").await;
    let barcode = format!("ASIDE-{}", fixtures::unique_suffix());
    let (biblio_id, items) =
        create_copies(&app, &admin_token, "Set Aside Title", &[&barcode]).await;
    sqlx::query("UPDATE items SET source_id = $2 WHERE id = $1")
        .bind(items[0])
        .bind(home)
        .execute(app.state.services.repository.pool())
        .await
        .expect("home site");
    // Insert the newer queue slot first so id and created_at disagree with position.
    let newer = pending_hold(&app, later_reader, biblio_id, 2, Some(home)).await;
    let oldest = pending_hold(&app, oldest_reader, biblio_id, 1, Some(home)).await;
    assert!(newer < oldest, "newer queue slot was inserted first");

    let (status, refused) = checkout(&app, &admin_token, borrower_id, items[0], false, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{refused}");
    assert_held_body(&refused, oldest, true);

    let (status, mismatch) = set_aside(&app, &admin_token, &barcode, oldest, items[0] + 1).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{mismatch}");

    let (status, skipped) = set_aside(&app, &admin_token, &barcode, newer, items[0]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{skipped}");
    assert_eq!(skipped["code"], "business_rule_violation", "{skipped}");
    let (newer_status, newer_position, newer_item) = hold_row(&app, newer).await;
    assert_eq!(newer_status, "pending");
    assert_eq!(newer_position, 2);
    assert_eq!(newer_item, None);
    let (oldest_status, oldest_position, oldest_item) = hold_row(&app, oldest).await;
    assert_eq!(oldest_status, "pending");
    assert_eq!(oldest_position, 1);
    assert_eq!(oldest_item, None);

    let (status, body) = set_aside(&app, &admin_token, &barcode, oldest, items[0]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "ready", "{body}");
    assert_eq!(fixtures::json_id(&body["hold"]["id"]), oldest, "{body}");
    assert_eq!(body["hold"]["status"], "ready", "{body}");
    assert_eq!(
        fixtures::json_id(&body["hold"]["itemId"]),
        items[0],
        "{body}"
    );
    assert!(body["hold"]["status"] != "to_pick", "{body}");
    let transit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM item_transits WHERE item_id = $1")
            .bind(items[0])
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("no transit");
    assert_eq!(transit_count, 0);
    let (ready_status, _, ready_item) = hold_row(&app, oldest).await;
    assert_eq!(ready_status, "ready");
    assert_eq!(ready_item, Some(items[0]));
    let (still_newer, _, still_unassigned) = hold_row(&app, newer).await;
    assert_eq!(still_newer, "pending");
    assert_eq!(still_unassigned, None);

    let null_pickup = format!("ASIDE-N-{}", fixtures::unique_suffix());
    let (null_biblio, null_items) =
        create_copies(&app, &admin_token, "Set Aside Null Pickup", &[&null_pickup]).await;
    sqlx::query("UPDATE items SET source_id = $2 WHERE id = $1")
        .bind(null_items[0])
        .bind(home)
        .execute(app.state.services.repository.pool())
        .await
        .expect("null-pickup site");
    let null_hold = pending_hold(&app, oldest_reader, null_biblio, 1, None).await;
    let (status, ready) =
        set_aside(&app, &admin_token, &null_pickup, null_hold, null_items[0]).await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["status"], "ready", "{ready}");
    assert_eq!(ready["hold"]["status"], "ready", "{ready}");
    let null_transits: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM item_transits WHERE item_id = $1")
            .bind(null_items[0])
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("null pickup transit count");
    assert_eq!(null_transits, 0);
}

#[tokio::test]
async fn set_aside_cross_site_becomes_ready_only_after_arrival() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (oldest_reader, _) = fixtures::create_reader(&app, &admin_token, "ttltxa").await;
    let (newer_reader, _) = fixtures::create_reader(&app, &admin_token, "ttltxb").await;
    let home = create_site(&app, &admin_token, "aside-origin").await;
    let away = create_site(&app, &admin_token, "aside-pickup").await;
    let barcode = format!("ASIDE-T-{}", fixtures::unique_suffix());
    let (biblio_id, items) =
        create_copies(&app, &admin_token, "Set Aside Transit", &[&barcode]).await;
    sqlx::query("UPDATE items SET source_id = $2 WHERE id = $1")
        .bind(items[0])
        .bind(home)
        .execute(app.state.services.repository.pool())
        .await
        .expect("home site");
    let newer = pending_hold(&app, newer_reader, biblio_id, 2, Some(away)).await;
    let oldest = pending_hold(&app, oldest_reader, biblio_id, 1, Some(away)).await;

    let (status, skipped) = set_aside(&app, &admin_token, &barcode, newer, items[0]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{skipped}");
    let (newer_status, _, newer_item) = hold_row(&app, newer).await;
    assert_eq!(newer_status, "pending");
    assert_eq!(newer_item, None);

    let (status, moved) = set_aside(&app, &admin_token, &barcode, oldest, items[0]).await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["status"], "in_transit", "{moved}");
    assert_eq!(moved["hold"]["status"], "pending", "{moved}");
    assert_ne!(moved["hold"]["status"], "ready", "{moved}");
    assert_ne!(moved["hold"]["status"], "to_pick", "{moved}");
    assert_eq!(
        fixtures::json_id(&moved["hold"]["itemId"]),
        items[0],
        "{moved}"
    );
    let (hold_status, _, hold_item) = hold_row(&app, oldest).await;
    assert_eq!(hold_status, "pending");
    assert_eq!(hold_item, Some(items[0]));
    let (transit_id, transit_status, to_source): (i64, String, i64) = sqlx::query_as(
        r#"
        SELECT id, status, to_source_id
        FROM item_transits
        WHERE item_id = $1 AND hold_id = $2
        "#,
    )
    .bind(items[0])
    .bind(oldest)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("transit");
    assert_eq!(transit_status, "requested");
    assert_eq!(to_source, away);

    let (status, shipped) = app
        .post_json(
            &format!("/api/v1/transits/{transit_id}/ship"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{shipped}");
    assert_eq!(shipped["status"], "inTransit", "{shipped}");
    let (during_ship, _, _) = hold_row(&app, oldest).await;
    assert_eq!(during_ship, "pending");

    let (status, received) = app
        .post_json(
            &format!("/api/v1/transits/{transit_id}/receive"),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{received}");
    assert_eq!(received["transit"]["status"], "received", "{received}");
    assert_eq!(received["hold"]["status"], "ready", "{received}");
    assert_ne!(received["hold"]["status"], "to_pick", "{received}");
    let (arrived, _, arrived_item) = hold_row(&app, oldest).await;
    assert_eq!(arrived, "ready");
    assert_eq!(arrived_item, Some(items[0]));
    let (still_newer, _, still_unassigned) = hold_row(&app, newer).await;
    assert_eq!(still_newer, "pending");
    assert_eq!(still_unassigned, None);
}
