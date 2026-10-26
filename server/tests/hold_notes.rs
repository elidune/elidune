//! Hold notes are split into `patronNote` and `internalNote` (#137).
//!
//! Former `holds.notes` values live in `internal_note` because staff could write
//! that column. Staff queues return both fields. `GET /me/holds` returns
//! `patronNote` only. `GET /holds` (search and public list) returns neither.
//! `internalNote` requires `holds_rights` write, not an account type name.
//! There is no `GET /holds/{id}`.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{AccountTypeSlug, Rights, UserClaims, UserRights};
use serde_json::{json, Value};

fn assert_error_shape(body: &Value) {
    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap_or_else(|| panic!("error object: {body}"))
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["code", "error", "message"], "{body}");
}

fn assert_patron_hold(body: &Value, patron_note: &str, secret: &str) {
    assert_eq!(body["patronNote"], patron_note, "{body}");
    assert!(body.get("internalNote").is_none(), "{body}");
    assert!(body.get("notes").is_none(), "{body}");
    assert!(!body.to_string().contains(secret), "{body}");
}

fn assert_staff_hold(body: &Value, patron_note: &str, internal_note: &str) {
    assert_eq!(body["patronNote"], patron_note, "{body}");
    assert_eq!(body["internalNote"], internal_note, "{body}");
    assert!(body.get("notes").is_none(), "{body}");
}

fn assert_no_hold_notes(body: &Value, secret: &str) {
    assert!(body.get("patronNote").is_none(), "{body}");
    assert!(body.get("internalNote").is_none(), "{body}");
    assert!(body.get("notes").is_none(), "{body}");
    assert!(!body.to_string().contains(secret), "{body}");
}

fn assert_closed_notes_cleared(body: &Value, secret: &str) {
    assert!(body["patronNote"].is_null(), "{body}");
    if let Some(internal) = body.get("internalNote") {
        assert!(internal.is_null(), "{body}");
    }
    assert!(body.get("notes").is_none(), "{body}");
    assert!(!body.to_string().contains(secret), "{body}");
}

async fn stored_notes(app: &TestApp, hold_id: i64) -> (Option<String>, Option<String>) {
    sqlx::query_as("SELECT patron_note, internal_note FROM holds WHERE id = $1")
        .bind(hold_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("stored hold notes")
}

async fn staff_list_row(app: &TestApp, token: &str, hold_id: i64) -> Value {
    let (status, head) = app
        .get_json_with_auth("/api/v1/holds?perPage=200&page=1", token)
        .await;
    assert_eq!(status, StatusCode::OK, "{head}");
    let pages = head["page_count"].as_i64().unwrap_or(1).max(1);
    for page in (1..=pages).rev() {
        let body = if page == 1 {
            head.clone()
        } else {
            let (status, body) = app
                .get_json_with_auth(&format!("/api/v1/holds?perPage=200&page={page}"), token)
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            body
        };
        if let Some(row) = body["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|row| fixtures::json_id(&row["id"]) == hold_id)
        {
            return row.clone();
        }
    }
    panic!("hold {hold_id} missing from staff list");
}

fn find_hold(items: &[Value], id: i64) -> &Value {
    items
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == id)
        .unwrap_or_else(|| panic!("hold {id} missing from {items:?}"))
}

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, i64) {
    let barcode = format!("HOLDNOTE-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    (
        fixtures::json_id(&body["biblio"]["id"]),
        fixtures::json_id(&body["biblio"]["items"][0]["id"]),
    )
}

async fn hold_count(app: &TestApp, biblio_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds WHERE biblio_id = $1")
        .bind(biblio_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("count holds")
}

fn retarget_holds(token: &str, secret: &str, holds: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights = UserRights {
        items_rights: Rights::Read,
        holds_rights: holds,
        ..UserRights::default()
    };
    claims.create_token(secret).expect("mint test token")
}

#[tokio::test]
async fn migration_replaces_holds_notes_with_patron_and_internal() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'holds'
          AND column_name IN ('notes', 'patron_note', 'internal_note')
        ORDER BY column_name
        "#,
    )
    .fetch_all(app.state.services.repository.pool())
    .await
    .expect("hold note columns");
    assert_eq!(
        columns,
        vec!["internal_note".to_string(), "patron_note".to_string()]
    );
}

