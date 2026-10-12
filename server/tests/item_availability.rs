//! Server-computed copy availability (#118).
//!
//! Staff responses use the full availability enum. Readers and the OPAC use the
//! public enum (`toSearch` is `unavailable`) and a type that has no staff fields.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

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

async fn create_biblio(
    app: &TestApp,
    admin_token: &str,
    title: &str,
    copies: Vec<Value>,
) -> (i64, Vec<(String, i64)>) {
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
        .map(|item| {
            (
                item["barcode"].as_str().unwrap_or_default().to_string(),
                fixtures::json_id(&item["id"]),
            )
        })
        .collect();
    (biblio_id, copies)
}

fn copy(barcode: &str, borrowable: bool) -> Value {
    json!({
        "barcode": barcode,
        "borrowable": borrowable,
        "callNumber": barcode
    })
}

async fn add_copy(
    app: &TestApp,
    admin_token: &str,
    biblio_id: i64,
    barcode: &str,
    borrowable: bool,
) -> i64 {
    let (status, body) = app
        .post_json(
            &format!("/api/v1/biblios/{biblio_id}/items"),
            &copy(barcode, borrowable),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "add copy: {body}");
    fixtures::json_id(&body["id"])
}

async fn place_title(app: &TestApp, token: &str, user_id: i64, biblio_id: i64) -> Value {
    let (status, body) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": user_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "place hold: {body}");
    body
}

async fn patron_name(app: &TestApp, user_id: i64) -> String {
    sqlx::query_scalar("SELECT btrim(concat_ws(' ', firstname, lastname)) FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("patron name")
}

fn find_copy(body: &Value, item_id: i64) -> &Value {
    let items = body["items"].as_array().expect("items");
    items
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == item_id)
        .unwrap_or_else(|| panic!("copy {item_id} missing from {body}"))
}

fn assert_state(copy: &Value, availability: &str, can_hold: bool) {
    assert_eq!(copy["availability"], availability, "availability: {copy}");
    assert_eq!(copy["canHoldItem"], can_hold, "canHoldItem: {copy}");
}

fn assert_no_patron(copy: &Value) {
    assert!(
        copy.get("holdUserId").is_none(),
        "reader saw holdUserId: {copy}"
    );
    assert!(
        copy.get("holdUserName").is_none(),
        "reader saw holdUserName: {copy}"
    );
}

async fn staff_biblio(app: &TestApp, token: &str, biblio_id: i64) -> Value {
    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "staff biblio: {body}");
    body
}

