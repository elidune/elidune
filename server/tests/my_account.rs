//! Patron account lists (`GET /me/loans`, `GET /me/holds`).
//!
//! Owner rows are filtered by `user_id = caller` in SQL and use `MyLoanView` / `MyHoldView`.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

async fn create_site(app: &TestApp, admin_token: &str, name: &str) -> (i64, String) {
    let name = format!("{name}-{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json("/api/v1/sites", &json!({ "name": name }), Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create site: {body}");
    (fixtures::json_id(&body["id"]), name)
}

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, i64, String) {
    let barcode = format!("ME-{}", fixtures::unique_suffix());
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
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let biblio_id = fixtures::json_id(&body["biblio"]["id"]);
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    (biblio_id, item_id, barcode)
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> Value {
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": user_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    body
}

fn find_row(items: &[Value], id: i64) -> &Value {
    items
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == id)
        .unwrap_or_else(|| panic!("row {id} missing from {items:?}"))
}

#[tokio::test]
async fn reader_sees_own_loan_refusal_and_not_another_readers_loans() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (borrower_id, borrower_token) = fixtures::create_reader(&app, &admin_token, "meloan").await;
    let (waiter_id, waiter_token) = fixtures::create_reader(&app, &admin_token, "mewait").await;

    let (blocked_biblio, blocked_item, _) =
        create_copy(&app, &admin_token, "My Loan Blocked").await;
    let (_, open_item, _) = create_copy(&app, &admin_token, "My Loan Early").await;

    let blocked = checkout(&app, &admin_token, borrower_id, blocked_item).await;
    let early = checkout(&app, &admin_token, borrower_id, open_item).await;
    let blocked_loan_id = fixtures::json_id(&blocked["id"]);
    let early_loan_id = fixtures::json_id(&early["id"]);

    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": waiter_id.to_string(),
                "biblioId": blocked_biblio.to_string()
            }),
            Some(&waiter_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "title hold: {hold}");
    assert_eq!(hold["status"], "pending");
    assert!(
        hold["itemId"].is_null(),
        "title hold stays unassigned: {hold}"
    );

    let (status, list) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &borrower_token)
        .await;
    assert_eq!(status, StatusCode::OK, "my loans: {list}");
    let items = list["items"].as_array().expect("items");

    let blocked_row = find_row(items, blocked_loan_id);
    assert!(
        blocked_row["dueDate"].is_string(),
        "due date: {blocked_row}"
    );
    assert_eq!(blocked_row["canRenew"], false);
    assert_eq!(blocked_row["renewalRefusalCode"], "hold_waiting");
    assert!(blocked_row.get("biblio").is_none(), "{blocked_row}");
    assert!(
        blocked_row.get("expectedReturnDate").is_none(),
        "{blocked_row}"
    );
    assert!(blocked_row.get("login").is_none(), "{blocked_row}");
    assert!(blocked_row.get("weedingStatus").is_none(), "{blocked_row}");

    let early_row = find_row(items, early_loan_id);
    assert_eq!(early_row["renewalRefusalCode"], "renewal_too_early");
    let opens = early_row["renewalOpensAt"].as_str().expect("opening date");
    assert!(
        opens.len() == 10 && opens.as_bytes()[4] == b'-' && opens.as_bytes()[7] == b'-',
        "opening date: {early_row}"
    );

    let (status, denied) = app
        .get_json_with_auth(&format!("/api/v1/users/{borrower_id}/loans"), &waiter_token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "other loans: {denied}");

    let (status, waiter_loans) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &waiter_token)
        .await;
    assert_eq!(status, StatusCode::OK, "waiter loans: {waiter_loans}");
    let waiter_items = waiter_loans["items"].as_array().expect("items");
    assert!(
        waiter_items
            .iter()
            .all(|row| fixtures::json_id(&row["id"]) != blocked_loan_id),
        "waiter saw the borrower's loan: {waiter_loans}"
    );
}

