//! Transit responses include the copy barcode, title, site, and a minimal patron.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::{json, Value};

fn retarget(token: &str, secret: &str, holds: Rights, users: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.holds_rights = holds;
    claims.rights.users_rights = users;
    claims.create_token(secret).expect("mint test token")
}

fn assert_minimal_patron(patron: &Value, card: &str) {
    assert_eq!(patron["cardNumber"], card, "{patron}");
    assert_eq!(patron["initials"], "HH", "{patron}");
    let mut keys: Vec<&str> = patron
        .as_object()
        .expect("patron object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["cardNumber", "initials"], "{patron}");
}

fn assert_copy_fields(row: &Value, barcode: &str, title: &str, source_id: i64) {
    assert!(row.get("barcode").is_some(), "barcode key: {row}");
    assert!(row.get("title").is_some(), "title key: {row}");
    assert!(row.get("sourceId").is_some(), "sourceId key: {row}");
    assert!(row.get("sourceName").is_some(), "sourceName key: {row}");
    assert!(row.get("patron").is_some(), "patron key: {row}");
    assert_eq!(row["barcode"], barcode, "{row}");
    assert_eq!(row["title"], title, "{row}");
    assert_eq!(fixtures::json_id(&row["sourceId"]), source_id, "{row}");
    assert!(
        row["sourceName"]
            .as_str()
            .is_some_and(|name| !name.is_empty()),
        "sourceName: {row}"
    );
}

fn assert_no_identity(body: &Value, login: &str) {
    let text = body.to_string();
    for needle in ["Helene", "Holder", login, "helene.holder@"] {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

struct Shipped {
    barcode: String,
    title: String,
    origin: i64,
    hold_id: i64,
    transit_id: i64,
    item_id: i64,
    card: String,
    login: String,
}

async fn ship_cross_site(app: &TestApp, admin_token: &str, label: &str) -> Shipped {
    let origin = create_site(app, admin_token, &format!("{label}-from")).await;
    let pickup = create_site(app, admin_token, &format!("{label}-to")).await;
    let (reader_id, reader_token) = fixtures::create_reader(app, admin_token, label).await;
    let card = format!("TRN-{reader_id}");
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Helene', lastname = 'Holder', barcode = $2,
            email = $3
        WHERE id = $1
        "#,
    )
    .bind(reader_id)
    .bind(&card)
    .bind(format!("helene.holder.{reader_id}@test.local"))
    .execute(app.state.services.repository.pool())
    .await
    .expect("reader identity");

    let suffix = fixtures::unique_suffix();
    let title = format!("TransitTitle-{suffix}");
    let barcode = format!("TRN-BC-{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "borrowable": true,
                    "sourceId": origin.to_string()
                }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "pickupSiteId": pickup.to_string()
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "place hold: {hold}");
    let hold_id = fixtures::json_id(&hold["id"]);

    let (status, shipped) = app
        .post_json(
            &format!("/api/v1/holds/{hold_id}/transits"),
            &json!({ "itemId": item_id.to_string(), "ship": true }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "ship: {shipped}");
    let transit = &shipped["transit"];
    assert_copy_fields(transit, &barcode, &title, origin);
    assert_minimal_patron(&transit["patron"], &card);
    assert_no_identity(&shipped, &login);

    Shipped {
        barcode,
        title,
        origin,
        hold_id,
        transit_id: fixtures::json_id(&transit["id"]),
        item_id,
        card,
        login,
    }
}

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

async fn transit_row(app: &TestApp, token: &str, transit_id: i64) -> Value {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/transits/{transit_id}"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "get transit: {body}");
    body["transit"].clone()
}

#[tokio::test]
async fn list_and_active_routes_include_copy_fields_and_minimal_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let shipped = ship_cross_site(&app, &admin_token, "trndisp").await;

    let listed = transit_row(&app, &admin_token, shipped.transit_id).await;
    assert_copy_fields(&listed, &shipped.barcode, &shipped.title, shipped.origin);
    assert_minimal_patron(&listed["patron"], &shipped.card);

    let (status, list) = app
        .get_json_with_auth(
            &format!("/api/v1/transits?itemId={}&perPage=20", shipped.item_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "list: {list}");
    let row = list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| fixtures::json_id(&row["id"]) == shipped.transit_id)
        .expect("listed transit");
    assert_copy_fields(row, &shipped.barcode, &shipped.title, shipped.origin);
    assert_minimal_patron(&row["patron"], &shipped.card);
    assert_no_identity(&list, &shipped.login);

    let (status, by_hold) = app
        .get_json_with_auth(
            &format!("/api/v1/holds/{}/transits", shipped.hold_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "hold transit: {by_hold}");
    assert_copy_fields(&by_hold, &shipped.barcode, &shipped.title, shipped.origin);
    assert_minimal_patron(&by_hold["patron"], &shipped.card);

    let (status, by_item) = app
        .get_json_with_auth(
            &format!("/api/v1/items/{}/transit", shipped.item_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "item transit: {by_item}");
    assert_copy_fields(&by_item, &shipped.barcode, &shipped.title, shipped.origin);
    assert_minimal_patron(&by_item["patron"], &shipped.card);

    let (status, received) = app
        .post_json(
            &format!("/api/v1/transits/{}/receive", shipped.transit_id),
            &json!({}),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "receive: {received}");
    assert_eq!(received["hold"]["status"], "ready");
    assert_minimal_patron(&received["transit"]["patron"], &shipped.card);
    assert_eq!(
        fixtures::json_id(&received["transit"]["sourceId"]),
        fixtures::json_id(&received["transit"]["toSourceId"]),
        "sourceId follows the copy after arrival: {received}"
    );
}

#[tokio::test]
async fn patron_is_null_without_holds_rights_and_minimal_with_them() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let shipped = ship_cross_site(&app, &admin_token, "trnmask").await;
    let secret = app.state.config.users.jwt_secret.clone();

    let no_holds = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let hidden = transit_row(&app, &no_holds, shipped.transit_id).await;
    assert_copy_fields(&hidden, &shipped.barcode, &shipped.title, shipped.origin);
    assert!(hidden["patron"].is_null(), "no holds rights: {hidden}");
    assert_no_identity(&hidden, &shipped.login);

    let holds_only = retarget(&admin_token, &secret, Rights::Read, Rights::None);
    let minimal = transit_row(&app, &holds_only, shipped.transit_id).await;
    assert_minimal_patron(&minimal["patron"], &shipped.card);
    assert_no_identity(&minimal, &shipped.login);

    let holds_and_users = retarget(&admin_token, &secret, Rights::Read, Rights::Read);
    let still_minimal = transit_row(&app, &holds_and_users, shipped.transit_id).await;
    assert_minimal_patron(&still_minimal["patron"], &shipped.card);
    assert_no_identity(&still_minimal, &shipped.login);
}

#[tokio::test]
async fn patron_is_null_for_a_return_without_a_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let shipped = ship_cross_site(&app, &admin_token, "trnhome").await;

    let (status, cancelled) = app
        .delete_with_auth(&format!("/api/v1/holds/{}", shipped.hold_id), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "cancel hold: {cancelled}");

    let (status, list) = app
        .get_json_with_auth(
            &format!("/api/v1/transits?itemId={}&perPage=20", shipped.item_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "list: {list}");
    let reverse = list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| row["holdId"].is_null() && row["reversedFromId"].is_string())
        .expect("reverse transit");
    assert_copy_fields(reverse, &shipped.barcode, &shipped.title, shipped.origin);
    assert!(reverse["patron"].is_null(), "return home: {reverse}");
}

#[tokio::test]
async fn patron_is_null_when_the_linked_hold_is_cancelled() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let shipped = ship_cross_site(&app, &admin_token, "trncan").await;

    let (status, _) = app
        .delete_with_auth(&format!("/api/v1/holds/{}", shipped.hold_id), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK);

    let row = transit_row(&app, &admin_token, shipped.transit_id).await;
    assert_eq!(fixtures::json_id(&row["holdId"]), shipped.hold_id, "{row}");
    assert_eq!(row["status"], "cancelled", "{row}");
    assert_copy_fields(&row, &shipped.barcode, &shipped.title, shipped.origin);
    assert!(row["patron"].is_null(), "cancelled hold: {row}");
    assert_no_identity(&row, &shipped.login);
}

