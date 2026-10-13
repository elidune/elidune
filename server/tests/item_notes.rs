//! Physical-copy notes are split into `publicNote` and `internalNote` (#109).
//!
//! Staff catalog reads return both. Patrons and the OPAC receive `publicNote` only.
//! Former `items.notes` values live in `internal_note`.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn copy_by_barcode<'a>(body: &'a Value, barcode: &str) -> &'a Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["barcode"].as_str() == Some(barcode))
        .unwrap_or_else(|| panic!("copy {barcode} missing from {body}"))
}

fn assert_no_staff_note(body: &Value, secret: &str) {
    assert!(body.get("internalNote").is_none(), "{body}");
    assert!(body.get("notes").is_none(), "{body}");
    let raw = body.to_string();
    assert!(!raw.contains(secret), "{body}");
    assert!(!raw.contains("\"notes\""), "{body}");
    if let Some(items) = body.get("items").and_then(Value::as_array) {
        for item in items {
            assert!(item.get("internalNote").is_none(), "{item}");
            assert!(item.get("notes").is_none(), "{item}");
        }
    }
}

fn assert_public_copy(copy: &Value, public_note: &str, secret: &str) {
    assert_eq!(copy["publicNote"], public_note, "{copy}");
    assert!(copy.get("internalNote").is_none(), "{copy}");
    assert!(copy.get("notes").is_none(), "{copy}");
    assert!(!copy.to_string().contains(secret), "{copy}");
}

fn assert_staff_copy(copy: &Value, public_note: &str, internal_note: &str) {
    assert_eq!(copy["publicNote"], public_note, "{copy}");
    assert_eq!(copy["internalNote"], internal_note, "{copy}");
    assert!(copy.get("notes").is_none(), "{copy}");
}

#[tokio::test]
async fn migration_replaces_items_notes_with_public_and_internal() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'items'
          AND column_name IN ('notes', 'public_note', 'internal_note')
        ORDER BY column_name
        "#,
    )
    .fetch_all(app.state.services.repository.pool())
    .await
    .expect("item note columns");
    assert_eq!(
        columns,
        vec!["internal_note".to_string(), "public_note".to_string()]
    );
}

#[tokio::test]
async fn staff_sees_both_notes_patrons_and_opac_see_public_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "copynote").await;
    let suffix = fixtures::unique_suffix();
    let barcode = format!("NOTE-{suffix}");
    let public_note = format!("CD missing {suffix}");
    let internal_note = format!("spine cracked {suffix}");
    let title = format!("Copy notes {suffix}");

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
                    "publicNote": public_note,
                    "internalNote": internal_note
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let created_copy = copy_by_barcode(&created["biblio"], &barcode);
    assert_staff_copy(created_copy, &public_note, &internal_note);

    let (status, staff) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff}");
    assert_staff_copy(
        copy_by_barcode(&staff, &barcode),
        &public_note,
        &internal_note,
    );

    let (status, staff_items) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff_items}");
    assert_staff_copy(
        copy_by_barcode(&json!({ "items": staff_items }), &barcode),
        &public_note,
        &internal_note,
    );

    let item_id = fixtures::json_id(&created_copy["id"]);
    let (status, by_id) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{by_id}");
    assert_staff_copy(
        copy_by_barcode(&by_id, &barcode),
        &public_note,
        &internal_note,
    );

    let (status, by_barcode) = app
        .get_json_with_auth(&format!("/api/v1/items/barcode/{barcode}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{by_barcode}");
    assert_staff_copy(
        copy_by_barcode(&by_barcode, &barcode),
        &public_note,
        &internal_note,
    );

    for (label, token) in [("reader", Some(reader_token.as_str())), ("opac", None)] {
        let path = format!("/api/v1/biblios/{biblio_id}");
        let (status, body) = match token {
            Some(token) => app.get_json_with_auth(&path, token).await,
            None => {
                app.get_json(&format!("/api/v1/opac/biblios/{biblio_id}"))
                    .await
            }
        };
        assert_eq!(status, StatusCode::OK, "{label} biblio: {body}");
        assert_public_copy(
            copy_by_barcode(&body, &barcode),
            &public_note,
            &internal_note,
        );
        assert_no_staff_note(&body, &internal_note);
    }

    let (status, reader_items) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/items"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader_items}");
    let reader_list = json!({ "items": reader_items });
    assert_public_copy(
        copy_by_barcode(&reader_list, &barcode),
        &public_note,
        &internal_note,
    );
    assert_no_staff_note(&reader_list, &internal_note);

    let (status, reader_item) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader_item}");
    assert_public_copy(
        copy_by_barcode(&reader_item, &barcode),
        &public_note,
        &internal_note,
    );

    let (status, reader_barcode) = app
        .get_json_with_auth(&format!("/api/v1/items/barcode/{barcode}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader_barcode}");
    assert_public_copy(
        copy_by_barcode(&reader_barcode, &barcode),
        &public_note,
        &internal_note,
    );

    let title_q = title.replace(' ', "%20");
    let (status, search) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios?title={title_q}&perPage=5"),
            &reader_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{search}");
    assert_no_staff_note(&search, &internal_note);

    let (status, opac_search) = app
        .get_json(&format!("/api/v1/opac/biblios?title={title_q}&perPage=5"))
        .await;
    assert_eq!(status, StatusCode::OK, "{opac_search}");
    assert_no_staff_note(&opac_search, &internal_note);
}