#[tokio::test]
async fn staff_sees_both_notes_and_patrons_see_patron_note_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "holdnote").await;
    let suffix = fixtures::unique_suffix();
    let patron_note = format!("please keep the CD {suffix}");
    let internal_note = format!("desk only {suffix}");
    let (biblio_id, item_id) =
        create_copy(&app, &admin_token, &format!("Hold notes {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "itemId": item_id.to_string(),
                "patronNote": format!("  {patron_note}  "),
                "internalNote": format!("  {internal_note}  ")
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let hold_id = fixtures::json_id(&created["id"]);
    assert_staff_hold(&created, &patron_note, &internal_note);

    let (status, mine) = app
        .get_json_with_auth("/api/v1/me/holds?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    let row = find_hold(mine["items"].as_array().expect("items"), hold_id);
    assert_patron_hold(row, &patron_note, &internal_note);
    assert!(!mine.to_string().contains(&internal_note), "{mine}");

    let (status, reader_list) = app
        .get_json_with_auth("/api/v1/holds?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader_list}");
    let row = find_hold(reader_list["items"].as_array().expect("items"), hold_id);
    assert_no_hold_notes(row, &internal_note);
    assert_no_hold_notes(row, &patron_note);

    let row = staff_list_row(&app, &admin_token, hold_id).await;
    assert_no_hold_notes(&row, &internal_note);
    assert_no_hold_notes(&row, &patron_note);

    for path in [
        format!("/api/v1/items/{item_id}/holds"),
        format!("/api/v1/biblios/{biblio_id}/holds"),
        format!("/api/v1/users/{reader_id}/holds"),
    ] {
        let (status, body) = app.get_json_with_auth(&path, &admin_token).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        let row = find_hold(body.as_array().expect("array"), hold_id);
        assert_staff_hold(row, &patron_note, &internal_note);
        let (status, denied) = app.get_json_with_auth(&path, &reader_token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {denied}");
        assert_error_shape(&denied);
        assert!(!denied.to_string().contains(&internal_note), "{denied}");
    }

    let (status, reader_created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{reader_created}");

    let (other_biblio, _) =
        create_copy(&app, &admin_token, &format!("Patron place {suffix}")).await;
    let own_note = format!("my note {suffix}");
    let (status, reader_created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": other_biblio.to_string(),
                "patronNote": own_note
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{reader_created}");
    let own_id = fixtures::json_id(&reader_created["id"]);
    assert_patron_hold(&reader_created, &own_note, &internal_note);

    let (status, cancelled) = app
        .delete_with_auth(&format!("/api/v1/holds/{own_id}"), &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_closed_notes_cleared(&cancelled, &own_note);
    assert_eq!(stored_notes(&app, own_id).await, (None, None));

    let (status, staff_cancel) = app
        .delete_with_auth(&format!("/api/v1/holds/{hold_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff_cancel}");
    assert_closed_notes_cleared(&staff_cancel, &patron_note);
    assert_closed_notes_cleared(&staff_cancel, &internal_note);
    assert_eq!(stored_notes(&app, hold_id).await, (None, None));
}

#[tokio::test]
async fn patron_internal_note_is_forbidden_and_legacy_notes_write_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "holddeny").await;
    let (biblio_id, _) = create_copy(&app, &admin_token, "Hold note denied").await;

    for body in [
        json!({
            "userId": reader_id.to_string(),
            "biblioId": biblio_id.to_string(),
            "patronNote": "visible",
            "internalNote": "secret"
        }),
        json!({
            "userId": reader_id.to_string(),
            "biblioId": biblio_id.to_string(),
            "internalNote": null
        }),
    ] {
        let (status, denied) = app
            .post_json("/api/v1/holds", &body, Some(&reader_token))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
        assert_error_shape(&denied);
        assert_eq!(denied["code"], "authorization_failed", "{denied}");
        assert_eq!(denied["error"], "Forbidden", "{denied}");
        assert_eq!(
            denied["message"], "Insufficient rights to set an internal hold note",
            "{denied}"
        );
        assert!(!denied.to_string().contains("secret"), "{denied}");
        assert_eq!(hold_count(&app, biblio_id).await, 0);
    }

    for notes in [json!("legacy"), json!(null)] {
        let (status, denied) = app
            .post_json(
                "/api/v1/holds",
                &json!({
                    "userId": reader_id.to_string(),
                    "biblioId": biblio_id.to_string(),
                    "patronNote": "should not stick",
                    "internalNote": "should not stick",
                    "notes": notes
                }),
                Some(&admin_token),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
        assert_error_shape(&denied);
        assert_eq!(denied["code"], "validation_error", "{denied}");
        assert_eq!(denied["error"], "Validation Error", "{denied}");
        let message = denied["message"].as_str().unwrap_or("");
        assert!(message.contains("notes"), "{message}");
        assert!(message.contains("patronNote"), "{message}");
        assert!(message.contains("internalNote"), "{message}");
        assert_eq!(hold_count(&app, biblio_id).await, 0);
    }
}

#[tokio::test]
async fn overlong_notes_are_rejected_and_four_thousand_scalars_are_kept() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "holdlong").await;
    let (biblio_id, _) = create_copy(&app, &admin_token, "Hold note length").await;
    let too_long = "é".repeat(4001);

    let (status, denied) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "patronNote": too_long
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    assert_error_shape(&denied);
    assert_eq!(denied["code"], "validation_error", "{denied}");
    assert_eq!(denied["error"], "Validation Error", "{denied}");
    assert!(
        denied["message"].as_str().unwrap_or("").contains("4000"),
        "{denied}"
    );
    assert_eq!(hold_count(&app, biblio_id).await, 0);

    let (status, denied) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "internalNote": too_long
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{denied}");
    assert_eq!(denied["code"], "validation_error", "{denied}");
    assert!(
        denied["message"]
            .as_str()
            .unwrap_or("")
            .contains("Internal note"),
        "{denied}"
    );
    assert_eq!(hold_count(&app, biblio_id).await, 0);

    let accepted = "é".repeat(4000);
    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "patronNote": accepted,
                "internalNote": accepted
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["patronNote"], accepted);
    assert_eq!(created["internalNote"], accepted);
    assert_eq!(hold_count(&app, biblio_id).await, 1);
}

#[tokio::test]
async fn note_visibility_follows_holds_rights_not_account_type() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "holdrights").await;
    let reader_claims = UserClaims::from_token(&reader_token, &secret).expect("reader");
    assert_eq!(reader_claims.account_type, AccountTypeSlug::Reader);
    let admin_claims = UserClaims::from_token(&admin_token, &secret).expect("admin");
    assert_eq!(admin_claims.account_type, AccountTypeSlug::Admin);

    let reader_write = retarget_holds(&reader_token, &secret, Rights::Write);
    let reader_read = retarget_holds(&reader_token, &secret, Rights::Read);
    let admin_own = retarget_holds(&admin_token, &secret, Rights::Own);
    let suffix = fixtures::unique_suffix();
    let patron_note = format!("shared {suffix}");
    let internal_note = format!("rights secret {suffix}");
    let (biblio_id, item_id) = create_copy(&app, &admin_token, &format!("Rights {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string(),
                "patronNote": patron_note,
                "internalNote": internal_note
            }),
            Some(&reader_write),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let hold_id = fixtures::json_id(&created["id"]);
    assert_staff_hold(&created, &patron_note, &internal_note);
    let minted = UserClaims::from_token(&reader_write, &secret).expect("reader write");
    assert_eq!(minted.account_type, AccountTypeSlug::Reader);
    assert_eq!(minted.rights.holds_rights, Rights::Write);

    let (status, queue) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}/holds"), &reader_read)
        .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_staff_hold(
        find_hold(queue.as_array().expect("queue"), hold_id),
        &patron_note,
        &internal_note,
    );
    let read_claims = UserClaims::from_token(&reader_read, &secret).expect("reader read");
    assert_eq!(read_claims.account_type, AccountTypeSlug::Reader);
    assert_eq!(read_claims.rights.holds_rights, Rights::Read);

    let (other_biblio, _) = create_copy(&app, &admin_token, &format!("Admin own {suffix}")).await;
    let (status, denied) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": admin_claims.user_id.to_string(),
                "biblioId": other_biblio.to_string(),
                "internalNote": "must not store"
            }),
            Some(&admin_own),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "authorization_failed", "{denied}");
    assert_eq!(hold_count(&app, other_biblio).await, 0);
    let own_claims = UserClaims::from_token(&admin_own, &secret).expect("admin own");
    assert_eq!(own_claims.account_type, AccountTypeSlug::Admin);
    assert_eq!(own_claims.rights.holds_rights, Rights::Own);

    let own_note = format!("admin as patron {suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": admin_claims.user_id.to_string(),
                "biblioId": other_biblio.to_string(),
                "patronNote": own_note
            }),
            Some(&admin_own),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_patron_hold(&created, &own_note, &internal_note);
    let own_id = fixtures::json_id(&created["id"]);

    let (status, listed) = app
        .get_json_with_auth("/api/v1/holds?perPage=50", &admin_own)
        .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_no_hold_notes(
        find_hold(listed["items"].as_array().expect("items"), own_id),
        &internal_note,
    );
    assert_no_hold_notes(
        find_hold(listed["items"].as_array().expect("items"), own_id),
        &own_note,
    );

    let (status, staff) = app
        .get_json_with_auth(
            &format!("/api/v1/biblios/{other_biblio}/holds"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{staff}");
    let row = find_hold(staff.as_array().expect("staff"), own_id);
    assert_eq!(row["patronNote"], own_note, "{row}");
    assert!(row["internalNote"].is_null(), "{row}");
    let _ = biblio_id;
}

#[tokio::test]
async fn patron_writes_patron_note_only_on_their_own_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (owner_id, owner_token) = fixtures::create_reader(&app, &admin_token, "holdown").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "holdother").await;
    let suffix = fixtures::unique_suffix();
    let own_note = format!("mine {suffix}");
    let (own_biblio, _) = create_copy(&app, &admin_token, &format!("Own hold {suffix}")).await;
    let (other_biblio, _) = create_copy(&app, &admin_token, &format!("Other hold {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": owner_id.to_string(),
                "biblioId": own_biblio.to_string(),
                "patronNote": own_note
            }),
            Some(&owner_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let hold_id = fixtures::json_id(&created["id"]);
    assert_patron_hold(&created, &own_note, "unused");
    assert_eq!(stored_notes(&app, hold_id).await, (Some(own_note), None));

    let (status, denied) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": other_id.to_string(),
                "biblioId": other_biblio.to_string(),
                "patronNote": format!("not theirs {suffix}")
            }),
            Some(&owner_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_error_shape(&denied);
    assert_eq!(denied["code"], "authorization_failed", "{denied}");
    assert_eq!(denied["error"], "Forbidden", "{denied}");
    assert_eq!(hold_count(&app, other_biblio).await, 0);
}