#[tokio::test]
async fn to_pick_is_on_hold_for_staff_and_hidden_for_readers() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "avpick").await;
    let name = patron_name(&app, reader_id).await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Availability ToPick {suffix}");
    let hold_barcode = format!("AVH-{suffix}");
    let (biblio_id, copies) =
        create_biblio(&app, &admin_token, &title, vec![copy(&hold_barcode, true)]).await;
    let hold_id_item = copies[0].1;

    let hold = place_title(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(hold["status"], "to_pick");
    assert_eq!(fixtures::json_id(&hold["itemId"]), hold_id_item);

    let staff = staff_biblio(&app, &admin_token, biblio_id).await;
    let staff_copy = find_copy(&staff, hold_id_item);
    assert_state(staff_copy, "onHold", false);
    assert_eq!(fixtures::json_id(&staff_copy["holdUserId"]), reader_id);
    assert_eq!(staff_copy["holdUserName"], name);

    let (status, reader) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "reader biblio: {reader}");
    let reader_copy = find_copy(&reader, hold_id_item);
    assert_state(reader_copy, "onHold", false);
    assert_no_patron(reader_copy);
    let raw = reader.to_string();
    assert!(!raw.contains("holdUserId"));
    assert!(!raw.contains("holdUserName"));
    assert!(!raw.contains(&name));

    let (status, opac) = app
        .get_json(&format!("/api/v1/opac/biblios/{biblio_id}"))
        .await;
    assert_eq!(status, StatusCode::OK, "opac biblio: {opac}");
    let opac_copy = find_copy(&opac, hold_id_item);
    assert_state(opac_copy, "onHold", false);
    assert_no_patron(opac_copy);

    let (status, item_detail) = app
        .get_json_with_auth(&format!("/api/v1/items/{hold_id_item}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "item detail: {item_detail}");
    assert_state(find_copy(&item_detail, hold_id_item), "onHold", false);
    assert_eq!(find_copy(&item_detail, hold_id_item)["holdUserName"], name);

    let (status, reader_item) = app
        .get_json_with_auth(&format!("/api/v1/items/{hold_id_item}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "reader item: {reader_item}");
    assert_no_patron(find_copy(&reader_item, hold_id_item));

    let (status, listed) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "item list: {listed}");
    let listed_copy = listed
        .as_array()
        .expect("list")
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == hold_id_item)
        .expect("listed copy");
    assert_state(listed_copy, "onHold", false);
    assert_eq!(listed_copy["holdUserName"], name);

    let (status, search) = app
        .get_json_with_auth(
            &format!(
                "/api/v1/biblios?title={}&perPage=5",
                urlencoding_title(&title)
            ),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "search: {search}");
    let hit = search["items"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == biblio_id)
        .expect("search hit");
    assert_state(find_copy(hit, hold_id_item), "onHold", false);
    assert_eq!(find_copy(hit, hold_id_item)["holdUserName"], name);

    let (status, opac_search) = app
        .get_json(&format!(
            "/api/v1/opac/biblios?title={}&perPage=5",
            urlencoding_title(&title)
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "opac search: {opac_search}");
    let opac_hit = opac_search["items"]
        .as_array()
        .expect("opac hits")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == biblio_id)
        .expect("opac hit");
    assert_state(find_copy(opac_hit, hold_id_item), "onHold", false);
    assert_no_patron(find_copy(opac_hit, hold_id_item));
}

fn urlencoding_title(title: &str) -> String {
    title.replace(' ', "%20")
}

#[tokio::test]
async fn ready_stays_on_hold_and_shelf_copy_is_available() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "avready").await;
    let suffix = fixtures::unique_suffix();
    let hold_barcode = format!("AVR-{suffix}");
    let shelf_barcode = format!("AVS-{suffix}");
    let (biblio_id, copies) = create_biblio(
        &app,
        &admin_token,
        &format!("Availability Ready {suffix}"),
        vec![copy(&hold_barcode, true)],
    )
    .await;
    let held = copies[0].1;
    let shelf = add_copy(&app, &admin_token, biblio_id, &shelf_barcode, true).await;

    let hold = place_title(&app, &reader_token, reader_id, biblio_id).await;
    assert_eq!(hold["status"], "to_pick");
    assert_eq!(fixtures::json_id(&hold["itemId"]), held);

    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{hold_barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "check in: {scan}");
    assert_eq!(scan["status"], "ready");
    assert_eq!(scan["hold"]["status"], "ready");

    let body = staff_biblio(&app, &admin_token, biblio_id).await;
    assert_state(find_copy(&body, held), "onHold", false);
    assert_eq!(
        fixtures::json_id(&find_copy(&body, held)["holdUserId"]),
        reader_id
    );
    assert_state(find_copy(&body, shelf), "available", true);
    assert!(find_copy(&body, shelf).get("holdUserId").is_none());
}