#[tokio::test]
async fn patron_is_null_when_the_linked_hold_is_expired() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let shipped = ship_cross_site(&app, &admin_token, "trnexp").await;

    sqlx::query("UPDATE holds SET status = 'expired' WHERE id = $1")
        .bind(shipped.hold_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("expire hold");

    let row = transit_row(&app, &admin_token, shipped.transit_id).await;
    assert_eq!(fixtures::json_id(&row["holdId"]), shipped.hold_id, "{row}");
    assert_copy_fields(&row, &shipped.barcode, &shipped.title, shipped.origin);
    assert!(row["patron"].is_null(), "expired hold: {row}");
    assert_no_identity(&row, &shipped.login);
}

#[tokio::test]
async fn same_site_transit_is_422_and_writes_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let site = create_site(&app, &admin_token, "trnsame").await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "trnsame").await;
    let suffix = fixtures::unique_suffix();
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("SameSite-{suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("SAME-{suffix}"),
                    "borrowable": true,
                    "sourceId": site.to_string()
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "pickupSiteId": site.to_string()
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "place hold: {hold}");
    assert_eq!(hold["status"], "to_pick", "{hold}");
    let hold_id = fixtures::json_id(&hold["id"]);

    let before_hold: (String, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT status, item_id, pickup_site_id FROM holds WHERE id = $1")
            .bind(hold_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("hold snapshot");
    let before_source: Option<i64> =
        sqlx::query_scalar("SELECT source_id FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("item site");
    let before_transits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM item_transits WHERE item_id = $1 OR hold_id = $2")
            .bind(item_id)
            .bind(hold_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("transit count");

    let (status, body) = app
        .post_json(
            &format!("/api/v1/holds/{hold_id}/transits"),
            &json!({
                "itemId": item_id.to_string(),
                "fromSourceId": site.to_string(),
                "toSourceId": site.to_string(),
                "ship": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let mut keys: Vec<&str> = body
        .as_object()
        .expect("error object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "error", "message"], "{body}");
    assert_eq!(body["code"], "transit_same_site", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{body}"
    );

    let after_hold: (String, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT status, item_id, pickup_site_id FROM holds WHERE id = $1")
            .bind(hold_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("hold after");
    let after_source: Option<i64> = sqlx::query_scalar("SELECT source_id FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("item site after");
    let after_transits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM item_transits WHERE item_id = $1 OR hold_id = $2")
            .bind(item_id)
            .bind(hold_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("transit count after");
    assert_eq!(after_hold, before_hold, "hold was written");
    assert_eq!(after_source, before_source, "item site was written");
    assert_eq!(after_transits, before_transits, "a transit row was written");
    assert_eq!(before_transits, 0);

    let (status, active) = app
        .get_json_with_auth(&format!("/api/v1/holds/{hold_id}/transits"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{active}");
    assert!(active.is_null(), "active transit: {active}");
}
