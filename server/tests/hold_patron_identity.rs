//! Hold patron identity on the biblio notice, the staff hold list, and the pick list.

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

fn assert_error_body(body: &Value) {
    for key in ["code", "error", "message"] {
        assert!(body.get(key).and_then(Value::as_str).is_some(), "error body missing {key}: {body}");
    }
}

fn assert_hidden(body: &Value, needles: &[&str]) {
    let text = body.to_string();
    for needle in needles {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
}

fn assert_minimized(body: &Value, user_id: i64, card: &str) {
    assert_eq!(body["id"], user_id.to_string(), "{body}");
    assert_eq!(body["cardNumber"], card, "{body}");
    assert_eq!(body["initials"], "HH", "{body}");
    let mut keys: Vec<&str> = body.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["cardNumber", "id", "initials"], "{body}");
}

fn assert_full(body: &Value, user_id: i64, login: &str, card: &str) {
    assert_eq!(body["id"], user_id.to_string(), "{body}");
    assert_eq!(body["firstname"], "Helene", "{body}");
    assert_eq!(body["lastname"], "Holder", "{body}");
    assert_eq!(body["login"], login, "{body}");
    assert_eq!(body["barcode"], card, "{body}");
    assert!(body.get("cardNumber").is_none(), "{body}");
    assert!(body.get("initials").is_none(), "{body}");
    assert!(body.get("email").is_none(), "{body}");
}

fn copy_by_barcode<'a>(body: &'a Value, barcode: &str) -> &'a Value {
    body["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["barcode"].as_str() == Some(barcode))
        .unwrap_or_else(|| panic!("copy {barcode} missing from {body}"))
}

struct HeldCopy {
    biblio_id: i64,
    hold_id: i64,
    barcode: String,
    holder_id: i64,
    card: String,
    login: String,
    email: String,
    phone: String,
}

async fn hold_a_copy(app: &TestApp, admin_token: &str) -> HeldCopy {
    let (holder_id, holder_token) = fixtures::create_reader(app, admin_token, "hold_id").await;
    let card = format!("CARD-{holder_id}");
    let email = format!("helene.holder.{holder_id}@secret.test");
    let phone = format!("06{holder_id}");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Helene',
            lastname = 'Holder',
            email = $2,
            phone = $3,
            barcode = $4
        WHERE id = $1
        "#,
    )
    .bind(holder_id)
    .bind(&email)
    .bind(&phone)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set holder identity");
    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(holder_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("holder login");

    let barcode = format!("HOLD-{}", fixtures::unique_suffix());
    let (status, biblio) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Hold privacy {barcode}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{biblio}");
    let biblio_id = fixtures::json_id(&biblio["biblio"]["id"]);

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
    assert_eq!(hold["status"], "to_pick", "{hold}");

    HeldCopy {
        biblio_id,
        hold_id: fixtures::json_id(&hold["id"]),
        barcode,
        holder_id,
        card,
        login,
        email,
        phone,
    }
}

fn secret_needles(held: &HeldCopy) -> Vec<&str> {
    vec!["Helene", "Holder", held.email.as_str(), held.phone.as_str(), held.login.as_str()]
}

fn hold_row(body: &Value, hold_id: i64) -> &Value {
    body.as_array()
        .expect("hold list")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == hold_id)
        .unwrap_or_else(|| panic!("hold {hold_id} missing from {body}"))
}

#[tokio::test]
async fn public_biblio_and_hold_routes_omit_hold_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let held = hold_a_copy(&app, &admin_token).await;
    let needles = secret_needles(&held);

    let (status, opac) = app.get_json(&format!("/api/v1/opac/biblios/{}", held.biblio_id)).await;
    assert_eq!(status, StatusCode::OK, "{opac}");
    let copy = copy_by_barcode(&opac, &held.barcode);
    assert_eq!(copy["onHold"], true, "{copy}");
    assert!(opac.get("holdsCount").is_some(), "{opac}");
    assert!(copy.get("holdFor").is_none(), "{copy}");
    assert_hidden(&opac, &needles);

    for path in ["/api/v1/holds", "/api/v1/holds/pick-list"] {
        let (status, body) = app.get_json(path).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path} {body}");
        assert_error_body(&body);
        assert_hidden(&body, &needles);
    }
}

