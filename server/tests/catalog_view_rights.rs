//! Staff versus public catalog JSON follows rights, not account type (#131).
//!
//! Staff, patron, and OPAC keep the shapes from #195 and #197. An intermediate
//! profile is an existing account type (`reader` or `librarian`) with custom rights.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{AccountTypeSlug, Rights, UserClaims, UserRights};
use serde_json::{json, Value};

fn retarget(token: &str, secret: &str, account_type: AccountTypeSlug, rights: UserRights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.account_type = account_type;
    claims.rights = rights;
    claims.create_token(secret).expect("mint test token")
}

fn patron_rights() -> UserRights {
    UserRights {
        items_rights: Rights::Read,
        users_rights: Rights::Own,
        loans_rights: Rights::Own,
        holds_rights: Rights::Own,
        settings_rights: Rights::Read,
        events_rights: Rights::Read,
        items_archive_rights: Rights::Read,
        ..UserRights::default()
    }
}

struct Notice {
    biblio_id: i64,
    title: String,
    held_id: i64,
    lost_id: i64,
    lost_barcode: String,
    hold_id: i64,
    holder_id: i64,
    holder_token: String,
    card: String,
    login: String,
    notes: String,
}

async fn seed(app: &TestApp, admin_token: &str) -> Notice {
    let (holder_id, holder_token) = fixtures::create_reader(app, admin_token, "viewhold").await;
    let card = format!("VIEW-{holder_id}");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Helene', lastname = 'Holder', barcode = $2
        WHERE id = $1
        "#,
    )
    .bind(holder_id)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("holder identity");
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(holder_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("holder login");

    let suffix = fixtures::unique_suffix();
    let title = format!("ViewRights-{suffix}");
    let held_barcode = format!("VIEW-HELD-{suffix}");
    let lost_barcode = format!("VIEW-LOST-{suffix}");
    let notes = format!("internal note {suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "notes": notes,
                "items": [
                    { "barcode": held_barcode, "borrowable": true },
                    { "barcode": lost_barcode, "borrowable": true }
                ]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let copies = created["biblio"]["items"].as_array().expect("created copies");
    let item_id = |barcode: &str| {
        copies
            .iter()
            .find(|item| item["barcode"].as_str() == Some(barcode))
            .map(|item| fixtures::json_id(&item["id"]))
            .unwrap_or_else(|| panic!("copy {barcode} missing from {created}"))
    };
    let held_id = item_id(&held_barcode);
    let lost_id = item_id(&lost_barcode);
    sqlx::query("UPDATE items SET circulation_status = 1 WHERE id = $1")
        .bind(lost_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark lost");

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": holder_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&holder_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{hold}");

    Notice {
        biblio_id,
        title,
        held_id,
        lost_id,
        lost_barcode,
        hold_id: fixtures::json_id(&hold["id"]),
        holder_id,
        holder_token,
        card,
        login,
        notes,
    }
}

fn copy_by_id(body: &Value, item_id: i64) -> Option<&Value> {
    body["items"].as_array().and_then(|items| items.iter().find(|item| fixtures::json_id(&item["id"]) == item_id))
}

fn assert_public_notice(body: &Value, notice: &Notice) {
    assert!(body.get("notes").is_none(), "notes leaked: {body}");
    assert!(copy_by_id(body, notice.lost_id).is_none(), "lost copy leaked: {body}");
    let held = copy_by_id(body, notice.held_id).unwrap_or_else(|| panic!("held copy missing: {body}"));
    assert!(held.get("circulationStatus").is_none(), "{held}");
    assert!(held.get("weedingStatus").is_none(), "{held}");
    assert!(held.get("holdFor").is_none(), "{held}");
    assert!(held.get("borrower").is_none(), "{held}");
    assert_eq!(held["onHold"], true, "{held}");
    let raw = body.to_string();
    assert!(!raw.contains("Helene"), "{body}");
    assert!(!raw.contains(&notice.notes), "{body}");
}

fn assert_staff_copies(body: &Value, notice: &Notice) {
    assert_eq!(body["notes"], notice.notes, "{body}");
    let lost = copy_by_id(body, notice.lost_id).unwrap_or_else(|| panic!("lost copy missing: {body}"));
    assert_eq!(lost["circulationStatus"], 1, "{lost}");
    assert!(lost.get("weedingStatus").is_some(), "{lost}");
    let held = copy_by_id(body, notice.held_id).expect("held copy");
    assert!(held.get("circulationStatus").is_some(), "{held}");
}

fn assert_minimized_hold_for(body: &Value, notice: &Notice) {
    let held = copy_by_id(body, notice.held_id).expect("held copy");
    let hold_for = &held["holdFor"];
    assert_eq!(hold_for["id"], notice.holder_id.to_string(), "{hold_for}");
    assert_eq!(hold_for["cardNumber"], notice.card, "{hold_for}");
    assert_eq!(hold_for["initials"], "HH", "{hold_for}");
    assert!(hold_for.get("firstname").is_none(), "{hold_for}");
    assert!(hold_for.get("login").is_none(), "{hold_for}");
    assert!(hold_for.get("barcode").is_none(), "{hold_for}");
}

fn assert_full_hold_for(body: &Value, notice: &Notice) {
    let held = copy_by_id(body, notice.held_id).expect("held copy");
    let hold_for = &held["holdFor"];
    assert_eq!(hold_for["firstname"], "Helene", "{hold_for}");
    assert_eq!(hold_for["lastname"], "Holder", "{hold_for}");
    assert_eq!(hold_for["login"], notice.login, "{hold_for}");
    assert_eq!(hold_for["barcode"], notice.card, "{hold_for}");
    assert!(hold_for.get("cardNumber").is_none(), "{hold_for}");
    assert!(hold_for.get("email").is_none(), "{hold_for}");
}

fn list_row(body: &Value, biblio_id: i64) -> &Value {
    body["items"]
        .as_array()
        .expect("list")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == biblio_id)
        .unwrap_or_else(|| panic!("biblio {biblio_id} missing from {body}"))
}