#[tokio::test]
async fn reader_sees_ready_hold_deadline_and_not_another_readers_holds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (owner_id, owner_token) = fixtures::create_reader(&app, &admin_token, "mehold").await;
    let (_, other_token) = fixtures::create_reader(&app, &admin_token, "mehide").await;
    let (site_id, site_name) = create_site(&app, &admin_token, "MyPickup").await;
    let (biblio_id, _, barcode) = create_copy(&app, &admin_token, "My Hold Ready").await;

    let (status, placed) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": owner_id.to_string(),
                "biblioId": biblio_id.to_string(),
                "pickupSiteId": site_id.to_string()
            }),
            Some(&owner_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "place hold: {placed}");
    let hold_id = fixtures::json_id(&placed["id"]);

    let (status, scan) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "check in: {scan}");
    assert_eq!(scan["hold"]["status"], "ready", "check in: {scan}");

    let (status, list) = app
        .get_json_with_auth("/api/v1/me/holds?activeOnly=true&perPage=50", &owner_token)
        .await;
    assert_eq!(status, StatusCode::OK, "my holds: {list}");
    let row = find_row(list["items"].as_array().expect("items"), hold_id);
    assert_eq!(row["status"], "ready");
    assert!(row["pickupDeadline"].is_string(), "deadline: {row}");
    assert_eq!(fixtures::json_id(&row["pickupSiteId"]), site_id);
    assert_eq!(row["pickupSiteName"], site_name);
    assert!(row["position"].is_number(), "position: {row}");
    assert!(row.get("user").is_none(), "{row}");
    assert!(row.get("notes").is_none(), "{row}");
    assert!(row.get("login").is_none(), "{row}");

    let (status, denied_loans) = app
        .get_json_with_auth(&format!("/api/v1/users/{owner_id}/holds"), &other_token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "other holds: {denied_loans}");

    let (status, other_holds) = app
        .get_json_with_auth("/api/v1/me/holds?perPage=50", &other_token)
        .await;
    assert_eq!(status, StatusCode::OK, "other account: {other_holds}");
    let other_items = other_holds["items"].as_array().expect("items");
    assert!(
        other_items
            .iter()
            .all(|row| fixtures::json_id(&row["id"]) != hold_id),
        "other reader saw the hold: {other_holds}"
    );
}

async fn assert_same_refusal(app: &TestApp, token: &str, loan_id: i64, code: &str) {
    let (status, list) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", token)
        .await;
    assert_eq!(status, StatusCode::OK, "my loans: {list}");
    let row = find_row(list["items"].as_array().expect("items"), loan_id);
    assert_eq!(row["canRenew"], false, "{code}: {row}");
    assert_eq!(row["renewalRefusalCode"], code, "{row}");
    if code == "renewal_too_early" {
        assert!(row["renewalOpensAt"].is_string(), "{row}");
    } else {
        assert!(row["renewalOpensAt"].is_null(), "{row}");
    }

    let (status, body) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/renew"), Some(token))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}: {body}");
    assert_eq!(body["code"], code, "{body}");
}

async fn create_fine_public_type(app: &TestApp, admin_token: &str, threshold: &str) -> i64 {
    let name = format!("renewfine_{}", fixtures::unique_suffix());
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
    fixtures::json_id(&body["id"])
}

