//! `users_rights = own` may read and update only the caller's contact record (#131).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::{json, Value};

fn retarget_users(token: &str, secret: &str, users: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.users_rights = users;
    claims.create_token(secret).expect("mint test token")
}

async fn user_of(app: &TestApp, token: &str, id: i64) -> (StatusCode, Value) {
    app.get_json_with_auth(&format!("/api/v1/users/{id}"), token)
        .await
}

#[tokio::test]
async fn own_rights_get_and_put_contact_fields_only() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "ownrec").await;
    let (other_id, _) = fixtures::create_reader(&app, &admin_token, "ownother").await;
    let secret = app.state.config.users.jwt_secret.clone();

    let (status, own) = user_of(&app, &reader_token, reader_id).await;
    assert_eq!(status, StatusCode::OK, "{own}");
    assert_eq!(fixtures::json_id(&own["id"]), reader_id, "{own}");
    assert_eq!(own["accountType"], "reader", "{own}");

    let (status, other) = user_of(&app, &reader_token, other_id).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{other}");

    let no_users = retarget_users(&reader_token, &secret, Rights::None);
    let (status, denied) = user_of(&app, &no_users, reader_id).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, staff_read) = user_of(&app, &admin_token, reader_id).await;
    assert_eq!(status, StatusCode::OK, "{staff_read}");

    let (status, updated) = app
        .put_json(
            &format!("/api/v1/users/{reader_id}"),
            &json!({
                "email": "ada.own@library.test",
                "phone": "0601020304",
                "addrStreet": "1 Rue des Livres",
                "addrZipCode": 75001,
                "addrCity": "Paris",
                "language": "english",
                "eventsConsent": true,
                "receiveReminders": false
            }),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["email"], "ada.own@library.test", "{updated}");
    assert_eq!(updated["phone"], "0601020304", "{updated}");
    assert_eq!(updated["addrStreet"], "1 Rue des Livres", "{updated}");
    assert_eq!(updated["addrZipCode"], 75001, "{updated}");
    assert_eq!(updated["addrCity"], "Paris", "{updated}");
    assert_eq!(updated["language"], "english", "{updated}");
    assert_eq!(updated["eventsConsent"], true, "{updated}");
    assert_eq!(updated["receiveReminders"], false, "{updated}");
    assert_eq!(updated["accountType"], "reader", "{updated}");

    let (status, again) = user_of(&app, &reader_token, reader_id).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["email"], "ada.own@library.test", "{again}");
    assert_eq!(again["language"], "english", "{again}");

    let forbidden = [
        json!({"email": "stolen@library.test", "accountType": "admin"}),
        json!({"email": "stolen@library.test", "usersRights": "w"}),
        json!({"email": "stolen@library.test", "rights": {"itemsRights": "w"}}),
        json!({"email": "stolen@library.test", "barcode": "NEWCARD"}),
        json!({"email": "stolen@library.test", "cardNumber": "NEWCARD"}),
        json!({"email": "stolen@library.test", "status": "blocked"}),
        json!({"email": "stolen@library.test", "fee": "local"}),
        json!({"email": "stolen@library.test", "fines": 500}),
        json!({"email": "stolen@library.test", "firstname": "Changed"}),
    ];
    for body in forbidden {
        let (status, rejected) = app
            .put_json(
                &format!("/api/v1/users/{reader_id}"),
                &body,
                Some(&reader_token),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body} -> {rejected}");
        let (status, current) = user_of(&app, &reader_token, reader_id).await;
        assert_eq!(status, StatusCode::OK, "{current}");
        assert_eq!(
            current["email"], "ada.own@library.test",
            "{body} wrote {current}"
        );
        assert_eq!(current["accountType"], "reader", "{current}");
        assert_eq!(current["phone"], "0601020304", "{current}");
        assert_ne!(current["status"], "blocked", "{current}");
    }

    let (status, bad_email) = app
        .put_json(
            &format!("/api/v1/users/{reader_id}"),
            &json!({"email": "not-an-email"}),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad_email}");
    let (status, current) = user_of(&app, &reader_token, reader_id).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["email"], "ada.own@library.test", "{current}");

    let (status, other_put) = app
        .put_json(
            &format!("/api/v1/users/{other_id}"),
            &json!({"email": "other@library.test"}),
            Some(&reader_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{other_put}");
    let (status, other_user) = user_of(&app, &admin_token, other_id).await;
    assert_eq!(status, StatusCode::OK, "{other_user}");
    assert_ne!(other_user["email"], "other@library.test", "{other_user}");

    let expiry_at = current["expiryAt"].clone();
    let (status, staff_put) = app
        .put_json(
            &format!("/api/v1/users/{reader_id}"),
            &json!({
                "phone": "0699999999",
                "expiryAt": expiry_at
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{staff_put}");
    assert_eq!(staff_put["phone"], "0699999999", "{staff_put}");
    assert_eq!(staff_put["email"], "ada.own@library.test", "{staff_put}");
}
