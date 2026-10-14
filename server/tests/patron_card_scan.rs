//! Desk card scan: loans staff get a minimal identity plus loan status.

mod common;

use std::sync::atomic::{AtomicI64, Ordering};

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

fn next_id() -> i64 {
    static N: AtomicI64 = AtomicI64::new(1);
    let tick = fixtures::unique_suffix() as i64;
    tick.wrapping_add(N.fetch_add(1, Ordering::Relaxed))
}

const LOAN_STATUS_KEYS: &[&str] = &[
    "activeLoans",
    "blockReasons",
    "blocked",
    "maxLoans",
    "overdueLoans",
    "readyHolds",
    "subscriptionExpiresAt",
    "subscriptionValid",
    "unpaidFinesCents",
];

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

fn assert_loan_status_shape(body: &Value) {
    assert_eq!(
        sorted_keys(&body["loanStatus"]),
        LOAN_STATUS_KEYS,
        "loanStatus keys: {body}"
    );
}

fn reasons(body: &Value) -> Vec<&str> {
    body["loanStatus"]["blockReasons"]
        .as_array()
        .expect("blockReasons")
        .iter()
        .map(|value| value.as_str().expect("reason code"))
        .collect()
}

async fn scan(app: &TestApp, token: &str, card: &str) -> (StatusCode, Value) {
    let path = format!("/api/v1/users/by-card/{card}");
    app.get_json_with_auth(&path, token).await
}

async fn create_patron(
    app: &TestApp,
    admin_token: &str,
    prefix: &str,
    public_type_id: Option<i64>,
) -> (i64, String) {
    let suffix = fixtures::unique_suffix();
    let login = format!("{prefix}_{suffix}");
    let card = format!("CARD-{suffix}");
    let public_type_id = match public_type_id {
        Some(id) => id,
        None => fixtures::public_type_id_by_name(app, admin_token, "adult").await,
    };
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Isolde",
                "lastname": "Moreau",
                "email": format!("{login}@secret.test"),
                "phone": "0612345678",
                "addrStreet": "12 Rue de la Paix",
                "addrCity": "Paris",
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "barcode": card,
                "sex": "f",
                "birthdate": "1990-06-01"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "create patron: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear password flag");
    (user_id, card)
}

