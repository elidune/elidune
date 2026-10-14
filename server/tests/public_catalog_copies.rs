//! Public, patron, and staff catalog copies (#196).
//!
//! OPAC and patron lists omit lost, declared missing, weeding-in-progress, and
//! withdrawn copies. The public count is the length of that list. Each remaining
//! copy is `available`, `onLoan`, `onSiteOnly`, or `unavailable`. Staff lists
//! keep every copy and include `circulationStatus`.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

struct Copies {
    biblio_id: i64,
    title: String,
    available: i64,
    on_loan: i64,
    on_site: i64,
    damaged: i64,
    to_search: i64,
    in_transit: i64,
    lost: i64,
    missing: i64,
    weeding: i64,
    withdrawn: i64,
    lost_barcode: String,
    patron_name: String,
}

fn copy(barcode: &str, borrowable: bool) -> Value {
    json!({
        "barcode": barcode,
        "borrowable": borrowable,
        "callNumber": barcode
    })
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

fn find_copy(body: &Value, item_id: i64) -> &Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == item_id)
        .unwrap_or_else(|| panic!("copy {item_id} missing from {body}"))
}

fn absent(body: &Value, item_id: i64) {
    let present = body["items"]
        .as_array()
        .expect("items")
        .iter()
        .any(|item| fixtures::json_id(&item["id"]) == item_id);
    assert!(!present, "copy {item_id} should be omitted: {body}");
}

fn assert_public_shape(copy: &Value) {
    for key in [
        "borrower",
        "holdFor",
        "holdUserId",
        "holdUserName",
        "loanId",
        "circulationStatus",
        "weedingStatus",
        "weedingReason",
        "notes",
        "internalNote",
        "login",
    ] {
        assert!(copy.get(key).is_none(), "{key} leaked: {copy}");
    }
}