/// Each refusal code on `GET /me/loans` is the code `POST /loans/{id}/renew` returns.
#[tokio::test]
async fn renewal_list_and_renew_share_every_refusal_code() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (blocked_id, blocked_token) =
        fixtures::create_reader(&app, &admin_token, "renewblock").await;
    let (_, blocked_item, _) = create_copy(&app, &admin_token, "Account Blocked").await;
    let blocked_loan = checkout(&app, &admin_token, blocked_id, blocked_item).await;
    let blocked_loan_id = fixtures::json_id(&blocked_loan["id"]);
    sqlx::query("UPDATE users SET status = 'blocked' WHERE id = $1")
        .bind(blocked_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("block account");
    assert_same_refusal(&app, &blocked_token, blocked_loan_id, "account_blocked").await;

    let low_type = create_fine_public_type(&app, &admin_token, "0.50").await;
    let (fined_id, fined_token) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "renewfine", Some(low_type))
            .await;
    let (_, fined_item, _) = create_copy(&app, &admin_token, "Fines Over").await;
    let fined_loan = checkout(&app, &admin_token, fined_id, fined_item).await;
    let fined_loan_id = fixtures::json_id(&fined_loan["id"]);
    app.state
        .services
        .repository
        .fines_create(
            fined_loan_id,
            fined_id,
            elidune_server::models::fine::Cents(100),
            Some("over threshold"),
        )
        .await
        .expect("create fine");
    assert_same_refusal(&app, &fined_token, fined_loan_id, "fines_over_threshold").await;

    let high_type = create_fine_public_type(&app, &admin_token, "100.00").await;
    let (under_id, under_token) =
        fixtures::create_reader_with_public_type(&app, &admin_token, "renewunder", Some(high_type))
            .await;
    let (_, under_item, _) = create_copy(&app, &admin_token, "Fines Under Override").await;
    let under_loan = checkout(&app, &admin_token, under_id, under_item).await;
    let under_loan_id = fixtures::json_id(&under_loan["id"]);
    app.state
        .services
        .repository
        .fines_create(
            under_loan_id,
            under_id,
            elidune_server::models::fine::Cents(100),
            Some("under"),
        )
        .await
        .expect("create small fine");
    assert_same_refusal(&app, &under_token, under_loan_id, "renewal_too_early").await;

    let (withdrawn_id, withdrawn_token) =
        fixtures::create_reader(&app, &admin_token, "renewgone").await;
    let (_, withdrawn_item, _) = create_copy(&app, &admin_token, "Withdrawn Copy").await;
    let withdrawn_loan = checkout(&app, &admin_token, withdrawn_id, withdrawn_item).await;
    let withdrawn_loan_id = fixtures::json_id(&withdrawn_loan["id"]);
    sqlx::query("UPDATE items SET weeding_status = 'withdrawn' WHERE id = $1")
        .bind(withdrawn_item)
        .execute(app.state.services.repository.pool())
        .await
        .expect("withdraw copy");
    assert_same_refusal(&app, &withdrawn_token, withdrawn_loan_id, "withdrawn").await;

    let (claim_id, claim_token) = fixtures::create_reader(&app, &admin_token, "renewclaim").await;
    let (_, claim_item, _) = create_copy(&app, &admin_token, "Claimed Returned").await;
    let claim_loan = checkout(&app, &admin_token, claim_id, claim_item).await;
    let claim_loan_id = fixtures::json_id(&claim_loan["id"]);
    let (status, claim) = app
        .post_json(
            &format!("/api/v1/loans/{claim_loan_id}/claimed-returned"),
            &json!({ "notes": "patron says it is back", "bill": false }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "claim: {claim}");
    assert_same_refusal(&app, &claim_token, claim_loan_id, "claimed_returned").await;

    let (holder_id, holder_token) = fixtures::create_reader(&app, &admin_token, "renewhold").await;
    let (waiter_id, waiter_token) = fixtures::create_reader(&app, &admin_token, "renewwait").await;
    let (held_biblio, held_item, _) = create_copy(&app, &admin_token, "Hold Waiting").await;
    let held_loan = checkout(&app, &admin_token, holder_id, held_item).await;
    let held_loan_id = fixtures::json_id(&held_loan["id"]);
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": waiter_id.to_string(),
                "biblioId": held_biblio.to_string()
            }),
            Some(&waiter_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "title hold: {hold}");
    assert_same_refusal(&app, &holder_token, held_loan_id, "hold_waiting").await;

    let (capped_id, capped_token) = fixtures::create_reader(&app, &admin_token, "renewcap").await;
    let (_, capped_item, _) = create_copy(&app, &admin_token, "Max Renewals").await;
    let capped_loan = checkout(&app, &admin_token, capped_id, capped_item).await;
    let capped_loan_id = fixtures::json_id(&capped_loan["id"]);
    sqlx::query("UPDATE loans SET nb_renews = 5 WHERE id = $1")
        .bind(capped_loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("exhaust renewals");
    assert_same_refusal(&app, &capped_token, capped_loan_id, "max_renewals").await;

    let (early_id, early_token) = fixtures::create_reader(&app, &admin_token, "renewearly").await;
    let (_, early_item, _) = create_copy(&app, &admin_token, "Too Early").await;
    let early_loan = checkout(&app, &admin_token, early_id, early_item).await;
    let early_loan_id = fixtures::json_id(&early_loan["id"]);
    assert_same_refusal(&app, &early_token, early_loan_id, "renewal_too_early").await;

    let (ok_id, ok_token) = fixtures::create_reader(&app, &admin_token, "renewok").await;
    let (_, ok_item, _) = create_copy(&app, &admin_token, "Renewable").await;
    let ok_loan = checkout(&app, &admin_token, ok_id, ok_item).await;
    let ok_loan_id = fixtures::json_id(&ok_loan["id"]);
    fixtures::pull_loan_into_renewal_window(&app, ok_loan_id).await;
    let (status, list) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &ok_token)
        .await;
    assert_eq!(status, StatusCode::OK, "open loans: {list}");
    let row = find_row(list["items"].as_array().expect("items"), ok_loan_id);
    assert_eq!(row["canRenew"], true, "{row}");
    assert!(row["renewalRefusalCode"].is_null(), "{row}");
    assert!(row["renewalOpensAt"].is_null(), "{row}");
    let (status, renewed) = app
        .post_empty(
            &format!("/api/v1/loans/{ok_loan_id}/renew"),
            Some(&ok_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "renew: {renewed}");
}