#[tokio::test]
async fn put_keeps_omitted_internal_note_and_blank_clears_public_note() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "noteclear").await;
    let suffix = fixtures::unique_suffix();
    let barcode = format!("CLEAR-{suffix}");
    let internal_note = format!("provenance {suffix}");

    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Clear note {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "borrowable": true,
                    "publicNote": "  CD missing  ",
                    "internalNote": internal_note
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);
    assert_eq!(
        created["biblio"]["items"][0]["publicNote"], "CD missing",
        "create trims the public note"
    );

    let (status, updated) = app
        .put_json(
            &format!("/api/v1/items/{item_id}"),
            &json!({
                "barcode": barcode,
                "borrowable": true,
                "publicNote": "2-disc box set"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["publicNote"], "2-disc box set");
    assert_eq!(updated["internalNote"], internal_note, "{updated}");

    let (status, cleared) = app
        .put_json(
            &format!("/api/v1/items/{item_id}"),
            &json!({
                "barcode": barcode,
                "borrowable": true,
                "publicNote": "   ",
                "internalNote": internal_note
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["publicNote"].is_null(), "{cleared}");
    assert_eq!(cleared["internalNote"], internal_note);

    let (status, reader) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader}");
    let copy = copy_by_barcode(&reader, &barcode);
    assert!(copy["publicNote"].is_null(), "{copy}");
    assert_no_staff_note(&reader, &internal_note);
}

#[tokio::test]
async fn overlong_public_note_returns_validation_error_body() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Long note {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/biblios/{biblio_id}/items"),
            &json!({
                "barcode": format!("LONG-{suffix}"),
                "borrowable": true,
                "publicNote": "x".repeat(4001)
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "validation_error");
    assert!(body["error"].is_string(), "{body}");
    assert!(
        body["message"].as_str().unwrap_or("").contains("4000"),
        "{body}"
    );
    assert_eq!(body.as_object().expect("object").len(), 3, "{body}");
}

fn assert_legacy_notes_rejected(body: &Value) {
    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap_or_else(|| panic!("error object: {body}"))
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "error", "message"], "{body}");
    assert_eq!(body["code"], "validation_error", "{body}");
    assert_eq!(body["error"], "Validation Error", "{body}");
    let message = body["message"].as_str().unwrap_or_else(|| panic!("{body}"));
    assert!(message.contains("notes"), "{message}");
    assert!(message.contains("publicNote"), "{message}");
    assert!(message.contains("internalNote"), "{message}");
}

async fn count_rows(app: &TestApp, sql: &str, value: &str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(value)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("count")
}

#[tokio::test]
async fn put_with_legacy_notes_is_400_and_writes_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let barcode = format!("LEGPUT-{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Legacy notes put {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "callNumber": "SHELF-A",
                    "borrowable": true,
                    "publicNote": "visible",
                    "internalNote": "staff only"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    async fn snapshot(
        app: &TestApp,
        item_id: i64,
    ) -> (Option<String>, Option<String>, Option<String>) {
        sqlx::query_as("SELECT call_number, public_note, internal_note FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("read item")
    }

    let before = snapshot(&app, item_id).await;
    for notes in [json!("still the old field"), Value::Null] {
        let (status, body) = app
            .put_json(
                &format!("/api/v1/items/{item_id}"),
                &json!({
                    "barcode": barcode,
                    "callNumber": "CHANGED",
                    "borrowable": false,
                    "publicNote": "should not stick",
                    "internalNote": "should not stick",
                    "notes": notes
                }),
                Some(&admin_token),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_legacy_notes_rejected(&body);
        assert_eq!(snapshot(&app, item_id).await, before, "{notes}");
    }
}

#[tokio::test]
async fn create_with_legacy_notes_is_400_and_writes_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Legacy notes parent {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_barcode = format!("LEGITEM-{suffix}");

    let (status, body) = app
        .post_json(
            &format!("/api/v1/biblios/{biblio_id}/items"),
            &json!({
                "barcode": item_barcode,
                "borrowable": true,
                "publicNote": "visible",
                "notes": "legacy"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_legacy_notes_rejected(&body);
    assert_eq!(
        count_rows(
            &app,
            "SELECT COUNT(*) FROM items WHERE barcode = $1",
            &item_barcode
        )
        .await,
        0
    );

    let nested_title = format!("Legacy notes nested {suffix}");
    let nested_barcode = format!("LEGNST-{suffix}");
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": nested_title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": nested_barcode,
                    "borrowable": true,
                    "internalNote": "should not be stored",
                    "notes": null
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_legacy_notes_rejected(&body);
    assert_eq!(
        count_rows(
            &app,
            "SELECT COUNT(*) FROM biblios WHERE title = $1",
            &nested_title
        )
        .await,
        0
    );
    assert_eq!(
        count_rows(
            &app,
            "SELECT COUNT(*) FROM items WHERE barcode = $1",
            &nested_barcode
        )
        .await,
        0
    );
}

#[tokio::test]
async fn note_limit_counts_unicode_characters() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Unicode note {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let accepted = "é".repeat(4000);
    assert_eq!(accepted.chars().count(), 4000);
    assert!(accepted.len() > 4000);

    let (status, body) = app
        .post_json(
            &format!("/api/v1/biblios/{biblio_id}/items"),
            &json!({
                "barcode": format!("UNI-{suffix}"),
                "borrowable": true,
                "publicNote": accepted,
                "internalNote": accepted
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["publicNote"], accepted);
    assert_eq!(body["internalNote"], accepted);

    let rejected = "é".repeat(4001);
    assert_eq!(rejected.chars().count(), 4001);
    for field in ["publicNote", "internalNote"] {
        let barcode = format!("UNIBAD-{field}-{suffix}");
        let (status, body) = app
            .post_json(
                &format!("/api/v1/biblios/{biblio_id}/items"),
                &json!({
                    "barcode": barcode,
                    "borrowable": true,
                    field: rejected
                }),
                Some(&admin_token),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field} {body}");
        assert_eq!(body["code"], "validation_error", "{body}");
        assert_eq!(body["error"], "Validation Error", "{body}");
        assert!(
            body["message"].as_str().unwrap_or("").contains("4000"),
            "{body}"
        );
        assert_eq!(body.as_object().expect("object").len(), 3, "{body}");
        assert_eq!(
            count_rows(
                &app,
                "SELECT COUNT(*) FROM items WHERE barcode = $1",
                &barcode
            )
            .await,
            0,
            "{field}"
        );
    }

    let nested_title = format!("Unicode nested {suffix}");
    let nested_note = "é".repeat(4000);
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": nested_title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("UNINEST-{suffix}"),
                    "borrowable": true,
                    "publicNote": nested_note
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["biblio"]["items"][0]["publicNote"], nested_note);

    let rejected_title = format!("Unicode nested reject {suffix}");
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": rejected_title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("UNIBADNEST-{suffix}"),
                    "borrowable": true,
                    "internalNote": "é".repeat(4001)
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["code"], "validation_error", "{body}");
    assert!(
        body["message"].as_str().unwrap_or("").contains("4000"),
        "{body}"
    );
    assert_eq!(
        count_rows(
            &app,
            "SELECT COUNT(*) FROM biblios WHERE title = $1",
            &rejected_title
        )
        .await,
        0
    );
}