#[tokio::test]
async fn staff_with_holds_write_can_set_patron_note() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "holdstaff").await;
    let staff = retarget_holds(&reader_token, &secret, Rights::Write);
    let suffix = fixtures::unique_suffix();
    let patron_note = format!("desk wrote the patron note {suffix}");
    let (biblio_id, _) = create_copy(&app, &admin_token, &format!("Staff note {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "patronNote": patron_note
            }),
            Some(&staff),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let hold_id = fixtures::json_id(&created["id"]);
    assert_eq!(created["patronNote"], patron_note, "{created}");
    assert!(created["internalNote"].is_null(), "{created}");
    assert_eq!(
        stored_notes(&app, hold_id).await,
        (Some(patron_note.clone()), None)
    );
    let claims = UserClaims::from_token(&staff, &secret).expect("staff write");
    assert_eq!(claims.account_type, AccountTypeSlug::Reader);
    assert_eq!(claims.rights.holds_rights, Rights::Write);

    let (status, queue) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}/holds"), &staff)
        .await;
    assert_eq!(status, StatusCode::OK, "{queue}");
    assert_eq!(
        find_hold(queue.as_array().expect("queue"), hold_id)["patronNote"],
        patron_note
    );
}

#[tokio::test]
async fn anonymization_removes_both_hold_notes() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "holderase").await;
    let suffix = fixtures::unique_suffix();
    let patron_note = format!("erase patron {suffix}");
    let internal_note = format!("erase internal {suffix}");
    let (biblio_id, _) = create_copy(&app, &admin_token, &format!("Erase {suffix}")).await;

    let (status, created) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": reader_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "patronNote": patron_note,
                "internalNote": internal_note
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{reader_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let remaining: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM holds
        WHERE user_id = $1
           OR patron_note = $2 OR internal_note = $2
           OR patron_note = $3 OR internal_note = $3
        "#,
    )
    .bind(reader_id)
    .bind(&patron_note)
    .bind(&internal_note)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("count leftover notes");
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn closed_holds_clear_both_notes() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "holdclose").await;
    let suffix = fixtures::unique_suffix();
    let patron_note = format!("close patron {suffix}");
    let internal_note = format!("close internal {suffix}");

    async fn place(
        app: &TestApp,
        token: &str,
        reader_id: i64,
        title: &str,
        patron_note: &str,
        internal_note: &str,
    ) -> i64 {
        let (biblio_id, _) = create_copy(app, token, title).await;
        let (status, created) = app
            .post_json(
                "/api/v1/holds",
                &json!({
                    "userId": reader_id.to_string(),
                    "biblioId": biblio_id.to_string(),
                    "patronNote": patron_note,
                    "internalNote": internal_note
                }),
                Some(token),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        fixtures::json_id(&created["id"])
    }

    let cancel_id = place(
        &app,
        &admin_token,
        reader_id,
        &format!("Cancel {suffix}"),
        &patron_note,
        &internal_note,
    )
    .await;
    let fulfill_id = place(
        &app,
        &admin_token,
        reader_id,
        &format!("Fulfill {suffix}"),
        &patron_note,
        &internal_note,
    )
    .await;
    let expire_id = place(
        &app,
        &admin_token,
        reader_id,
        &format!("Expire {suffix}"),
        &patron_note,
        &internal_note,
    )
    .await;

    let (status, cancelled) = app
        .delete_with_auth(&format!("/api/v1/holds/{cancel_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_closed_notes_cleared(&cancelled, &patron_note);
    assert_eq!(stored_notes(&app, cancel_id).await, (None, None));
    let status_text: String = sqlx::query_scalar("SELECT status FROM holds WHERE id = $1")
        .bind(cancel_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("cancel status");
    assert_eq!(status_text, "cancelled");

    let fulfilled = app
        .state
        .services
        .repository
        .holds_fulfill(fulfill_id)
        .await
        .expect("fulfill");
    assert!(fulfilled.patron_note.is_none());
    assert!(fulfilled.internal_note.is_none());
    assert_eq!(stored_notes(&app, fulfill_id).await, (None, None));

    sqlx::query(
        "UPDATE holds SET status = 'ready', expires_at = NOW() - INTERVAL '1 day' WHERE id = $1",
    )
    .bind(expire_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("mark overdue");
    let expired = app
        .state
        .services
        .repository
        .holds_expire_overdue()
        .await
        .expect("expire");
    assert!(expired.contains(&expire_id), "{expired:?}");
    assert_eq!(stored_notes(&app, expire_id).await, (None, None));
}
