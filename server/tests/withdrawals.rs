//! Weeding withdrawals (#139): event rows, blockers, rights, legacy import, OPAC.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::fixtures;
use common::TestApp;
use http_body_util::BodyExt;
use serde_json::{json, Value};

const CSV_HEADER: &str = "Code-barres;Titre;Auteur;Cote;Site;Date d'acquisition;Prix d'achat;Motif;Précision;Destination;Date de retrait;Retiré par;Éliminé le;Éliminé par";

async fn unlock_admin(app: &TestApp, admin_token: &str) {
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", admin_token).await;
    if status != StatusCode::OK {
        return;
    }
    let admin_id = fixtures::json_id(&me["id"]);
    let _ = app.state.services.users.set_must_change_password(admin_id, false).await;
}

async fn create_title(app: &TestApp, admin_token: &str, title: &str, copies: usize) -> (i64, Vec<i64>) {
    let items: Vec<Value> = (0..copies)
        .map(|i| {
            json!({
                "barcode": format!("WV-{}-{i}", fixtures::unique_suffix()),
                "borrowable": true
            })
        })
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
    let item_ids = body["biblio"]["items"].as_array().expect("items").iter().map(|item| fixtures::json_id(&item["id"])).collect();
    (biblio_id, item_ids)
}

fn withdraw_body(reason: &str, destination: &str) -> Value {
    json!({ "reason": reason, "destination": destination })
}

async fn withdraw(app: &TestApp, token: &str, item_id: i64, reason: &str, destination: &str) -> (StatusCode, Value) {
    app.post_json(&format!("/api/v1/items/{item_id}/withdrawals"), &withdraw_body(reason, destination), Some(token)).await
}