async fn seed(app: &TestApp) -> (String, String, Copies) {
    let admin_token = fixtures::ensure_first_setup(app).await;
    let (holder_id, holder_token) = fixtures::create_reader(app, &admin_token, "pubhold").await;
    let (borrower_id, _) = fixtures::create_reader(app, &admin_token, "publoan").await;
    let patron_name = "Nola Secret".to_string();
    sqlx::query("UPDATE users SET firstname = 'Nola', lastname = 'Secret' WHERE id = $1")
        .bind(holder_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("name holder");

    let origin = create_site(app, &admin_token, "PubOrigin").await;
    let pickup = create_site(app, &admin_token, "PubPickup").await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Public copies {suffix}");
    let transit_barcode = format!("PUB-TR-{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": transit_barcode,
                    "borrowable": true,
                    "sourceId": origin.to_string()
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let in_transit = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

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
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");
    assert_eq!(hold["status"], "to_pick");
    sqlx::query("UPDATE holds SET pickup_site_id = $2 WHERE id = $1")
        .bind(fixtures::json_id(&hold["id"]))
        .bind(pickup)
        .execute(app.state.services.repository.pool())
        .await
        .expect("pickup site");
    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{transit_barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "transit scan: {scan}");
    assert_eq!(scan["status"], "in_transit");

    let available = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-AV-{suffix}"),
        true,
    )
    .await;
    let on_loan = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-LN-{suffix}"),
        true,
    )
    .await;
    let on_site = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-OS-{suffix}"),
        false,
    )
    .await;
    let damaged = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-DM-{suffix}"),
        true,
    )
    .await;
    let to_search = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-TS-{suffix}"),
        true,
    )
    .await;
    let lost_barcode = format!("PUB-LO-{suffix}");
    let lost = add_copy(app, &admin_token, biblio_id, &lost_barcode, true).await;
    let missing = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-MI-{suffix}"),
        true,
    )
    .await;
    let weeding = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-WD-{suffix}"),
        true,
    )
    .await;
    let withdrawn = add_copy(
        app,
        &admin_token,
        biblio_id,
        &format!("PUB-WN-{suffix}"),
        true,
    )
    .await;

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": borrower_id.to_string(),
                "itemId": on_loan.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan}");

    for (id, status_code) in [(damaged, 2_i16), (to_search, 4), (lost, 1), (missing, 5)] {
        sqlx::query("UPDATE items SET circulation_status = $2 WHERE id = $1")
            .bind(id)
            .bind(status_code)
            .execute(app.state.services.repository.pool())
            .await
            .expect("circulation status");
    }

    let (status, candidate) = app
        .post_json(
            &format!("/api/v1/items/{weeding}/weeding"),
            &json!({ "status": "candidate", "reason": "pilon in progress" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "candidate: {candidate}");
    let (status, withdrawal) = app
        .post_json(
            &format!("/api/v1/items/{withdrawn}/withdrawals"),
            &json!({ "reason": "worn", "destination": "pulp" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {withdrawal}");

    (
        admin_token,
        holder_token,
        Copies {
            biblio_id,
            title,
            available,
            on_loan,
            on_site,
            damaged,
            to_search,
            in_transit,
            lost,
            missing,
            weeding,
            withdrawn,
            lost_barcode,
            patron_name,
        },
    )
}

fn assert_public_copies(body: &Value, copies: &Copies) {
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 6, "public count is the filtered list: {body}");
    assert_eq!(
        find_copy(body, copies.available)["availability"],
        "available"
    );
    assert_eq!(find_copy(body, copies.on_loan)["availability"], "onLoan");
    assert_eq!(
        find_copy(body, copies.on_site)["availability"],
        "onSiteOnly"
    );
    assert_eq!(
        find_copy(body, copies.damaged)["availability"],
        "unavailable"
    );
    assert_eq!(
        find_copy(body, copies.to_search)["availability"],
        "unavailable"
    );
    assert_eq!(
        find_copy(body, copies.in_transit)["availability"],
        "unavailable"
    );
    assert_eq!(find_copy(body, copies.available)["canHoldItem"], true);
    assert_eq!(find_copy(body, copies.on_loan)["canHoldItem"], true);
    assert_eq!(find_copy(body, copies.on_site)["canHoldItem"], false);
    assert_eq!(find_copy(body, copies.damaged)["canHoldItem"], false);
    for id in [
        copies.available,
        copies.on_loan,
        copies.on_site,
        copies.damaged,
        copies.to_search,
        copies.in_transit,
    ] {
        assert_public_shape(find_copy(body, id));
    }
    for id in [
        copies.lost,
        copies.missing,
        copies.weeding,
        copies.withdrawn,
    ] {
        absent(body, id);
    }
    let raw = body.to_string();
    assert!(!raw.contains(&copies.patron_name));
    assert!(!raw.contains("pilon in progress"));
    assert!(!raw.contains("holdFor"));
    assert!(!raw.contains("borrower"));
}

fn assert_staff_copies(body: &Value, copies: &Copies) {
    let items = body["items"].as_array().expect("items");
    assert_eq!(items.len(), 10, "staff keep the real total: {body}");
    assert_eq!(
        find_copy(body, copies.available)["availability"],
        "available"
    );
    assert_eq!(find_copy(body, copies.on_loan)["availability"], "onLoan");
    assert_eq!(
        find_copy(body, copies.on_site)["availability"],
        "unavailable"
    );
    assert_eq!(
        find_copy(body, copies.damaged)["availability"],
        "unavailable"
    );
    assert_eq!(
        find_copy(body, copies.to_search)["availability"],
        "toSearch"
    );
    assert_eq!(
        find_copy(body, copies.in_transit)["availability"],
        "inTransit"
    );
    assert_eq!(find_copy(body, copies.lost)["availability"], "unavailable");
    assert_eq!(
        find_copy(body, copies.missing)["availability"],
        "unavailable"
    );
    assert_eq!(find_copy(body, copies.weeding)["availability"], "available");
    assert_eq!(
        find_copy(body, copies.withdrawn)["availability"],
        "unavailable"
    );
    assert!(find_copy(body, copies.available)["circulationStatus"].is_null());
    assert_eq!(find_copy(body, copies.lost)["circulationStatus"], 1);
    assert_eq!(find_copy(body, copies.damaged)["circulationStatus"], 2);
    assert_eq!(find_copy(body, copies.to_search)["circulationStatus"], 4);
    assert_eq!(find_copy(body, copies.missing)["circulationStatus"], 5);
}

fn search_hit(body: &Value, biblio_id: i64) -> &Value {
    body["items"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == biblio_id)
        .expect("search hit")
}

#[tokio::test]
async fn public_opac_omits_lost_missing_and_weeding_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (_admin, _patron, copies) = seed(&app).await;

    let (status, detail) = app
        .get_json(&format!("/api/v1/opac/biblios/{}", copies.biblio_id))
        .await;
    assert_eq!(status, StatusCode::OK, "opac detail: {detail}");
    assert_public_copies(&detail, &copies);

    let (status, search) = app
        .get_json(&format!(
            "/api/v1/opac/biblios?title={}&perPage=5",
            copies.title.replace(' ', "%20")
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "opac search: {search}");
    assert_public_copies(search_hit(&search, copies.biblio_id), &copies);
}

#[tokio::test]
async fn patron_catalog_omits_lost_missing_and_weeding_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (_admin_token, patron_token, copies) = seed(&app).await;

    let (status, detail) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{}", copies.biblio_id),
            &patron_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "patron detail: {detail}");
    assert_public_copies(&detail, &copies);

    let (status, search) = app
        .get_json_with_auth(
            &format!(
                "/api/v1/biblios?title={}&perPage=5",
                copies.title.replace(' ', "%20")
            ),
            &patron_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "patron search: {search}");
    assert_public_copies(search_hit(&search, copies.biblio_id), &copies);

    let (status, listed) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{}/items", copies.biblio_id),
            &patron_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "patron items: {listed}");
    let wrapped = json!({ "items": listed });
    assert_public_copies(&wrapped, &copies);

    let (status, hidden) = app
        .get_json_with_auth(&format!("/api/v1/items/{}", copies.lost), &patron_token)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "lost item: {hidden}");
    let (status, hidden_barcode) = app
        .get_json_with_auth(
            &format!("/api/v1/items/barcode/{}", copies.lost_barcode),
            &patron_token,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "lost barcode: {hidden_barcode}"
    );

    let (status, visible) = app
        .get_json_with_auth(
            &format!("/api/v1/items/{}", copies.available),
            &patron_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "available item: {visible}");
    assert_eq!(
        find_copy(&visible, copies.available)["availability"],
        "available"
    );
    assert_public_shape(find_copy(&visible, copies.available));
}

#[tokio::test]
async fn staff_catalog_keeps_every_copy_and_circulation_status() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (admin_token, _patron, copies) = seed(&app).await;

    let (status, detail) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{}", copies.biblio_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "staff detail: {detail}");
    assert_staff_copies(&detail, &copies);

    let (status, search) = app
        .get_json_with_auth(
            &format!(
                "/api/v1/biblios?title={}&perPage=5",
                copies.title.replace(' ', "%20")
            ),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "staff search: {search}");
    assert_staff_copies(search_hit(&search, copies.biblio_id), &copies);

    let (status, listed) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{}/items", copies.biblio_id),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "staff items: {listed}");
    assert_staff_copies(&json!({ "items": listed }), &copies);

    let (status, lost) = app
        .get_json_with_auth(&format!("/api/v1/items/{}", copies.lost), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "staff lost item: {lost}");
    assert_eq!(find_copy(&lost, copies.lost)["circulationStatus"], 1);
    assert_eq!(find_copy(&lost, copies.lost)["availability"], "unavailable");
}
