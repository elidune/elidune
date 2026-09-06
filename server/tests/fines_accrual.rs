//! Overdue fine accrual: grace, cap, and one-open-fine-per-loan idempotency.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

async fn unlock_admin(app: &TestApp, admin_token: &str) {
    let (status, me) = app.get_json_with_auth("/api/v1/auth/me", admin_token).await;
    if status != StatusCode::OK {
        return;
    }
    let admin_id = fixtures::json_id(&me["id"]);
    let _ = app
        .state
        .services
        .users
        .set_must_change_password(admin_id, false)
        .await;
}

fn decimal_str(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_f64().map(|n| format!("{n:.2}")))
        .unwrap_or_else(|| value.to_string().trim_matches('"').to_string())
}

async fn seed_catalog(app: &TestApp, admin_token: &str, barcode: &str) -> i64 {
    let payload = json!({
        "title": format!("Fine Accrual {barcode}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": barcode,
            "borrowable": true
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

async fn checkout(app: &TestApp, admin_token: &str, user_id: i64, item_id: i64) -> i64 {
    let payload = json!({
        "userId": user_id.to_string(),
        "itemId": item_id.to_string(),
        "force": true
    });
    let (status, body) = app
        .post_json("/api/v1/loans", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    fixtures::json_id(&body["id"])
}

async fn backdate_loan(app: &TestApp, loan_id: i64, overdue_days: i64) {
    sqlx::query("UPDATE loans SET expiry_at = NOW() - ($1 || ' days')::interval WHERE id = $2")
        .bind(overdue_days)
        .bind(loan_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("backdate loan");
}

#[tokio::test]
async fn accrue_single_loan_grace_cap_and_idempotency() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "fineaccrue").await;
    let item_id = seed_catalog(
        &app,
        &admin_token,
        &format!("FA-{}", fixtures::unique_suffix()),
    )
    .await;
    let loan_id = checkout(&app, &admin_token, reader_id, item_id).await;

    let rule = json!({
        "dailyRate": "0.50",
        "maxAmount": "10.00",
        "graceDays": 3
    });
    let (status, body) = app
        .put_json("/api/v1/fines/rules", &rule, Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "upsert rule: {body}");

    backdate_loan(&app, loan_id, 2).await;
    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "grace: {body}");

    let (status, listed) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed["fines"].as_array().unwrap().is_empty());

    backdate_loan(&app, loan_id, 10).await;
    let (status, first) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "first accrue: {first}");
    assert_eq!(first["outcome"], "created");
    assert_eq!(first["breakdown"]["billableDays"], 7);
    assert_eq!(decimal_str(&first["breakdown"]["amount"]), "3.50");
    let fine_id = first["fine"]["id"].clone();

    let (status, second) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "second accrue: {second}");
    assert_eq!(second["outcome"], "unchanged");
    assert_eq!(second["fine"]["id"], fine_id);

    backdate_loan(&app, loan_id, 13).await;
    let (status, third) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "updated accrue: {third}");
    assert_eq!(third["outcome"], "updated");
    assert_eq!(third["fine"]["id"], fine_id);
    assert_eq!(decimal_str(&third["fine"]["amount"]), "5.00");

    backdate_loan(&app, loan_id, 100).await;
    let (status, capped) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "capped accrue: {capped}");
    assert_eq!(capped["fine"]["id"], fine_id);
    assert_eq!(capped["breakdown"]["capped"], true);
    assert_eq!(decimal_str(&capped["fine"]["amount"]), "10.00");

    let (status, listed) = app
        .get_json_with_auth(&format!("/api/v1/users/{reader_id}/fines"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["fines"].as_array().unwrap().len(), 1);

    let (status, batch) = app
        .post_empty(
            &format!("/api/v1/users/{reader_id}/fines/accrue"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "patron batch: {batch}");
    assert_eq!(batch["unchanged"], 1);
    assert_eq!(batch["created"], 0);

    let (status, _) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/fines/accrue"),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn accrue_batch_all_overdue_via_staff_path() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    unlock_admin(&app, &admin_token).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "finebatch").await;
    let item_id = seed_catalog(
        &app,
        &admin_token,
        &format!("FB-{}", fixtures::unique_suffix()),
    )
    .await;
    let loan_id = checkout(&app, &admin_token, reader_id, item_id).await;

    let rule = json!({
        "dailyRate": "0.50",
        "maxAmount": "10.00",
        "graceDays": 3
    });
    let (status, body) = app
        .put_json("/api/v1/fines/rules", &rule, Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "upsert rule: {body}");

    backdate_loan(&app, loan_id, 10).await;

    let (status, report) = app
        .post_json(
            "/api/v1/fines/accrue",
            &json!({ "userId": reader_id.to_string() }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "batch accrue: {report}");
    assert_eq!(report["created"], 1);
    assert_eq!(report["results"][0]["loanId"], loan_id.to_string());
}