async fn create_audience(app: &TestApp, admin_token: &str, max_loans: i16, threshold: &str) -> i64 {
    let name = format!("cardscan_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "amount",
                "unpaidThresholdCents": elidune_server::models::fine::money_to_cents(
                    threshold.parse().expect("threshold")
                )
                .expect("cents")
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {body}");
    let id = fixtures::json_id(&body["id"]);
    let (status, settings) = app
        .put_json(
            &format!("/api/v1/public-types/{id}/loan-settings"),
            &json!({
                "settings": [{ "nbMax": max_loans, "duration": 21, "nbRenews": 1 }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "loan settings: {settings}");
    id
}

async fn borrowable_item(app: &TestApp, admin_token: &str) -> (i64, i64) {
    let suffix = fixtures::unique_suffix();
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Card scan {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": format!("COPY-{suffix}"), "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "biblio: {biblio}");
    let biblio_id = fixtures::json_id(&biblio["biblio"]["id"]);
    let item_id = fixtures::json_id(&biblio["biblio"]["items"][0]["id"]);
    (biblio_id, item_id)
}

async fn checkout(
    app: &TestApp,
    token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(token),
    )
    .await
}

fn assert_no_personal_text(body: &Value, forbidden: &[&str]) {
    let text = body.to_string();
    for needle in forbidden {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

#[tokio::test]
async fn loans_only_card_scan_is_minimal_and_users_read_is_full() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let (reader_id, card) = create_patron(&app, &admin_token, "scanmin", None).await;

    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("login");

    let (biblio_a, item_a) = borrowable_item(&app, &admin_token).await;
    let (biblio_b, item_b) = borrowable_item(&app, &admin_token).await;
    let (biblio_c, item_c) = borrowable_item(&app, &admin_token).await;
    let site_a: i64 = sqlx::query_scalar(
        r#"INSERT INTO sources (name, is_archive, "default") VALUES ($1, 0, FALSE) RETURNING id"#,
    )
    .bind(format!("site-a-{}", fixtures::unique_suffix()))
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("site a");
    let site_b: i64 = sqlx::query_scalar(
        r#"INSERT INTO sources (name, is_archive, "default") VALUES ($1, 0, FALSE) RETURNING id"#,
    )
    .bind(format!("site-b-{}", fixtures::unique_suffix()))
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("site b");

    for (biblio_id, item_id, site_id, status_name) in [
        (biblio_a, item_a, Some(site_a), "ready"),
        (biblio_b, item_b, Some(site_b), "ready"),
        (biblio_c, item_c, None, "pending"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO holds (id, user_id, item_id, biblio_id, position, status, pickup_site_id)
            VALUES ($1, $2, $3, $4, 1, $5, $6)
            "#,
        )
        .bind(next_id())
        .bind(reader_id)
        .bind(item_id)
        .bind(biblio_id)
        .bind(status_name)
        .bind(site_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("hold");
    }

    let personal = [
        "Isolde",
        "Moreau",
        "0612345678",
        "12 Rue de la Paix",
        login.as_str(),
        "@secret.test",
    ];

    let loans_write = retarget(&admin_token, &secret, Rights::None, Rights::Write);
    let (status, body) = scan(&app, &loans_write, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        sorted_keys(&body),
        ["cardNumber", "id", "initials", "loanStatus"],
        "loans-only body: {body}"
    );
    assert_eq!(body["id"], reader_id.to_string(), "{body}");
    assert_eq!(body["cardNumber"], card, "{body}");
    assert_eq!(body["initials"], "IM", "{body}");
    assert_loan_status_shape(&body);
    assert_eq!(body["loanStatus"]["blocked"], false, "{body}");
    assert_eq!(body["loanStatus"]["blockReasons"], json!([]), "{body}");
    assert_eq!(body["loanStatus"]["subscriptionValid"], true, "{body}");
    assert!(
        body["loanStatus"]["subscriptionExpiresAt"].is_null(),
        "{body}"
    );
    assert_eq!(body["loanStatus"]["activeLoans"], 0, "{body}");
    assert_eq!(body["loanStatus"]["overdueLoans"], 0, "{body}");
    assert_eq!(body["loanStatus"]["unpaidFinesCents"], 0, "{body}");
    assert_eq!(body["loanStatus"]["readyHolds"], 2, "{body}");
    assert!(body["loanStatus"]["maxLoans"].is_number(), "{body}");
    assert_no_personal_text(&body, &personal);

    let loans_read = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let (status, body) = scan(&app, &loans_read, &card).await;
    assert_eq!(status, StatusCode::OK, "loans read: {body}");
    assert_eq!(
        sorted_keys(&body),
        ["cardNumber", "id", "initials", "loanStatus"]
    );
    assert_no_personal_text(&body, &personal);

    let users_own = retarget(&admin_token, &secret, Rights::Own, Rights::None);
    let (status, _) = scan(&app, &users_own, &card).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, listed) = app
        .get_json_with_auth(
            &format!("/api/v1/users?name=Isolde&barcode={card}"),
            &loans_write,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "name or list search: {listed}"
    );
    let (status, listed) = app
        .get_json_with_auth(&format!("/api/v1/users?name={}", "Moreau"), &loans_write)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "name search: {listed}");

    let prefix = &card[..card.len().saturating_sub(2)];
    let (status, missing) = scan(&app, &loans_write, prefix).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "prefix is not a scan: {missing}"
    );

    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::None);
    let (status, full) = scan(&app, &users_read, &card).await;
    assert_eq!(status, StatusCode::OK, "users read: {full}");
    assert_eq!(full["firstname"], "Isolde", "{full}");
    assert_eq!(full["lastname"], "Moreau", "{full}");
    assert_eq!(full["login"], login, "{full}");
    assert_eq!(
        full["email"].as_str().unwrap_or(""),
        format!("{login}@secret.test")
    );
    assert_eq!(full["phone"], "0612345678", "{full}");
    assert_eq!(full["barcode"], card, "{full}");
    assert!(full.get("cardNumber").is_none(), "{full}");
    assert!(full.get("initials").is_none(), "{full}");
    assert_loan_status_shape(&full);
    assert_eq!(full["loanStatus"]["readyHolds"], 2, "{full}");
}

#[tokio::test]
async fn card_scan_reports_each_checkout_block_reason() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let audience = create_audience(&app, &admin_token, 1, "0").await;
    let (reader_id, card) = create_patron(&app, &admin_token, "scanblock", Some(audience)).await;
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);

    sqlx::query("UPDATE users SET status = 'blocked' WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("block account");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");
    assert_eq!(reasons(&body), ["account_blocked"], "{body}");
    assert_no_personal_text(&body, &["Isolde", "Moreau"]);

    sqlx::query(
        "UPDATE users SET status = 'active', expiry_at = '2020-01-15T12:00:00Z' WHERE id = $1",
    )
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("expire subscription");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["subscriptionValid"], false, "{body}");
    assert_eq!(
        body["loanStatus"]["subscriptionExpiresAt"], "2020-01-15",
        "{body}"
    );
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");
    assert_eq!(reasons(&body), ["subscription_expired"], "{body}");

    sqlx::query("UPDATE users SET expiry_at = NULL WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("clear expiry");
    let (_, item_id) = borrowable_item(&app, &admin_token).await;
    let (status, created) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "at cap: {created}");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["activeLoans"], 1, "{body}");
    assert_eq!(body["loanStatus"]["maxLoans"], 1, "{body}");
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");
    assert_eq!(reasons(&body), ["max_loans"], "{body}");

    let loan_id = fixtures::json_id(&created["id"]);
    sqlx::query("UPDATE loans SET expiry_at = NOW() - interval '3 days' WHERE id = $1")
        .bind(loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("make overdue");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["overdueLoans"], 1, "{body}");
    assert_eq!(reasons(&body), ["max_loans", "overdue_loans"], "{body}");
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");

    let wide = create_audience(&app, &admin_token, 5, "100.00").await;
    let (late_id, late_card) = create_patron(&app, &admin_token, "scanlate", Some(wide)).await;
    let (_, late_item) = borrowable_item(&app, &admin_token).await;
    let (status, late_loan) = checkout(&app, &admin_token, late_id, late_item, false).await;
    assert_eq!(status, StatusCode::CREATED, "{late_loan}");
    sqlx::query("UPDATE loans SET expiry_at = NOW() - interval '3 days' WHERE id = $1")
        .bind(fixtures::json_id(&late_loan["id"]))
        .execute(app.state.services.repository.pool())
        .await
        .expect("overdue only");
    let (status, body) = scan(&app, &loans_only, &late_card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["overdueLoans"], 1, "{body}");
    assert_eq!(reasons(&body), ["overdue_loans"], "{body}");
    assert_eq!(
        body["loanStatus"]["blocked"], false,
        "overdue loans do not block checkout: {body}"
    );
}

