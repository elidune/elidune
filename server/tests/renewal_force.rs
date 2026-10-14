//! Desk force renewal (#134).
//!
//! The server puts `forceable` on every renewal 422. It is true only when the
//! caller has loans write and the code is liftable. The account-type name is
//! not consulted. `force` lifts `max_renewals` and `renewal_too_early` and
//! writes `loan.renewal_forced` (staff, reason, loan). A waiting hold, a
//! withdrawn copy, and a claims-returned copy stay refused.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

async fn create_copy(app: &TestApp, admin_token: &str, title: &str) -> (i64, i64, String) {
    let barcode = format!("FRC-{}", fixtures::unique_suffix());
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

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> i64 {
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
    fixtures::json_id(&body["id"])
}

/// Custom account type. The code is never `admin`, `librarian`, or `reader`.
async fn user_with_loans_rights(
    app: &TestApp,
    admin_token: &str,
    loans_rights: &str,
) -> (i64, String, String) {
    let code = format!("circ{}", fixtures::unique_suffix() % 1_000_000_000);
    assert!(!matches!(code.as_str(), "admin" | "librarian" | "reader"));
    let (status, body) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": format!("Circulation {code}"),
                "loansRights": loans_rights,
                "settingsRights": "n"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "account type: {body}");
    assert_eq!(body["loansRights"], loans_rights, "{body}");
    assert_eq!(body["isSystem"], false, "{body}");

    let login = format!("circ_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(app, admin_token, "adult").await;
    let (status, body) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Desk",
                "lastname": "Circulation",
                "email": format!("{login}@test.local"),
                "accountType": code,
                "publicType": public_type_id.to_string(),
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "user: {body}");
    let user_id = fixtures::json_id(&body["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (status, session) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "login: {session}");
    let token = session["token"].as_str().expect("token").to_string();
    (user_id, token, code)
}

fn assert_refusal(status: StatusCode, body: &Value, code: &str, forceable: bool) {
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    let mut keys: Vec<_> = body
        .as_object()
        .unwrap_or_else(|| panic!("object body: {body}"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, ["code", "error", "forceable", "message"], "{body}");
    assert_eq!(body["code"], code, "{body}");
    assert_eq!(body["error"], "Business Rule Violation", "{body}");
    assert_eq!(body["forceable"], forceable, "{body}");
    assert!(!body["message"].as_str().unwrap_or("").is_empty(), "{body}");
}

async fn renewal_count(app: &TestApp, loan_id: i64) -> i16 {
    sqlx::query_scalar("SELECT COALESCE(nb_renews, 0)::int2 FROM loans WHERE id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("renewal count")
}

async fn forced_audits(
    app: &TestApp,
    loan_id: i64,
    expected: usize,
) -> Vec<(Option<i64>, String, Value)> {
    let pool = app.state.services.repository.pool();
    for _ in 0..40 {
        let rows: Vec<(Option<i64>, String, Value)> = sqlx::query_as(
            r#"
            SELECT user_id, entity_type, payload
            FROM audit_log
            WHERE event_type = 'loan.renewal_forced' AND entity_id = $1
            ORDER BY id
            "#,
        )
        .bind(loan_id)
        .fetch_all(pool)
        .await
        .expect("audit query");
        if rows.len() >= expected {
            return rows;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("loan.renewal_forced audit missing for loan {loan_id}");
}

fn assert_forced_audit(
    rows: &[(Option<i64>, String, Value)],
    staff_id: i64,
    loan_id: i64,
    reasons: &[&str],
) {
    assert_eq!(rows.len(), reasons.len(), "{rows:?}");
    let mut found: Vec<&str> = rows
        .iter()
        .map(|row| {
            assert_eq!(row.0, Some(staff_id), "{row:?}");
            assert_eq!(row.1, "loan");
            assert_eq!(row.2["loanId"], loan_id.to_string(), "{row:?}");
            assert_eq!(row.2["staffId"], staff_id.to_string(), "{row:?}");
            row.2["reason"].as_str().unwrap_or("")
        })
        .collect();
    found.sort_unstable();
    let mut expected: Vec<&str> = reasons.to_vec();
    expected.sort_unstable();
    assert_eq!(found, expected, "{rows:?}");
}

#[tokio::test]
async fn loans_write_forces_max_renewals_and_renewal_too_early_with_audit() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "forcepatron").await;
    let (staff_id, desk_token, account_type) =
        user_with_loans_rights(&app, &admin_token, "w").await;
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", &desk_token).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["accountType"], account_type, "{me}");

    let (_, capped_item, _) = create_copy(&app, &admin_token, "Force Cap").await;
    let capped = checkout(&app, &admin_token, reader_id, capped_item).await;
    sqlx::query(
        "UPDATE loans SET nb_renews = 2, expiry_at = NOW() + interval '1 day' WHERE id = $1",
    )
    .bind(capped)
    .execute(app.state.services.repository.pool())
    .await
    .expect("put the loan at the renewal cap inside the window");

    let (status, refused) = app
        .post_empty(&format!("/api/v1/loans/{capped}/renew"), Some(&desk_token))
        .await;
    assert_refusal(status, &refused, "max_renewals", true);
    assert!(
        refused["message"]
            .as_str()
            .unwrap_or("")
            .contains("Maximum renewals reached"),
        "{refused}"
    );

    let (status, forced) = app
        .post_empty(
            &format!("/api/v1/loans/{capped}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{forced}");
    assert_eq!(fixtures::json_id(&forced["id"]), capped, "{forced}");
    assert_eq!(renewal_count(&app, capped).await, 3);
    let audits = forced_audits(&app, capped, 1).await;
    assert_forced_audit(&audits, staff_id, capped, &["max_renewals"]);

    let (_, early_item, barcode) = create_copy(&app, &admin_token, "Force Early").await;
    let early = checkout(&app, &admin_token, reader_id, early_item).await;
    let (status, refused) = app
        .post_empty(&format!("/api/v1/loans/{early}/renew"), Some(&desk_token))
        .await;
    assert_refusal(status, &refused, "renewal_too_early", true);
    assert!(
        refused["message"]
            .as_str()
            .unwrap_or("")
            .contains("Renewal is possible from"),
        "{refused}"
    );

    let (status, forced) = app
        .post_empty(
            &format!("/api/v1/loans/items/{barcode}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{forced}");
    assert_eq!(fixtures::json_id(&forced["id"]), early, "{forced}");
    assert_eq!(renewal_count(&app, early).await, 1);
    let audits = forced_audits(&app, early, 1).await;
    assert_forced_audit(&audits, staff_id, early, &["renewal_too_early"]);

    let (_, both_item, _) = create_copy(&app, &admin_token, "Force Both").await;
    let both = checkout(&app, &admin_token, reader_id, both_item).await;
    sqlx::query("UPDATE loans SET nb_renews = 2 WHERE id = $1")
        .bind(both)
        .execute(app.state.services.repository.pool())
        .await
        .expect("cap a loan that is also too early");
    let (status, forced) = app
        .post_empty(
            &format!("/api/v1/loans/{both}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{forced}");
    let audits = forced_audits(&app, both, 2).await;
    assert_forced_audit(
        &audits,
        staff_id,
        both,
        &["max_renewals", "renewal_too_early"],
    );
}

#[tokio::test]
async fn force_does_not_lift_hold_withdrawn_or_claimed_returned() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "blockpatron").await;
    let (waiter_id, _) = fixtures::create_reader(&app, &admin_token, "blockwait").await;
    let (_, desk_token, _) = user_with_loans_rights(&app, &admin_token, "w").await;

    let (biblio_id, held_item, _) = create_copy(&app, &admin_token, "Force Hold").await;
    let held = checkout(&app, &admin_token, reader_id, held_item).await;
    let (status, hold) = app
        .post_json(
            "/api/v1/holds",
            &json!({
                "userId": waiter_id.to_string(),
                "biblioId": biblio_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "hold: {hold}");
    let (status, refused) = app
        .post_empty(
            &format!("/api/v1/loans/{held}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_refusal(status, &refused, "hold_waiting", false);
    assert_eq!(renewal_count(&app, held).await, 0);

    let (_, withdrawn_item, _) = create_copy(&app, &admin_token, "Force Withdrawn").await;
    let withdrawn = checkout(&app, &admin_token, reader_id, withdrawn_item).await;
    sqlx::query("UPDATE items SET weeding_status = 'withdrawn' WHERE id = $1")
        .bind(withdrawn_item)
        .execute(app.state.services.repository.pool())
        .await
        .expect("withdraw the copy");
    let (status, refused) = app
        .post_empty(
            &format!("/api/v1/loans/{withdrawn}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_refusal(status, &refused, "withdrawn", false);
    assert_eq!(renewal_count(&app, withdrawn).await, 0);

    let (_, claimed_item, _) = create_copy(&app, &admin_token, "Force Claimed").await;
    let claimed = checkout(&app, &admin_token, reader_id, claimed_item).await;
    sqlx::query("UPDATE items SET circulation_status = 3 WHERE id = $1")
        .bind(claimed_item)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark claims returned");
    let (status, refused) = app
        .post_empty(
            &format!("/api/v1/loans/{claimed}/renew?force=true"),
            Some(&desk_token),
        )
        .await;
    assert_refusal(status, &refused, "claimed_returned", false);
    assert_eq!(renewal_count(&app, claimed).await, 0);

    let forced_rows: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint FROM audit_log
        WHERE event_type = 'loan.renewal_forced'
          AND entity_id IN ($1, $2, $3)
        "#,
    )
    .bind(held)
    .bind(withdrawn)
    .bind(claimed)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("count forced audits");
    assert_eq!(forced_rows, 0);
}

#[tokio::test]
async fn caller_without_loans_write_is_not_offered_force() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (user_id, token, account_type) = user_with_loans_rights(&app, &admin_token, "n").await;
    assert_ne!(account_type, "admin");
    let (_, item_id, _) = create_copy(&app, &admin_token, "No Loans Write").await;
    let loan_id = checkout(&app, &admin_token, user_id, item_id).await;

    let (status, refused) = app
        .post_empty(&format!("/api/v1/loans/{loan_id}/renew"), Some(&token))
        .await;
    assert_refusal(status, &refused, "renewal_too_early", false);

    let (status, denied) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    assert_eq!(denied["code"], "authorization_failed", "{denied}");
    assert!(denied.get("forceable").is_none(), "{denied}");
    assert_eq!(renewal_count(&app, loan_id).await, 0);
}