#[tokio::test]
async fn patron_biblio_and_hold_list_omit_other_hold_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let held = hold_a_copy(&app, &admin_token).await;
    let (_other_id, other_token) = fixtures::create_reader(&app, &admin_token, "other_hold").await;
    let needles = secret_needles(&held);

    let (status, biblio) = app.get_json_with_auth(&format!("/api/v1/biblios/{}", held.biblio_id), &other_token).await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let copy = copy_by_barcode(&biblio, &held.barcode);
    assert_eq!(copy["onHold"], true, "{copy}");
    assert!(biblio.get("holdsCount").is_some(), "{biblio}");
    assert!(copy.get("holdFor").is_none(), "{copy}");
    assert_hidden(&biblio, &needles);

    let (status, holds) = app.get_json_with_auth("/api/v1/holds?activeOnly=true", &other_token).await;
    assert_eq!(status, StatusCode::OK, "{holds}");
    assert_hidden(&holds, &needles);

    let (status, slip) = app.get_json_with_auth("/api/v1/holds/pick-list", &other_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{slip}");
    assert_error_body(&slip);
    assert_hidden(&slip, &needles);
}

#[tokio::test]
async fn loans_only_staff_get_minimized_hold_for_on_biblio_list_and_slip() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let held = hold_a_copy(&app, &admin_token).await;
    let loans_only = retarget(&admin_token, &secret, Rights::None, Rights::Read);
    let needles = secret_needles(&held);

    let (status, biblio) = app.get_json_with_auth(&format!("/api/v1/biblios/{}", held.biblio_id), &loans_only).await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let copy = copy_by_barcode(&biblio, &held.barcode);
    assert_eq!(copy["availability"], "onHold", "{copy}");
    assert_minimized(&copy["holdFor"], held.holder_id, &held.card);
    assert_hidden(&biblio, &needles);

    let (status, labels) = app.get_json_with_auth("/api/v1/holds/pick-list", &loans_only).await;
    assert_eq!(status, StatusCode::OK, "{labels}");
    let label = labels
        .as_array()
        .expect("pick list")
        .iter()
        .find(|row| fixtures::json_id(&row["holdId"]) == held.hold_id)
        .expect("label");
    assert_minimized(&label["holdFor"], held.holder_id, &held.card);
    assert!(label["pickDueAt"].is_string(), "{label}");
    assert!(label["pickupDeadline"].is_null(), "{label}");
    assert!(label.get("userName").is_none(), "{label}");
    assert_hidden(label, &needles);

    let (status, returned) = app.post_empty(&format!("/api/v1/loans/items/{}/return", held.barcode), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{returned}");

    let (status, holds) = app.get_json_with_auth(&format!("/api/v1/biblios/{}/holds", held.biblio_id), &loans_only).await;
    assert_eq!(status, StatusCode::OK, "{holds}");
    let row = hold_row(&holds, held.hold_id);
    assert_eq!(row["status"], "ready", "{row}");
    assert_minimized(&row["user"], held.holder_id, &held.card);
    assert!(row["expiresAt"].is_string(), "pickup deadline: {row}");
    assert_hidden(row, &needles);
}

#[tokio::test]
async fn users_read_staff_get_full_hold_patron_on_biblio_list_and_slip() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let secret = app.state.config.users.jwt_secret.clone();
    let held = hold_a_copy(&app, &admin_token).await;
    let users_read = retarget(&admin_token, &secret, Rights::Read, Rights::None);

    let (status, biblio) = app.get_json_with_auth(&format!("/api/v1/biblios/{}", held.biblio_id), &users_read).await;
    assert_eq!(status, StatusCode::OK, "{biblio}");
    let copy = copy_by_barcode(&biblio, &held.barcode);
    assert_full(&copy["holdFor"], held.holder_id, &held.login, &held.card);
    assert!(copy.get("borrower").is_none(), "{copy}");
    assert_hidden(&copy["holdFor"], &[held.email.as_str(), held.phone.as_str()]);

    let (status, labels) = app.get_json_with_auth("/api/v1/holds/pick-list", &users_read).await;
    assert_eq!(status, StatusCode::OK, "{labels}");
    let label = labels
        .as_array()
        .expect("pick list")
        .iter()
        .find(|row| fixtures::json_id(&row["holdId"]) == held.hold_id)
        .expect("label");
    assert_minimized(&label["holdFor"], held.holder_id, &held.card);
    assert!(label["pickDueAt"].is_string(), "{label}");
    assert_hidden(label, &secret_needles(&held));

    let (status, returned) = app.post_empty(&format!("/api/v1/loans/items/{}/return", held.barcode), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "{returned}");

    let (status, holds) = app.get_json_with_auth(&format!("/api/v1/biblios/{}/holds", held.biblio_id), &users_read).await;
    assert_eq!(status, StatusCode::OK, "{holds}");
    let row = hold_row(&holds, held.hold_id);
    assert_full(&row["user"], held.holder_id, &held.login, &held.card);
    assert!(row["expiresAt"].is_string(), "pickup deadline: {row}");
    assert_hidden(&row["user"], &[held.email.as_str(), held.phone.as_str()]);
}