async fn patch_json(app: &TestApp, uri: &str, body: &Value, token: &str) -> (StatusCode, Value) {
    let response = app
        .request(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get_text(app: &TestApp, uri: &str, token: &str) -> (StatusCode, String, Option<String>) {
    let response = app
        .request(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    let status = response.status();
    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).map(str::to_string);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap_or_default();
    (status, text, content_type)
}

fn enc(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn snowflake() -> i64 {
    fixtures::unique_suffix() as i64
}

async fn insert_source(app: &TestApp, name: &str) -> i64 {
    sqlx::query_scalar(r#"INSERT INTO sources (name, is_archive, "default") VALUES ($1, 0, FALSE) RETURNING id"#)
        .bind(name)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("insert source")
}

#[tokio::test]
async fn withdraw_validates_reason_and_refuses_loan_transit_and_pickup_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "wvblock").await;
    let (_, item_ids) = create_title(&app, &admin_token, "Withdrawal blockers", 4).await;

    let (status, detail) = app
        .post_json(
            &format!("/api/v1/items/{}/withdrawals", item_ids[0]),
            &json!({ "reason": "other", "reasonDetail": "no", "destination": "pulp" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "short detail: {detail}");
    assert_eq!(detail["code"], "withdrawal_reason_detail_required");

    let (status, dest) = app
        .post_json(&format!("/api/v1/items/{}/withdrawals", item_ids[0]), &json!({ "reason": "worn" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "destination: {dest}");
    assert_eq!(dest["code"], "withdrawal_destination_required");

    let (status, forbidden) = withdraw(&app, &reader_token, item_ids[0], "worn", "pulp").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader: {forbidden}");
    assert_eq!(forbidden["code"], "authorization_failed");

    let (status, loan_body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_ids[0].to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan_body}");
    let (status, on_loan) = withdraw(&app, &admin_token, item_ids[0], "worn", "pulp").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "on loan: {on_loan}");
    assert_eq!(on_loan["code"], "item_on_loan");

    let from_site: i64 = sqlx::query_scalar("SELECT id FROM sources ORDER BY id LIMIT 1")
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("source");
    let to_site = insert_source(&app, &format!("Transit {}", fixtures::unique_suffix())).await;
    sqlx::query(
        r#"
        INSERT INTO item_transits (id, item_id, from_site_id, to_site_id, status)
        VALUES ($1, $2, $3, $4, 'in_transit')
        "#,
    )
    .bind(snowflake())
    .bind(item_ids[1])
    .bind(from_site)
    .bind(to_site)
    .execute(app.state.services.repository.pool())
    .await
    .expect("transit");
    let (status, transit) = withdraw(&app, &admin_token, item_ids[1], "worn", "sale").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "transit: {transit}");
    assert_eq!(transit["code"], "item_in_transit");

    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "wvhold").await;
    let (held_biblio, held_items) = create_title(&app, &admin_token, "Pickup hold blocks weeding", 1).await;
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": holder_id.to_string(),
                "biblioId": held_biblio.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");
    assert!(hold["status"] == "to_pick" || hold["status"] == "ready", "pickup hold: {hold}");
    assert_eq!(fixtures::json_id(&hold["itemId"]), held_items[0]);
    let (status, on_hold) = withdraw(&app, &admin_token, held_items[0], "surplus", "donation").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "hold: {on_hold}");
    assert_eq!(on_hold["code"], "item_on_hold");

    let (status, created) = withdraw(&app, &admin_token, item_ids[3], "worn", "pulp").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {created}");
    assert_eq!(created["reason"], "worn");
    assert_eq!(created["destination"], "pulp");
    assert_eq!(created["legacy"], false);
    assert!(created["cancelledAt"].is_null());

    let (status, again) = withdraw(&app, &admin_token, item_ids[3], "obsolete", "deposit").await;
    assert_eq!(status, StatusCode::CONFLICT, "second withdraw: {again}");
    assert_eq!(again["code"], "withdrawal_already_active");

    let (status, via_weeding) = app
        .post_json(&format!("/api/v1/items/{}/weeding", item_ids[3]), &json!({ "status": "onShelf" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "undo via weeding: {via_weeding}");
    assert_eq!(via_weeding["code"], "withdrawal_use_cancel");

    let withdrawal_id = fixtures::json_id(&created["id"]);
    let (status, not_legacy) = patch_json(
        &app,
        &format!("/api/v1/item-withdrawals/{withdrawal_id}"),
        &json!({ "reason": "worn", "destination": "donation" }),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "complete: {not_legacy}");
    assert_eq!(not_legacy["code"], "withdrawal_not_legacy");
}

#[tokio::test]
async fn batch_withdraw_rolls_back_when_any_copy_is_blocked() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "wvbatch").await;
    let (_, item_ids) = create_title(&app, &admin_token, "Batch withdrawal", 2).await;

    let (status, loan_body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_ids[1].to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan_body}");

    let (status, batch) = app
        .post_json(
            "/api/v1/items/withdrawals",
            &json!({
                "itemIds": [item_ids[0].to_string(), item_ids[1].to_string()],
                "reason": "obsolete",
                "destination": "pulp"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "batch: {batch}");
    assert_eq!(batch["code"], "item_on_loan");

    let shelf: String = sqlx::query_scalar("SELECT weeding_status FROM items WHERE id = $1")
        .bind(item_ids[0])
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("status");
    assert_eq!(shelf, "on_shelf", "a failed batch must not withdraw the first copy");

    let (status, empty) = app
        .post_json("/api/v1/items/withdrawals", &json!({ "itemIds": [], "reason": "worn", "destination": "pulp" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "empty: {empty}");
    assert_eq!(empty["code"], "validation_error");

    let (status, dup) = app
        .post_json(
            "/api/v1/items/withdrawals",
            &json!({
                "itemIds": [item_ids[0].to_string(), item_ids[0].to_string()],
                "reason": "worn",
                "destination": "pulp"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "duplicate: {dup}");
    assert_eq!(dup["code"], "validation_error");
}

#[tokio::test]
async fn cancel_withdrawal_uses_a_separate_right_and_restores_the_shelf() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "wvcancel").await;
    let (_, item_ids) = create_title(&app, &admin_token, "Cancel withdrawal", 2).await;

    let (status, created) = withdraw(&app, &admin_token, item_ids[0], "newEdition", "donation").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {created}");
    let withdrawal_id = fixtures::json_id(&created["id"]);

    let (status, reader_cancel) = app.post_empty(&format!("/api/v1/item-withdrawals/{withdrawal_id}/cancel"), Some(&reader_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader cancel: {reader_cancel}");
    assert_eq!(reader_cancel["code"], "authorization_failed");

    let (status, group_before) = app.get_json_with_auth("/api/v1/account-types/group", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "group type: {group_before}");
    let (status, _) = app
        .put_json("/api/v1/account-types/group", &json!({ "weedingRights": "w", "weedingCancelRights": "n" }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "grant weeding without cancel");

    let (group_user, _) = fixtures::create_reader(&app, &admin_token, "wvgroup").await;
    sqlx::query("UPDATE users SET account_type = 'group' WHERE id = $1")
        .bind(group_user)
        .execute(app.state.services.repository.pool())
        .await
        .expect("group account");
    let login = sqlx::query_scalar::<_, String>("SELECT login FROM users WHERE id = $1")
        .bind(group_user)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("login");
    let (status, session) = app.post_json("/api/v1/auth/login", &json!({ "username": login, "password": "readerpass1234" }), None).await;
    assert_eq!(status, StatusCode::OK, "group login: {session}");
    let group_token = session["token"].as_str().expect("token").to_string();

    let (status, group_cancel) = app.post_empty(&format!("/api/v1/item-withdrawals/{withdrawal_id}/cancel"), Some(&group_token)).await;
    let (status_withdraw, group_withdraw) = withdraw(&app, &group_token, item_ids[1], "surplus", "sale").await;

    let _ = app
        .put_json(
            "/api/v1/account-types/group",
            &json!({
                "weedingRights": group_before["weedingRights"],
                "weedingCancelRights": group_before["weedingCancelRights"]
            }),
            Some(&admin_token),
        )
        .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "cancel right: {group_cancel}");
    assert_eq!(group_cancel["code"], "authorization_failed");
    assert_eq!(status_withdraw, StatusCode::CREATED, "weeding right: {group_withdraw}");

    let (status, cancelled) = app.post_empty(&format!("/api/v1/item-withdrawals/{withdrawal_id}/cancel"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "admin cancel: {cancelled}");
    assert!(cancelled["cancelledAt"].is_string());
    let weeding: String = sqlx::query_scalar("SELECT weeding_status FROM items WHERE id = $1")
        .bind(item_ids[0])
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("status");
    let reason: Option<String> = sqlx::query_scalar("SELECT weeding_reason FROM items WHERE id = $1")
        .bind(item_ids[0])
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reason");
    assert_eq!(weeding, "on_shelf");
    assert!(reason.is_none());

    let (status, again) = withdraw(&app, &admin_token, item_ids[0], "worn", "pulp").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw again: {again}");
}

#[tokio::test]
async fn legacy_backfill_exports_an_unspecified_destination_and_can_be_completed() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let suffix = fixtures::unique_suffix();
    let (_, item_ids) = create_title(&app, &admin_token, &format!("Legacy weeding {suffix}"), 1).await;
    let item_id = item_ids[0];
    let shelf = format!("LEG-{suffix}");
    let site_id = insert_source(&app, &format!("Legacy site {suffix}")).await;
    sqlx::query(
        r#"
        UPDATE items
        SET weeding_status = 'withdrawn',
            weeding_reason = 'old free text',
            call_number = $2,
            site_id = $3
        WHERE id = $1
        "#,
    )
    .bind(item_id)
    .bind(&shelf)
    .bind(site_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("mark legacy withdrawn");

    sqlx::query(
        r#"
        INSERT INTO item_withdrawals (
            id, item_id, withdrawn_at, reason, reason_detail, destination, agent_id, legacy,
            disposed_at, disposed_by
        )
        SELECT
            (SELECT COALESCE(MAX(id), 0) FROM item_withdrawals)
                + ROW_NUMBER() OVER (ORDER BY i.id),
            i.id,
            COALESCE(i.updated_at, i.created_at, NOW()),
            'other',
            CASE
                WHEN i.weeding_reason IS NOT NULL AND char_length(btrim(i.weeding_reason)) >= 3
                    THEN btrim(i.weeding_reason)
                WHEN i.weeding_reason IS NOT NULL AND char_length(btrim(i.weeding_reason)) > 0
                    THEN btrim(i.weeding_reason) || ' (reprise)'
                ELSE 'reprise'
            END,
            NULL,
            NULL,
            TRUE,
            COALESCE(i.updated_at, i.created_at, NOW()),
            NULL
        FROM items i
        WHERE i.weeding_status = 'withdrawn'
          AND NOT EXISTS (
              SELECT 1 FROM item_withdrawals w
              WHERE w.item_id = i.id AND w.cancelled_at IS NULL
          )
        "#,
    )
    .execute(app.state.services.repository.pool())
    .await
    .expect("backfill");

    let (status, list) = app
        .get_json_with_auth(&format!("/api/v1/item-withdrawals?callNumber={}&destination=unset", enc(&shelf)), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "legacy list: {list}");
    assert_eq!(list["total"], 1);
    let row = &list["items"][0];
    assert_eq!(row["reason"], "other");
    assert_eq!(row["reasonDetail"], "old free text");
    assert!(row["destination"].is_null());
    assert_eq!(row["legacy"], true);
    assert_eq!(row["callNumber"], shelf);
    assert_eq!(row["disposedAt"], row["withdrawnAt"]);
    assert!(row["disposedBy"].is_null());
    assert!(row["disposedByName"].is_null());

    let (status, missing) = app.get_json_with_auth("/api/v1/item-withdrawals/export.csv", &admin_token).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "export period: {missing}");
    assert_eq!(missing["code"], "validation_error");

    let query = format!("from={}&to={}&callNumber={}", enc("2020-01-01T00:00:00Z"), enc("2035-01-01T00:00:00Z"), enc(&shelf));
    let (status, french, content_type) = get_text(&app, &format!("/api/v1/item-withdrawals/export.csv?{query}&lang=french"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "french csv: {french}");
    assert!(content_type.unwrap_or_default().contains("text/csv"), "content type");
    assert!(french.contains("non renseignée (reprise)"), "{french}");
    assert!(french.contains(CSV_HEADER), "{french}");
    assert!(!french.contains('_'), "{french}");
    assert!(french.starts_with('\u{feff}'));
    let french_line = french.trim_start_matches('\u{feff}').lines().nth(1).expect("french data");
    let french_cols: Vec<&str> = french_line.split(';').collect();
    assert_eq!(french_cols.len(), 14, "{french_line}");
    assert_eq!(french_cols[1], format!("Legacy weeding {suffix}"));
    assert_eq!(french_cols[2], "");
    assert_eq!(french_cols[3], shelf);
    assert_eq!(french_cols[4], format!("Legacy site {suffix}"));
    assert_eq!(french_cols[6], "", "unknown price is an empty cell");
    assert_eq!(french_cols[9], "non renseignée (reprise)");

    let (status, english, _) = get_text(&app, &format!("/api/v1/item-withdrawals/export.csv?{query}&lang=english"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "english csv: {english}");
    assert!(english.contains("not specified (legacy import)"), "{english}");
    assert!(english.contains(CSV_HEADER), "{english}");
    let english_line = english.trim_start_matches('\u{feff}').lines().nth(1).expect("english data");
    let english_cols: Vec<&str> = english_line.split(';').collect();
    assert_eq!(english_cols[6], "");
    assert_eq!(english_cols[9], "not specified (legacy import)");
    assert!(!english.contains("Withdrawn on"), "{english}");
    assert!(!english.contains('_'), "{english}");

    let withdrawal_id = fixtures::json_id(&row["id"]);
    let (status, completed) = patch_json(
        &app,
        &format!("/api/v1/item-withdrawals/{withdrawal_id}"),
        &json!({
            "reason": "other",
            "reasonDetail": "old free text",
            "destination": "donation"
        }),
        &admin_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "complete legacy: {completed}");
    assert_eq!(completed["destination"], "donation");
    assert_eq!(completed["legacy"], true);
    assert_eq!(completed["reasonDetail"], "old free text");
}

#[tokio::test]
async fn staff_list_filters_by_period_site_reason_destination_and_totals() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let suffix = fixtures::unique_suffix();
    let (biblio_id, item_ids) = create_title(&app, &admin_token, &format!("Filter weeding {suffix}"), 2).await;
    let site_id = insert_source(&app, &format!("Filter site {suffix}")).await;
    let shelf_a = format!("FLT-{suffix}-A");
    let shelf_b = format!("FLT-{suffix}-B");
    for (item_id, shelf) in [(item_ids[0], &shelf_a), (item_ids[1], &shelf_b)] {
        sqlx::query("UPDATE items SET site_id = $2, call_number = $3 WHERE id = $1")
            .bind(item_id)
            .bind(site_id)
            .bind(shelf)
            .execute(app.state.services.repository.pool())
            .await
            .expect("shelf");
    }
    let (status, collection) = app
        .post_json(
            "/api/v1/collections",
            &json!({
                "name": format!("Weed col {suffix}"),
                "key": format!("weed-{suffix}")
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "collection: {collection}");
    let collection_id = fixtures::json_id(&collection["id"]);
    sqlx::query("INSERT INTO biblio_collections (biblio_id, collection_id, position) VALUES ($1, $2, 1)")
        .bind(biblio_id)
        .bind(collection_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("link collection");

    let (status, worn) = withdraw(&app, &admin_token, item_ids[0], "worn", "pulp").await;
    assert_eq!(status, StatusCode::CREATED, "worn: {worn}");
    let (status, obsolete) = withdraw(&app, &admin_token, item_ids[1], "obsolete", "donation").await;
    assert_eq!(status, StatusCode::CREATED, "obsolete: {obsolete}");

    let base = format!("from={}&to={}&siteId={site_id}", enc("2020-01-01T00:00:00Z"), enc("2035-01-01T00:00:00Z"));
    let (status, listed) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{base}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "list: {listed}");
    assert_eq!(listed["total"], 2);
    let reasons = listed["totalsByReason"].as_array().expect("reasons");
    assert!(reasons.iter().any(|row| row["reason"] == "worn" && row["count"] == 1));
    assert!(reasons.iter().any(|row| row["reason"] == "obsolete" && row["count"] == 1));
    let destinations = listed["totalsByDestination"].as_array().expect("destinations");
    assert!(destinations.iter().any(|row| row["destination"] == "pulp" && row["count"] == 1));
    assert!(destinations.iter().any(|row| row["destination"] == "donation" && row["count"] == 1));

    let (status, worn_only) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{base}&reason=worn"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "reason filter: {worn_only}");
    assert_eq!(worn_only["total"], 1);
    assert_eq!(worn_only["totalsByReason"][0]["reason"], "worn");
    assert_eq!(worn_only["totalsByReason"][0]["count"], 1);

    let (status, donated) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{base}&destination=donation"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "destination filter: {donated}");
    assert_eq!(donated["total"], 1);
    assert_eq!(donated["items"][0]["reason"], "obsolete");

    let (status, by_shelf) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{base}&callNumber={}", enc(&shelf_a)), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "shelf: {by_shelf}");
    assert_eq!(by_shelf["total"], 1);
    assert_eq!(by_shelf["items"][0]["callNumber"], shelf_a);

    let (status, by_collection) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{base}&collectionId={collection_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "collection: {by_collection}");
    assert_eq!(by_collection["total"], 2);
}

#[tokio::test]
async fn delete_refuses_history_and_a_biblio_keeps_withdrawn_copies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "wvdel").await;

    let (_loan_biblio, loan_items) = create_title(&app, &admin_token, "Delete loan history", 1).await;
    let (status, loan_body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": loan_items[0].to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {loan_body}");
    let loan_id = fixtures::json_id(&loan_body["id"]);
    sqlx::query(
        r#"
        INSERT INTO fines (id, loan_id, user_id, amount, status, charge_type)
        VALUES ($1, $2, $3, 150, 'pending', 'overdue')
        "#,
    )
    .bind(snowflake())
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("fine");
    let (status, refused) = app.delete_with_auth(&format!("/api/v1/items/{}?force=true", loan_items[0]), &admin_token).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "delete loan: {refused}");
    assert_eq!(refused["code"], "item_has_history_use_weeding");
    let message = refused["message"].as_str().unwrap_or_default();
    assert!(message.contains("loan"), "{refused}");
    assert!(message.contains("fine"), "{refused}");

    let (hold_biblio, hold_items) = create_title(&app, &admin_token, "Delete hold history", 1).await;
    let (holder_id, _) = fixtures::create_reader(&app, &admin_token, "wvdelh").await;
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": holder_id.to_string(),
                "biblioId": hold_biblio.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "title hold: {hold}");
    sqlx::query("UPDATE holds SET item_id = NULL WHERE biblio_id = $1")
        .bind(hold_biblio)
        .execute(app.state.services.repository.pool())
        .await
        .expect("unpin hold");
    let (status, clean_item) = app.delete_with_auth(&format!("/api/v1/items/{}", hold_items[0]), &admin_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "delete unpinned copy: {clean_item}");
    sqlx::query("DELETE FROM items WHERE biblio_id = $1")
        .bind(hold_biblio)
        .execute(app.state.services.repository.pool())
        .await
        .expect("remove archived copy row");
    let (status, biblio_holds) = app.delete_with_auth(&format!("/api/v1/biblios/{hold_biblio}?force=true"), &admin_token).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "biblio holds: {biblio_holds}");
    assert_eq!(biblio_holds["code"], "biblio_has_holds");

    let (weeded_biblio, weeded_items) = create_title(&app, &admin_token, "Delete withdrawn biblio", 1).await;
    let (status, withdrawn) = withdraw(&app, &admin_token, weeded_items[0], "outOfPolicy", "deposit").await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {withdrawn}");
    let still_counted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE id = $1 AND archived_at IS NULL AND weeding_status = 'withdrawn'")
        .bind(weeded_items[0])
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("count");
    assert_eq!(still_counted, 1, "a withdrawn copy stays in non-archived statistics");
    let (status, biblio_items) = app.delete_with_auth(&format!("/api/v1/biblios/{weeded_biblio}?force=true"), &admin_token).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "biblio items: {biblio_items}");
    assert_eq!(biblio_items["code"], "biblio_has_items");

    let (empty_biblio, empty_items) = create_title(&app, &admin_token, "Delete empty biblio", 1).await;
    sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(empty_items[0])
        .execute(app.state.services.repository.pool())
        .await
        .expect("drop only copy");
    let (status, deleted) = app.delete_with_auth(&format!("/api/v1/biblios/{empty_biblio}"), &admin_token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "empty biblio: {deleted}");
}

#[tokio::test]
async fn opac_hides_withdrawn_copies_and_never_exposes_the_reason() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "wvopac").await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Mixed weeding {suffix}");
    let (biblio_id, item_ids) = create_title(&app, &admin_token, &title, 2).await;
    let kept = item_ids[0];
    let weeded = item_ids[1];
    let barcode: String = sqlx::query_scalar("SELECT barcode FROM items WHERE id = $1")
        .bind(weeded)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("barcode");

    let (status, created) = app
        .post_json(
            &format!("/api/v1/items/{weeded}/withdrawals"),
            &json!({
                "reason": "other",
                "reasonDetail": format!("secret discard {suffix}"),
                "destination": "pulp"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {created}");

    let (status, staff) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "staff: {staff}");
    assert_eq!(staff["items"].as_array().expect("items").len(), 2);
    let staff_weeded = staff["items"].as_array().unwrap().iter().find(|item| fixtures::json_id(&item["id"]) == weeded).expect("staff copy");
    assert_eq!(staff_weeded["weedingStatus"], "withdrawn");
    assert!(staff_weeded["weedingReason"].is_null());

    let (status, reader) = app.get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "reader mixed biblio: {reader}");
    let public_items = reader["items"].as_array().expect("public items");
    assert_eq!(public_items.len(), 1);
    assert_eq!(fixtures::json_id(&public_items[0]["id"]), kept);
    assert!(reader.get("weedingStatus").is_none());
    assert!(!reader.to_string().contains(&format!("secret discard {suffix}")));
    assert!(public_items[0].get("weedingStatus").is_none());
    assert!(public_items[0].get("weedingReason").is_none());

    let (status, opac) = app.get_json(&format!("/api/v1/opac/biblios/{biblio_id}")).await;
    assert_eq!(status, StatusCode::OK, "opac mixed: {opac}");
    assert_eq!(opac["items"].as_array().expect("opac items").len(), 1);
    assert!(!opac.to_string().contains("weedingStatus"));
    assert!(!opac.to_string().contains(&format!("secret discard {suffix}")));

    let (status, by_item) = app.get_json_with_auth(&format!("/api/v1/items/{weeded}"), &reader_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "reader item: {by_item}");
    let (status, by_barcode) = app.get_json_with_auth(&format!("/api/v1/items/barcode/{barcode}"), &reader_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "reader barcode: {by_barcode}");
    let (status, staff_barcode) = app.get_json_with_auth(&format!("/api/v1/items/barcode/{barcode}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "staff barcode: {staff_barcode}");

    let encoded = enc(&title);
    let (status, reader_search) = app.get_json_with_auth(&format!("/api/v1/biblios?title={encoded}&perPage=5"), &reader_token).await;
    assert_eq!(status, StatusCode::OK, "reader search: {reader_search}");
    let hit = reader_search["items"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == biblio_id)
        .expect("mixed title stays searchable");
    assert_eq!(hit["items"].as_array().expect("copies").len(), 1);
}

fn dispose_failure(body: &Value, id: i64) -> &Value {
    body["failures"]
        .as_array()
        .expect("failures")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == id)
        .unwrap_or_else(|| panic!("no failure for {id}: {body}"))
}

#[tokio::test]
async fn dispose_batch_is_atomic_and_blocks_cancel() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &admin_token).await;
    assert_eq!(status, StatusCode::OK, "me: {me}");
    let admin_id = fixtures::json_id(&me["id"]);
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "wvdisp").await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Dispose batch {suffix}");
    let (biblio_id, item_ids) = create_title(&app, &admin_token, &title, 4).await;
    let shelf = format!("DSP-{suffix}");
    let fresh = item_ids[1];
    let to_cancel = item_ids[2];
    let to_dispose = item_ids[3];
    for item_id in &item_ids {
        sqlx::query("UPDATE items SET call_number = $2, price = CASE WHEN id = $3 THEN 0 ELSE price END WHERE id = $1")
            .bind(item_id)
            .bind(&shelf)
            .bind(fresh)
            .execute(app.state.services.repository.pool())
            .await
            .expect("shelf");
    }

    let (status, fresh_row) = withdraw(&app, &admin_token, fresh, "worn", "pulp").await;
    assert_eq!(status, StatusCode::CREATED, "fresh: {fresh_row}");
    assert!(fresh_row["disposedAt"].is_null());
    assert!(fresh_row["disposedBy"].is_null());
    let fresh_id = fixtures::json_id(&fresh_row["id"]);
    let (status, cancel_row) = withdraw(&app, &admin_token, to_cancel, "surplus", "donation").await;
    assert_eq!(status, StatusCode::CREATED, "cancel candidate: {cancel_row}");
    let cancel_id = fixtures::json_id(&cancel_row["id"]);
    let (status, disposed_row) = withdraw(&app, &admin_token, to_dispose, "obsolete", "sale").await;
    assert_eq!(status, StatusCode::CREATED, "dispose candidate: {disposed_row}");
    let disposed_id = fixtures::json_id(&disposed_row["id"]);

    let (status, forbidden) = app
        .post_json("/api/v1/item-withdrawals/dispose", &json!({ "withdrawalIds": [fresh_id.to_string()] }), Some(&reader_token))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "reader dispose: {forbidden}");
    assert_eq!(forbidden["code"], "authorization_failed");

    let (status, empty) = app.post_json("/api/v1/item-withdrawals/dispose", &json!({ "withdrawalIds": [] }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "empty dispose: {empty}");
    assert_eq!(empty["code"], "validation_error");

    let (status, duplicate) = app
        .post_json(
            "/api/v1/item-withdrawals/dispose",
            &json!({ "withdrawalIds": [fresh_id.to_string(), fresh_id.to_string()] }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "duplicate dispose: {duplicate}");
    assert_eq!(duplicate["code"], "validation_error");

    let (status, cancelled) = app.post_empty(&format!("/api/v1/item-withdrawals/{cancel_id}/cancel"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "cancel: {cancelled}");

    let (status, marked) = app
        .post_json("/api/v1/item-withdrawals/dispose", &json!({ "withdrawalIds": [disposed_id.to_string()] }), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "dispose one: {marked}");
    assert!(marked["withdrawals"][0]["disposedAt"].is_string());
    assert_eq!(fixtures::json_id(&marked["withdrawals"][0]["disposedBy"]), admin_id);
    assert!(marked["withdrawals"][0]["disposedByName"].is_string());

    let (status, mixed) = app
        .post_json(
            "/api/v1/item-withdrawals/dispose",
            &json!({
                "withdrawalIds": [
                    fresh_id.to_string(),
                    cancel_id.to_string(),
                    disposed_id.to_string(),
                    "-1"
                ]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "mixed: {mixed}");
    assert_eq!(mixed["code"], "withdrawal_dispose_rejected");
    assert_eq!(dispose_failure(&mixed, -1)["code"], "not_found");
    assert_eq!(dispose_failure(&mixed, cancel_id)["code"], "withdrawal_not_active");
    assert_eq!(dispose_failure(&mixed, disposed_id)["code"], "withdrawal_already_disposed");
    assert_eq!(mixed["failures"].as_array().expect("failures").len(), 3);

    let still_open: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar("SELECT disposed_at FROM item_withdrawals WHERE id = $1")
        .bind(fresh_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("fresh disposed_at");
    assert!(still_open.is_none(), "a rejected batch must not dispose the valid id");

    let (status, conflicts) = app
        .post_json(
            "/api/v1/item-withdrawals/dispose",
            &json!({
                "withdrawalIds": [cancel_id.to_string(), disposed_id.to_string()]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "conflicts: {conflicts}");
    assert_eq!(conflicts["code"], "withdrawal_dispose_rejected");
    assert_eq!(conflicts["failures"].as_array().expect("failures").len(), 2);

    let (status, dated) = app
        .post_json(
            "/api/v1/item-withdrawals/dispose",
            &json!({
                "withdrawalIds": [fresh_id.to_string()],
                "disposedAt": "2020-01-02T03:04:05Z"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "dated dispose: {dated}");
    let disposed_at = dated["withdrawals"][0]["disposedAt"].as_str().expect("date");
    assert!(disposed_at.starts_with("2020-01-02T03:04:05"), "{disposed_at}");

    let (status, blocked) = app.post_empty(&format!("/api/v1/item-withdrawals/{fresh_id}/cancel"), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::CONFLICT, "cancel disposed: {blocked}");
    assert_eq!(blocked["code"], "withdrawal_already_disposed");
    let weeding: String = sqlx::query_scalar("SELECT weeding_status FROM items WHERE id = $1")
        .bind(fresh)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("status");
    assert_eq!(weeding, "withdrawn");

    let list_query = format!("from={}&to={}&callNumber={}&disposed=", enc("2019-01-01T00:00:00Z"), enc("2035-01-01T00:00:00Z"), enc(&shelf));
    let (status, yes) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{list_query}true"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "disposed filter: {yes}");
    assert_eq!(yes["total"], 2, "{yes}");
    let (status, no) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals?{list_query}false"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "not disposed: {no}");
    assert_eq!(no["total"], 0, "{no}");

    let (status, csv, _) = get_text(&app, &format!("/api/v1/item-withdrawals/export.csv?{list_query}true&lang=english"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "csv: {csv}");
    assert!(csv.starts_with('\u{feff}'), "{csv}");
    assert!(csv.contains(CSV_HEADER), "{csv}");
    assert!(csv.contains("02/01/2020"), "{csv}");
    assert!(!csv.contains("2020-01-02T03:04:05"), "{csv}");
    let mut prices = Vec::new();
    for line in csv.trim_start_matches('\u{feff}').lines().skip(1) {
        let cols: Vec<&str> = line.split(';').collect();
        assert_eq!(cols.len(), 14, "{line}");
        prices.push(cols[6].to_string());
    }
    prices.sort();
    assert_eq!(prices, vec!["".to_string(), "0,00".to_string()], "null price stays empty and a stored 0 is 0,00: {csv}");

    let (status, opac) = app.get_json(&format!("/api/v1/opac/biblios/{biblio_id}")).await;
    assert_eq!(status, StatusCode::OK, "opac keeps the shelf copy: {opac}");
    let public = opac.to_string();
    assert!(!public.contains("disposedAt"), "{opac}");
    assert!(!public.contains("disposedBy"), "{opac}");
    assert!(!public.contains("2020-01-02T03:04:05"), "{opac}");
}

#[tokio::test]
async fn csv_neutralizes_formulas_and_uses_french_excel_layout() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (biblio_id, items) = create_title(&app, &admin_token, "Formula export", 1).await;
    let item_id = items[0];
    let shelf = format!("FORM-{}", fixtures::unique_suffix());
    let acquired_at = chrono::DateTime::parse_from_rfc3339("2026-03-15T23:30:00Z").expect("acquired").with_timezone(&chrono::Utc);
    sqlx::query("UPDATE items SET call_number = $2, price = 1250, created_at = $3 WHERE id = $1")
        .bind(item_id)
        .bind(&shelf)
        .bind(acquired_at)
        .execute(app.state.services.repository.pool())
        .await
        .expect("shelfmark");
    for (lastname, firstname, position) in [("Zola", "Emile", 2_i16), ("Hugo", "Victor", 1)] {
        let author_id: i64 = sqlx::query_scalar("INSERT INTO authors (lastname, firstname) VALUES ($1, $2) RETURNING id")
            .bind(lastname)
            .bind(firstname)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("author");
        sqlx::query("INSERT INTO biblio_authors (biblio_id, author_id, position) VALUES ($1, $2, $3)")
            .bind(biblio_id)
            .bind(author_id)
            .bind(position)
            .execute(app.state.services.repository.pool())
            .await
            .expect("biblio author");
    }

    let detail = r#"=HYPERLINK("http://evil")"#;
    let (status, created) = app
        .post_json(
            &format!("/api/v1/items/{item_id}/withdrawals"),
            &json!({
                "reason": "other",
                "reasonDetail": detail,
                "destination": "pulp"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "withdraw: {created}");
    let withdrawn_at = created["withdrawnAt"].as_str().expect("withdrawnAt");
    let instant = chrono::DateTime::parse_from_rfc3339(withdrawn_at).expect("rfc3339").with_timezone(&chrono::Utc);
    let expected_date = instant.with_timezone(&chrono_tz::Europe::Paris).format("%d/%m/%Y").to_string();

    let query = format!("from={}&to={}&callNumber={}&separator=comma", enc("2020-01-01T00:00:00Z"), enc("2035-01-01T00:00:00Z"), enc(&shelf));
    let (status, csv, _) = get_text(&app, &format!("/api/v1/item-withdrawals/export.csv?{query}&lang=french"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "csv: {csv}");
    assert!(csv.starts_with('\u{feff}'), "BOM: {csv}");
    let header = csv.trim_start_matches('\u{feff}').lines().next().expect("header");
    assert_eq!(header, CSV_HEADER);
    assert!(csv.contains("\"'=HYPERLINK(\"\"http://evil\"\")\""), "{csv}");
    assert!(csv.contains(&expected_date), "date {expected_date} in {csv}");
    assert!(!csv.contains(withdrawn_at), "no RFC3339 instant in {csv}");
    let data = csv.trim_start_matches('\u{feff}').lines().nth(1).expect("data");
    let cols: Vec<&str> = data.split(';').collect();
    assert_eq!(cols.len(), 14, "{data}");
    assert_eq!(cols[1], "Formula export");
    assert_eq!(cols[2], "Hugo Victor, Zola Emile");
    assert_eq!(cols[3], shelf);
    assert_eq!(cols[5], "16/03/2026");
    assert_eq!(cols[6], "12,50");
    assert_eq!(cols[10], expected_date);
}

#[tokio::test]
async fn withdrawal_fiche_is_readable_by_copy_and_by_id() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, item_ids) = create_title(&app, &admin_token, "Withdrawal fiche", 1).await;
    let item_id = item_ids[0];

    let (status, missing) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}/withdrawals"), &admin_token).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no active row: {missing}");

    let (status, created) = withdraw(&app, &admin_token, item_id, "worn", "pulp").await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("id");

    let (status, by_item) = app.get_json_with_auth(&format!("/api/v1/items/{item_id}/withdrawals"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{by_item}");
    assert_eq!(by_item["id"], id);
    assert!(by_item["disposedAt"].is_null());

    let (status, by_id) = app.get_json_with_auth(&format!("/api/v1/item-withdrawals/{id}"), &admin_token).await;
    assert_eq!(status, StatusCode::OK, "{by_id}");
    let item_id_str = item_id.to_string();
    assert_eq!(by_id["itemId"].as_str(), Some(item_id_str.as_str()));
    assert_eq!(by_id["reason"], "worn");
}

#[tokio::test]
async fn batch_withdraw_accepts_copies_from_different_biblios() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (_, first) = create_title(&app, &admin_token, "Batch biblio A", 1).await;
    let (_, second) = create_title(&app, &admin_token, "Batch biblio B", 1).await;

    let (status, body) = app
        .post_json(
            "/api/v1/items/withdrawals",
            &json!({
                "itemIds": [first[0].to_string(), second[0].to_string()],
                "reason": "surplus",
                "destination": "donation"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let rows = body["withdrawals"].as_array().expect("withdrawals");
    assert_eq!(rows.len(), 2);
    let item_ids: Vec<&str> = rows.iter().map(|row| row["itemId"].as_str().unwrap_or_default()).collect();
    let first_id = first[0].to_string();
    let second_id = second[0].to_string();
    assert!(item_ids.contains(&first_id.as_str()));
    assert!(item_ids.contains(&second_id.as_str()));
}