async fn get(app: &TestApp, path: &str, token: Option<&str>) -> (StatusCode, Value) {
    match token {
        Some(token) => app.get_json_with_auth(path, token).await,
        None => app.get_json(path).await,
    }
}

#[tokio::test]
async fn staff_patron_opac_and_intermediate_catalog_views_follow_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let notice = seed(&app, &admin_token).await;
    let secret = app.state.config.users.jwt_secret.clone();

    let (status, staff) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{staff}");
    assert_staff_copies(&staff, &notice);
    assert_full_hold_for(&staff, &notice);

    let (status, patron) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&notice.holder_token)).await;
    assert_eq!(status, StatusCode::OK, "{patron}");
    assert_public_notice(&patron, &notice);

    let (status, opac) = get(&app, &format!("/api/v1/opac/biblios/{}", notice.biblio_id), None).await;
    assert_eq!(status, StatusCode::OK, "{opac}");
    assert_public_notice(&opac, &notice);

    let (status, lost_public) = get(&app, &format!("/api/v1/items/{}", notice.lost_id), Some(&notice.holder_token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{lost_public}");
    let (status, lost_staff) = get(&app, &format!("/api/v1/items/barcode/{}", notice.lost_barcode), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{lost_staff}");
    assert_eq!(lost_staff["items"][0]["circulationStatus"], 1, "{lost_staff}");

    let cataloger = retarget(
        &notice.holder_token,
        &secret,
        AccountTypeSlug::Reader,
        UserRights {
            items_rights: Rights::Write,
            users_rights: Rights::Own,
            loans_rights: Rights::None,
            holds_rights: Rights::Own,
            ..UserRights::default()
        },
    );
    let (status, intermediate) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&cataloger)).await;
    assert_eq!(status, StatusCode::OK, "{intermediate}");
    assert_staff_copies(&intermediate, &notice);
    assert!(
        copy_by_id(&intermediate, notice.held_id).expect("held").get("holdFor").is_none(),
        "catalog write without holds rights omits holdFor: {intermediate}"
    );

    let (status, cataloger_items) = get(&app, &format!("/api/v1/biblios/{}/items", notice.biblio_id), Some(&cataloger)).await;
    assert_eq!(status, StatusCode::OK, "{cataloger_items}");
    assert!(
        cataloger_items.as_array().expect("item list").iter().any(|item| fixtures::json_id(&item["id"]) == notice.lost_id),
        "catalog write keeps the lost copy: {cataloger_items}"
    );

    let librarian_public = retarget(&admin_token, &secret, AccountTypeSlug::Librarian, patron_rights());
    let (status, demoted) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&librarian_public)).await;
    assert_eq!(status, StatusCode::OK, "{demoted}");
    assert_public_notice(&demoted, &notice);

    let (status, search) = get(&app, &format!("/api/v1/biblios?title={}", notice.title), Some(&librarian_public)).await;
    assert_eq!(status, StatusCode::OK, "{search}");
    let row = list_row(&search, notice.biblio_id);
    assert!(row.get("weedingStatus").is_none(), "{row}");
    assert!(
        row["items"].as_array().expect("copies").iter().all(|item| fixtures::json_id(&item["id"]) != notice.lost_id),
        "public search omits the lost copy: {row}"
    );

    let loans_only = retarget(
        &notice.holder_token,
        &secret,
        AccountTypeSlug::Reader,
        UserRights {
            items_rights: Rights::Read,
            loans_rights: Rights::Read,
            users_rights: Rights::Own,
            holds_rights: Rights::Own,
            ..UserRights::default()
        },
    );
    let (status, loans_view) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&loans_only)).await;
    assert_eq!(status, StatusCode::OK, "{loans_view}");
    assert_staff_copies(&loans_view, &notice);
    assert!(
        copy_by_id(&loans_view, notice.held_id).expect("held").get("holdFor").is_none(),
        "loans read without holds rights omits holdFor: {loans_view}"
    );

    let users_read = retarget(
        &notice.holder_token,
        &secret,
        AccountTypeSlug::Group,
        UserRights {
            items_rights: Rights::Read,
            users_rights: Rights::Read,
            loans_rights: Rights::None,
            holds_rights: Rights::None,
            ..UserRights::default()
        },
    );
    let (status, users_view) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&users_read)).await;
    assert_eq!(status, StatusCode::OK, "{users_view}");
    assert!(
        copy_by_id(&users_view, notice.held_id).expect("held").get("holdFor").is_none(),
        "users read without holds rights omits holdFor: {users_view}"
    );

    let holds_only = retarget(
        &notice.holder_token,
        &secret,
        AccountTypeSlug::Reader,
        UserRights {
            items_rights: Rights::Read,
            holds_rights: Rights::Read,
            users_rights: Rights::None,
            loans_rights: Rights::None,
            ..UserRights::default()
        },
    );
    let (status, holds_view) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&holds_only)).await;
    assert_eq!(status, StatusCode::OK, "{holds_view}");
    assert_minimized_hold_for(&holds_view, &notice);

    let holds_and_users = retarget(
        &notice.holder_token,
        &secret,
        AccountTypeSlug::Reader,
        UserRights {
            items_rights: Rights::Read,
            holds_rights: Rights::Read,
            users_rights: Rights::Read,
            loans_rights: Rights::None,
            ..UserRights::default()
        },
    );
    let (status, full_view) = get(&app, &format!("/api/v1/biblios/{}", notice.biblio_id), Some(&holds_and_users)).await;
    assert_eq!(status, StatusCode::OK, "{full_view}");
    assert_full_hold_for(&full_view, &notice);

    let (status, patron_holds) = get(&app, "/api/v1/holds", Some(&notice.holder_token)).await;
    assert_eq!(status, StatusCode::OK, "{patron_holds}");
    let patron_row = patron_holds["items"]
        .as_array()
        .expect("holds")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == notice.hold_id)
        .expect("patron hold");
    assert!(patron_row["biblio"].get("weedingStatus").is_none(), "{patron_row}");
    assert!(patron_row["user"].get("login").is_none(), "{patron_row}");

    let holds_desk = retarget(
        &admin_token,
        &secret,
        AccountTypeSlug::Reader,
        UserRights {
            items_rights: Rights::Read,
            holds_rights: Rights::Read,
            users_rights: Rights::None,
            loans_rights: Rights::None,
            ..UserRights::default()
        },
    );
    let (status, desk_holds) = get(&app, "/api/v1/holds", Some(&holds_desk)).await;
    assert_eq!(status, StatusCode::OK, "{desk_holds}");
    let desk_row = desk_holds["items"]
        .as_array()
        .expect("holds")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == notice.hold_id)
        .expect("desk hold");
    assert!(desk_row["biblio"].get("weedingStatus").is_some(), "{desk_row}");
    assert_eq!(desk_row["user"]["cardNumber"], notice.card, "{desk_row}");
    assert_eq!(desk_row["user"]["initials"], "HH", "{desk_row}");
    assert!(desk_row["user"].get("firstname").is_none(), "{desk_row}");
}
