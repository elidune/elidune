//! Barcode uniqueness among active copies and desk resolution.

mod common;

use axum::http::StatusCode;
use common::{fixtures, TestApp};
use serde_json::json;

async fn site(app: &TestApp, token: &str) -> i64 {
    let (status, body) = app.get_json_with_auth("/api/v1/sites", token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    fixtures::json_id(&body["items"][0]["id"])
}

#[tokio::test]
async fn active_barcode_unique_archived_may_duplicate() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let token = fixtures::ensure_first_setup(&app).await;
    let site_id = site(&app, &token).await;
    let shared = format!("DUP-{}", fixtures::unique_token());

    let (status, first) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("First {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{"barcode": shared, "borrowable": true, "siteId": site_id.to_string()}]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let first_id = fixtures::json_id(&first["biblio"]["items"][0]["id"]);

    app.delete_with_auth(&format!("/api/v1/items/{first_id}"), &token).await;

    let stored: String = sqlx::query_scalar("SELECT barcode FROM items WHERE id = $1")
        .bind(first_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap();
    assert_eq!(stored, shared, "delete must not rename barcode");

    let (status, second) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Second {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{"barcode": shared, "borrowable": true, "siteId": site_id.to_string()}]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");

    let (status, third) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Third {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{"barcode": shared, "borrowable": true, "siteId": site_id.to_string()}]
            }),
            Some(&token),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{third}");
}