#[tokio::test]
async fn transit_loan_tosearch_and_blocked_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let origin = create_site(&app, &admin_token, "AvOrigin").await;
    let pickup = create_site(&app, &admin_token, "AvPickup").await;
    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "avtrn").await;
    let (borrower_id, _) = fixtures::create_reader(&app, &admin_token, "avloan").await;
    let suffix = fixtures::unique_suffix();

    let transit_barcode = format!("AVT-{suffix}");
    let (transit_biblio, transit_copies) = create_biblio(
        &app,
        &admin_token,
        &format!("Availability Transit {suffix}"),
        vec![json!({
            "barcode": transit_barcode,
            "borrowable": true,
            "sourceId": origin.to_string()
        })],
    )
    .await;
    let transit_item = transit_copies[0].1;
    let hold = place_title(&app, &holder_token, holder_id, transit_biblio).await;
    assert_eq!(hold["status"], "to_pick");
    sqlx::query("UPDATE holds SET pickup_site_id = $2 WHERE id = $1")
        .bind(fixtures::json_id(&hold["id"]))
        .bind(pickup)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set pickup");
    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{transit_barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "transit scan: {scan}");
    assert_eq!(scan["status"], "in_transit");
    let transit_body = staff_biblio(&app, &admin_token, transit_biblio).await;
    assert_state(find_copy(&transit_body, transit_item), "inTransit", false);
    assert_no_patron(find_copy(&transit_body, transit_item));

    let loan_barcode = format!("AVL-{suffix}");
    let (loan_biblio, loan_copies) = create_biblio(
        &app,
        &admin_token,
        &format!("Availability Loan {suffix}"),
        vec![copy(&loan_barcode, true)],
    )
    .await;
    let loan_item = loan_copies[0].1;
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": borrower_id.to_string(),
                "itemId": loan_item.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");
    let loan_body = staff_biblio(&app, &admin_token, loan_biblio).await;
    assert_state(find_copy(&loan_body, loan_item), "onLoan", true);

    sqlx::query("UPDATE items SET circulation_status = 4 WHERE id = $1")
        .bind(loan_item)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark to_search while on loan");
    let precedence = staff_biblio(&app, &admin_token, loan_biblio).await;
    assert_state(find_copy(&precedence, loan_item), "onLoan", true);

    let search_barcode = format!("AVQ-{suffix}");
    let (search_biblio, search_copies) = create_biblio(
        &app,
        &admin_token,
        &format!("Availability Search {suffix}"),
        vec![copy(&search_barcode, true)],
    )
    .await;
    let search_item = search_copies[0].1;
    let missing = place_title(&app, &holder_token, holder_id, search_biblio).await;
    assert_eq!(missing["status"], "to_pick");
    let hold_id = fixtures::json_id(&missing["id"]);
    let (status, not_found) = app
        .post_empty(
            &format!("/api/v1/holds/{hold_id}/pick-not-found"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "not found: {not_found}");
    let search_body = staff_biblio(&app, &admin_token, search_biblio).await;
    assert_state(find_copy(&search_body, search_item), "toSearch", false);

    let (blocked_biblio, _) = create_biblio(
        &app,
        &admin_token,
        &format!("Availability Blocked {suffix}"),
        vec![copy(&format!("AVINC-{suffix}"), false)],
    )
    .await;
    let lost = add_copy(
        &app,
        &admin_token,
        blocked_biblio,
        &format!("AVLOST-{suffix}"),
        true,
    )
    .await;
    let damaged = add_copy(
        &app,
        &admin_token,
        blocked_biblio,
        &format!("AVDMG-{suffix}"),
        true,
    )
    .await;
    let claimed = add_copy(
        &app,
        &admin_token,
        blocked_biblio,
        &format!("AVCLM-{suffix}"),
        true,
    )
    .await;
    let withdrawn = add_copy(
        &app,
        &admin_token,
        blocked_biblio,
        &format!("AVWD-{suffix}"),
        true,
    )
    .await;
    let candidate = add_copy(
        &app,
        &admin_token,
        blocked_biblio,
        &format!("AVCAND-{suffix}"),
        true,
    )
    .await;

    sqlx::query("UPDATE items SET circulation_status = 1 WHERE id = $1")
        .bind(lost)
        .execute(app.state.services.repository.pool())
        .await
        .expect("lost");
    sqlx::query("UPDATE items SET circulation_status = 2 WHERE id = $1")
        .bind(damaged)
        .execute(app.state.services.repository.pool())
        .await
        .expect("damaged");
    sqlx::query("UPDATE items SET circulation_status = 3 WHERE id = $1")
        .bind(claimed)
        .execute(app.state.services.repository.pool())
        .await
        .expect("claimed");

    let (status, wd) = app
        .post_json(
            &format!("/api/v1/items/{withdrawn}/withdrawals"),
            &json!({ "reason": "worn", "destination": "pulp" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {wd}");
    let (status, cand) = app
        .post_json(
            &format!("/api/v1/items/{candidate}/weeding"),
            &json!({ "status": "candidate" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "candidate: {cand}");

    let blocked = staff_biblio(&app, &admin_token, blocked_biblio).await;
    let incomplete = blocked["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["barcode"].as_str().unwrap_or("").starts_with("AVINC-"))
        .expect("incomplete");
    assert_state(incomplete, "unavailable", false);
    assert_state(find_copy(&blocked, lost), "unavailable", false);
    assert_state(find_copy(&blocked, damaged), "unavailable", false);
    assert_state(find_copy(&blocked, claimed), "unavailable", false);
    assert_state(find_copy(&blocked, withdrawn), "unavailable", false);
    assert_state(find_copy(&blocked, candidate), "available", true);

    let (status, by_barcode) = app
        .get_json_with_auth(
            &format!("/api/v1/items/barcode/{loan_barcode}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "barcode: {by_barcode}");
    assert_state(find_copy(&by_barcode, loan_item), "onLoan", true);
}

const PUBLIC_FORBIDDEN_KEYS: &[&str] = &[
    "holdUserId",
    "holdUserName",
    "login",
    "notes",
    "weedingStatus",
    "weedingReason",
];

fn assert_public_catalog(body: &Value) {
    fn walk(value: &Value, path: &str) {
        match value {
            Value::Object(map) => {
                for key in PUBLIC_FORBIDDEN_KEYS {
                    assert!(
                        !map.contains_key(*key),
                        "public JSON has {key} at {path}: {value}"
                    );
                }
                for (key, child) in map {
                    walk(child, &format!("{path}.{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    walk(body, "$");
}

struct HiddenCopy {
    biblio_id: i64,
    item_id: i64,
    title: String,
    reader_token: String,
}

async fn seed_tosearch_copy(app: &TestApp, label: &str) -> HiddenCopy {
    let admin_token = fixtures::ensure_first_setup(app).await;
    let (_reader_id, reader_token) =
        fixtures::create_reader(app, &admin_token, &format!("avpub{label}")).await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Public availability {label} {suffix}");
    let barcode = format!("AVP-{label}-{suffix}");
    let (biblio_id, copies) =
        create_biblio(app, &admin_token, &title, vec![copy(&barcode, true)]).await;
    let item_id = copies[0].1;

    sqlx::query("UPDATE biblios SET notes = 'staff biblio note' WHERE id = $1")
        .bind(biblio_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("biblio notes");
    sqlx::query("UPDATE items SET notes = 'staff copy note' WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("copy notes");

    let (status, weeding) = app
        .post_json(
            &format!("/api/v1/items/{item_id}/weeding"),
            &json!({ "status": "candidate", "reason": "internal discard note" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "weeding: {weeding}");

    let (reader_id, _) =
        fixtures::create_reader(app, &admin_token, &format!("avhold{label}")).await;
    let hold = place_title(app, &admin_token, reader_id, biblio_id).await;
    assert_eq!(hold["status"], "to_pick");
    let hold_id = fixtures::json_id(&hold["id"]);
    let (status, not_found) = app
        .post_empty(
            &format!("/api/v1/holds/{hold_id}/pick-not-found"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "pick not found: {not_found}");

    let staff = staff_biblio(app, &admin_token, biblio_id).await;
    assert_state(find_copy(&staff, item_id), "toSearch", false);

    HiddenCopy {
        biblio_id,
        item_id,
        title,
        reader_token,
    }
}

#[tokio::test]
async fn reader_biblio_detail_hides_staff_fields_and_to_search() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let hidden = seed_tosearch_copy(&app, "biblio").await;
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{}", hidden.biblio_id),
            &hidden.reader_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "reader biblio: {body}");
    assert_public_catalog(&body);
    assert_state(find_copy(&body, hidden.item_id), "unavailable", false);
}

#[tokio::test]
async fn opac_biblio_detail_hides_staff_fields_and_to_search() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let hidden = seed_tosearch_copy(&app, "opac").await;
    let (status, body) = app
        .get_json(&format!("/api/v1/opac/biblios/{}", hidden.biblio_id))
        .await;
    assert_eq!(status, StatusCode::OK, "opac biblio: {body}");
    assert_public_catalog(&body);
    assert_state(find_copy(&body, hidden.item_id), "unavailable", false);
}

#[tokio::test]
async fn reader_item_detail_hides_staff_fields_and_to_search() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let hidden = seed_tosearch_copy(&app, "item").await;
    let (status, body) = app
        .get_json_with_auth(
            &format!("/api/v1/items/{}", hidden.item_id),
            &hidden.reader_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "reader item: {body}");
    assert_public_catalog(&body);
    assert_state(find_copy(&body, hidden.item_id), "unavailable", false);
}

#[tokio::test]
async fn reader_search_hides_staff_fields_and_to_search() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let hidden = seed_tosearch_copy(&app, "search").await;
    let (status, body) = app
        .get_json_with_auth(
            &format!(
                "/api/v1/biblios?title={}&perPage=5",
                urlencoding_title(&hidden.title)
            ),
            &hidden.reader_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "reader search: {body}");
    assert_public_catalog(&body);
    let hit = body["items"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == hidden.biblio_id)
        .expect("search hit");
    assert_state(find_copy(hit, hidden.item_id), "unavailable", false);
}

fn assert_no_login_anywhere(body: &Value) {
    fn walk(value: &Value, path: &str) {
        match value {
            Value::Object(map) => {
                assert!(
                    !map.contains_key("login"),
                    "reader JSON has login at {path}: {value}"
                );
                for (key, child) in map {
                    walk(child, &format!("{path}.{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(child, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    walk(body, "$");
}

fn assert_public_patron(user: &Value) {
    assert!(user.is_object(), "public patron: {user}");
    assert!(
        user.get("login").is_none(),
        "public patron has login: {user}"
    );
    assert!(
        user.get("barcode").is_none(),
        "public patron has barcode: {user}"
    );
}

/// Staff patron search returns `login` and `barcode`. Reader loan and hold rows do not.
#[tokio::test]
async fn reader_patron_summary_omits_login_staff_picker_includes_it() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "picker").await;
    let card = format!("CARD-{}", fixtures::unique_suffix());
    sqlx::query("UPDATE users SET barcode = $1 WHERE id = $2")
        .bind(&card)
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set card number");

    let (status, profile) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "staff profile: {profile}");
    let login = profile["login"].as_str().expect("staff profile login");

    let (status, denied) = app
        .get_json_with_auth(&format!("/api/v1/users?name={login}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader user list: {denied}");

    let (status, list) = app
        .get_json_with_auth(&format!("/api/v1/users?name={login}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "staff user list: {list}");
    let row = list["items"]
        .as_array()
        .expect("users")
        .iter()
        .find(|user| fixtures::json_id(&user["id"]) == reader_id)
        .expect("picker row");
    assert_eq!(row["login"], login);
    assert_eq!(row["barcode"], card);

    let barcode = format!("AVP-loan-{}", fixtures::unique_suffix());
    let (biblio_id, copies) = create_biblio(
        &app,
        &admin_token,
        "Picker loan",
        vec![copy(&barcode, true)],
    )
    .await;
    let item_id = copies[0].1;
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");

    let (status, loans) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/loans"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "reader loans: {loans}");
    assert!(
        !loans["items"].as_array().expect("loans").is_empty(),
        "{loans}"
    );
    assert_no_login_anywhere(&loans);

    place_title(&app, &admin_token, reader_id, biblio_id).await;
    let (status, holds) = app
        .get_json_with_auth("/api/v1/holds?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "reader holds: {holds}");
    assert_no_login_anywhere(&holds);
    let hold_user = &holds["items"]
        .as_array()
        .expect("holds")
        .iter()
        .find(|row| fixtures::json_id(&row["userId"]) == reader_id)
        .expect("reader hold")["user"];
    assert_public_patron(hold_user);

    let (status, staff_holds) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/holds"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "staff holds: {staff_holds}");
    let staff_user = &staff_holds
        .as_array()
        .expect("holds")
        .iter()
        .find(|row| fixtures::json_id(&row["userId"]) == reader_id)
        .expect("staff hold")["user"];
    assert_eq!(staff_user["login"], login);
    assert!(
        staff_user.get("barcode").is_none() || staff_user["barcode"].is_null(),
        "card number stays off embedded hold patrons: {staff_user}"
    );
}