#[tokio::test]
async fn card_scan_lost_card_blocks_checkout() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let audience = create_audience(&app, &admin_token, 5, "100.00").await;
    let (reader_id, card) = create_patron(&app, &admin_token, "scanlost", Some(audience)).await;
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);

    sqlx::query("UPDATE users SET card_lost = TRUE WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark card lost");

    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        sorted_keys(&body),
        ["cardNumber", "id", "initials", "loanStatus"]
    );
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");
    assert_eq!(reasons(&body), ["card_lost"], "{body}");
    assert_eq!(body["loanStatus"]["subscriptionValid"], true, "{body}");
    assert_no_personal_text(&body, &["Isolde", "Moreau"]);

    let (_, item_id) = borrowable_item(&app, &admin_token).await;
    let (status, refused) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_card_lost_body(status, &refused, Some(false));

    let (status, forced) = checkout(&app, &admin_token, reader_id, item_id, true).await;
    assert_card_lost_body(status, &forced, Some(false));

    let (status, batch) = app
        .post_json(
            "/api/v1/loans/batch-create",
            &json!({
                "userId": reader_id.to_string(),
                "barcodes": ["LOST-CARD-COPY"],
                "force": true
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    assert_eq!(batch["created"], 0, "{batch}");
    assert_eq!(batch["errors"], 1, "{batch}");
    assert_eq!(batch["results"][0]["success"], false, "{batch}");
    assert_eq!(batch["results"][0]["code"], "card_lost", "{batch}");
    assert!(
        batch["results"][0]["error"]
            .as_str()
            .unwrap_or("")
            .contains("lost or stolen"),
        "{batch}"
    );
}

fn assert_card_lost_body(status: StatusCode, body: &Value, forceable: Option<bool>) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let keys: &[&str] = if forceable.is_some() {
        &["code", "error", "forceable", "message"]
    } else {
        &["code", "error", "message"]
    };
    assert_eq!(sorted_keys(body), keys, "{body}");
    if let Some(flag) = forceable {
        assert_eq!(body["forceable"], flag, "{body}");
    }
    assert_eq!(body["code"], "card_lost", "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert_eq!(
        body["message"], "Library card is reported lost or stolen",
        "{body}"
    );
}

#[tokio::test]
async fn lost_card_renewal_refused_with_admin_force_return_and_payment_accepted() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let audience = create_audience(&app, &admin_token, 5, "100.00").await;
    let (reader_id, _) = create_patron(&app, &admin_token, "renewlost", Some(audience)).await;
    let (_, item_id) = borrowable_item(&app, &admin_token).await;
    let (status, loan) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    let barcode: Option<String> = sqlx::query_scalar("SELECT barcode FROM items WHERE id = $1")
        .bind(item_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("item barcode");
    let barcode = barcode.expect("barcode");

    sqlx::query("UPDATE users SET card_lost = TRUE WHERE id = $1")
        .bind(reader_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark card lost");

    let (status, forced) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_card_lost_body(status, &forced, Some(false));

    let (status, by_barcode) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_card_lost_body(status, &by_barcode, Some(false));

    let login: Option<String> = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("patron login");
    let (status, session) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "patron login: {session}");
    let patron_token = session["token"].as_str().expect("token");

    let (status, mine) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", patron_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    let row = mine["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == loan_id)
        .expect("loan on /me/loans");
    assert_eq!(row["canRenew"], false, "{row}");
    assert_eq!(row["renewalRefusalCode"], "card_lost", "{row}");

    let (status, patron_renew) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew"),
            Some(patron_token),
        )
        .await;
    assert_card_lost_body(status, &patron_renew, Some(false));

    let fine_id = next_id();
    sqlx::query(
        "INSERT INTO fines (id, loan_id, user_id, amount, paid_amount, status, charge_type)
         VALUES ($1, $2, $3, 500, 0, 'pending', 'overdue')",
    )
    .bind(fine_id)
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("insert fine");
    let (status, paid) = app
        .post_json(
            &format!("/api/v1/fines/{fine_id}/pay"),
            &json!({ "amount": "5.00" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "payment: {paid}");
    assert_eq!(paid["status"], "paid", "{paid}");

    let (status, returned) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {returned}");
}

#[tokio::test]
async fn card_scan_unpaid_fines_follow_the_configured_threshold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let audience = create_audience(&app, &admin_token, 8, "10.00").await;
    let (reader_id, card) = create_patron(&app, &admin_token, "scanfines", Some(audience)).await;
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Write);

    let (_, item_id) = borrowable_item(&app, &admin_token).await;
    let (status, created) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let loan_id = fixtures::json_id(&created["id"]);
    let fine_id = next_id();
    sqlx::query(
        r#"
        INSERT INTO fines (id, loan_id, user_id, amount, paid_amount, status, charge_type)
        VALUES ($1, $2, $3, 900, 0, 'pending', 'overdue')
        "#,
    )
    .bind(fine_id)
    .bind(loan_id)
    .bind(reader_id)
    .execute(app.state.services.repository.pool())
    .await
    .expect("fine below threshold");

    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["unpaidFinesCents"], 900, "{body}");
    assert_eq!(body["loanStatus"]["blocked"], false, "{body}");
    assert_eq!(reasons(&body), Vec::<&str>::new(), "{body}");

    sqlx::query("UPDATE fines SET amount = 1000 WHERE id = $1")
        .bind(fine_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("fine at threshold");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["unpaidFinesCents"], 1000, "{body}");
    assert_eq!(
        body["loanStatus"]["blocked"], false,
        "equal to the threshold does not block: {body}"
    );
    assert!(!reasons(&body).contains(&"unpaid_fines"), "{body}");

    sqlx::query("UPDATE fines SET amount = 1001 WHERE id = $1")
        .bind(fine_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("fine above threshold");
    let (status, body) = scan(&app, &loans_only, &card).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["loanStatus"]["unpaidFinesCents"], 1001, "{body}");
    assert_eq!(body["loanStatus"]["blocked"], true, "{body}");
    assert_eq!(reasons(&body), ["unpaid_fines"], "{body}");
    assert_no_personal_text(&body, &["Isolde", "Moreau"]);
}
