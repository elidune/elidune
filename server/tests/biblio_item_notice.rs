//! `GET /biblios/:id` copy availability: due date, hold flag, and borrower shape.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::{json, Value};

fn retarget(token: &str, secret: &str, users: Rights, loans: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.users_rights = users;
    claims.rights.loans_rights = loans;
    claims.create_token(secret).expect("mint test token")
}

fn copy_by_id(body: &Value, item_id: i64) -> &Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == item_id)
        .unwrap_or_else(|| panic!("copy {item_id} missing from {body}"))
}

fn assert_no_borrower(copy: &Value) {
    assert!(copy.get("borrower").is_none(), "borrower leaked: {copy}");
    assert!(copy.get("loanId").is_none(), "loanId leaked: {copy}");
    assert!(copy.get("holdUserId").is_none(), "{copy}");
    assert!(copy.get("holdUserName").is_none(), "{copy}");
    assert!(copy.get("cardNumber").is_none(), "{copy}");
    assert!(copy.get("initials").is_none(), "{copy}");
    assert!(copy.get("firstname").is_none(), "{copy}");
    assert!(copy.get("lastname").is_none(), "{copy}");
    assert!(copy.get("login").is_none(), "{copy}");
    assert!(copy.get("email").is_none(), "{copy}");
}

fn assert_text_hidden(body: &Value, needles: &[&str]) {
    let text = body.to_string();
    for needle in needles {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

fn assert_same_instant(left: &Value, right: &Value) {
    let micros = |value: &Value| {
        chrono::DateTime::parse_from_rfc3339(value.as_str().expect("timestamp"))
            .unwrap_or_else(|err| panic!("rfc3339 {value}: {err}"))
            .timestamp_micros()
    };
    assert_eq!(micros(left), micros(right), "{left} vs {right}");
}

fn assert_minimized(borrower: &Value, user_id: i64, card: &str) {
    assert_eq!(borrower["id"], user_id.to_string(), "{borrower}");
    assert_eq!(borrower["cardNumber"], card, "{borrower}");
    assert_eq!(borrower["initials"], "IM", "{borrower}");
    let mut keys: Vec<&str> = borrower
        .as_object()
        .expect("borrower object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["cardNumber", "id", "initials"], "{borrower}");
}

async fn create_biblio(app: &TestApp, admin_token: &str, copies: Vec<Value>) -> (i64, Vec<i64>) {
    let suffix = fixtures::unique_suffix();
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Notice availability {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": copies
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let ids = body["biblio"]["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| fixtures::json_id(&item["id"]))
        .collect();
    (biblio_id, ids)
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> Value {
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": user_id.to_string(),
                "itemId": item_id.to_string(),
                "force": false
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

#[tokio::test]
async fn biblio_notice_due_date_hold_flag_and_borrower_follow_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (borrower_id, borrower_token) =
        fixtures::create_reader(&app, &admin_token, "notice_bor").await;
    let (holder_id, holder_token) =
        fixtures::create_reader(&app, &admin_token, "notice_hold").await;

    let card = format!("CARD-{}", fixtures::unique_suffix());
    let email = format!("isolde.notice.{borrower_id}@secret.test");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Isolde',
            lastname = 'Moreau',
            email = $2,
            phone = '0612345678',
            addr_street = '12 Rue de la Paix',
            barcode = $3
        WHERE id = $1
        "#,
    )
    .bind(borrower_id)
    .bind(&email)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set borrower identity");

    let holder_email = format!("helene.holder.{holder_id}@secret.test");
    sqlx::query(
        "UPDATE users SET firstname = 'Helene', lastname = 'Holder', email = $2 WHERE id = $1",
    )
    .bind(holder_id)
    .bind(&holder_email)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set holder identity");

    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(borrower_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");

    let secret_bits = [
        "Isolde",
        "Moreau",
        email.as_str(),
        "0612345678",
        "12 Rue de la Paix",
        card.as_str(),
        "Helene",
        "Holder",
        holder_email.as_str(),
        login.as_str(),
    ];

    let suffix = fixtures::unique_suffix();
    let loaned_barcode = format!("NTC-A-{suffix}");
    let free_barcode = format!("NTC-B-{suffix}");
    let (biblio_id, copies) = create_biblio(
        &app,
        &admin_token,
        vec![
            json!({ "barcode": loaned_barcode, "borrowable": true }),
            json!({ "barcode": free_barcode, "borrowable": true }),
        ],
    )
    .await;
    let loaned_id = copies[0];
    let free_id = copies[1];
    let loan = checkout(&app, &admin_token, borrower_id, loaned_id).await;
    let path = format!("/api/v1/biblios/{biblio_id}");

    let loans_read = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let loans_write = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let users_own = retarget(&admin_token, &secret, Rights::Own, Rights::Write);
    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::Write);
    let users_read_only = retarget(&admin_token, &secret, Rights::Read, Rights::None);

    let (patron_status, patron) = app.get_json_with_auth(&path, &borrower_token).await;
    let (opac_status, opac) = app
        .get_json(&format!("/api/v1/opac/biblios/{biblio_id}"))
        .await;
    for (label, status, body) in [
        ("patron", patron_status, &patron),
        ("opac", opac_status, &opac),
    ] {
        assert_eq!(status, StatusCode::OK, "{label} {body}");
        assert_text_hidden(body, &secret_bits);
        let loaned = copy_by_id(body, loaned_id);
        let free = copy_by_id(body, free_id);
        assert_no_borrower(loaned);
        assert_no_borrower(free);
        assert_same_instant(&loaned["dueDate"], &loan["expiryAt"]);
        assert_eq!(loaned["onHold"], false, "{loaned}");
        assert_eq!(loaned["availability"], "onLoan", "{loaned}");
        assert!(free["dueDate"].is_null(), "{free}");
        assert_eq!(free["onHold"], false, "{free}");
    }

    for token in [&loans_read, &loans_write, &users_own] {
        let (status, body) = app.get_json_with_auth(&path, token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_text_hidden(
            &body,
            &[
                "Isolde",
                "Moreau",
                email.as_str(),
                "0612345678",
                login.as_str(),
            ],
        );
        let loaned = copy_by_id(&body, loaned_id);
        assert_eq!(loaned["loanId"], loan["id"], "{loaned}");
        assert_same_instant(&loaned["dueDate"], &loan["expiryAt"]);
        assert_eq!(loaned["onHold"], false, "{loaned}");
        assert_minimized(&loaned["borrower"], borrower_id, &card);
        let free = copy_by_id(&body, free_id);
        assert!(free.get("borrower").is_none(), "{free}");
        assert!(free.get("loanId").is_none(), "{free}");
        assert!(free["dueDate"].is_null(), "{free}");
    }

    let (status, full) = app.get_json_with_auth(&path, &users_read).await;
    assert_eq!(status, StatusCode::OK, "{full}");
    let borrower = &copy_by_id(&full, loaned_id)["borrower"];
    assert_eq!(borrower["id"], borrower_id.to_string(), "{borrower}");
    assert_eq!(borrower["firstname"], "Isolde", "{borrower}");
    assert_eq!(borrower["lastname"], "Moreau", "{borrower}");
    assert_eq!(borrower["login"], login, "{borrower}");
    assert_eq!(borrower["barcode"], card, "{borrower}");
    assert!(borrower.get("cardNumber").is_none(), "{borrower}");
    assert!(borrower.get("initials").is_none(), "{borrower}");
    assert!(borrower.get("email").is_none(), "{borrower}");
    assert!(borrower.get("phone").is_none(), "{borrower}");
    assert_same_instant(&copy_by_id(&full, loaned_id)["dueDate"], &loan["expiryAt"]);
    assert_eq!(copy_by_id(&full, loaned_id)["onHold"], false);

    let (status, cataloger) = app.get_json_with_auth(&path, &users_read_only).await;
    assert_eq!(status, StatusCode::OK, "{cataloger}");
    assert!(
        copy_by_id(&cataloger, loaned_id).get("borrower").is_none(),
        "users read without loans rights must not see the borrower: {cataloger}"
    );
    assert_text_hidden(
        &cataloger,
        &[
            "Isolde",
            "Moreau",
            card.as_str(),
            email.as_str(),
            login.as_str(),
        ],
    );
    assert_eq!(
        copy_by_id(&cataloger, loaned_id)["loanId"],
        loan["id"],
        "staff without loans rights still keep loanId"
    );

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
    assert_eq!(fixtures::json_id(&hold["itemId"]), free_id, "{hold}");

    let (status, after) = app.get_json_with_auth(&path, &borrower_token).await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_text_hidden(&after, &secret_bits);
    assert_eq!(copy_by_id(&after, loaned_id)["onHold"], false, "{after}");
    assert_eq!(copy_by_id(&after, free_id)["onHold"], true, "{after}");
    assert_no_borrower(copy_by_id(&after, free_id));

    let only_barcode = format!("NTC-ONE-{suffix}");
    let (only_biblio, only_copies) = create_biblio(
        &app,
        &admin_token,
        vec![json!({ "barcode": only_barcode, "borrowable": true })],
    )
    .await;
    let only_id = only_copies[0];
    let only_loan = checkout(&app, &admin_token, borrower_id, only_id).await;
    let (status, queued) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": holder_id.to_string(),
                "biblioId": only_biblio.to_string()
            }),
            Some(&holder_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{queued}");
    assert!(
        queued["itemId"].is_null(),
        "title hold stays unassigned while the only copy is on loan: {queued}"
    );

    let only_path = format!("/api/v1/biblios/{only_biblio}");
    let (status, queued_notice) = app
        .get_json(&format!("/api/v1/opac/biblios/{only_biblio}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{queued_notice}");
    let queued_copy = copy_by_id(&queued_notice, only_id);
    assert_eq!(queued_copy["availability"], "onLoan", "{queued_copy}");
    assert_eq!(queued_copy["onHold"], true, "{queued_copy}");
    assert_same_instant(&queued_copy["dueDate"], &only_loan["expiryAt"]);
    assert_no_borrower(queued_copy);
    assert_text_hidden(&queued_notice, &secret_bits);

    let (status, staff_queued) = app.get_json_with_auth(&only_path, &loans_write).await;
    assert_eq!(status, StatusCode::OK, "{staff_queued}");
    assert_eq!(copy_by_id(&staff_queued, only_id)["onHold"], true);
    assert_minimized(
        &copy_by_id(&staff_queued, only_id)["borrower"],
        borrower_id,
        card.as_str(),
    );
}
