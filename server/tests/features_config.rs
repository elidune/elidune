//! Instance feature flags: library-info exposure and API enforcement.

mod common;

use axum::http::StatusCode;
use common::{fixtures, TestApp};
use once_cell::sync::Lazy;
use serde_json::json;
use tokio::sync::Mutex;

static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[tokio::test]
async fn library_info_exposes_enabled_features_defaults() {
    let _guard = TEST_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _ = fixtures::ensure_first_setup(&app).await;

    let (status, body) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let features = &body["enabledFeatures"];
    assert_eq!(features["acquisitions"], true);
    assert_eq!(features["purchaseSuggestions"], true);
    assert_eq!(features["weeding"], true);
}

#[tokio::test]
async fn disabling_purchase_suggestions_blocks_api_and_updates_library_info() {
    let _guard = TEST_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let mut value = serde_json::to_value(app.state.dynamic_config.read_features()).unwrap();
    value["purchase_suggestions"] = json!(false);

    let (status, body) = app.put_json("/api/v1/admin/config/features", &json!({ "value": value }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "put features: {body}");
    assert_eq!(body["value"]["purchase_suggestions"], false);

    let (status, info) = app.get_json("/api/v1/library-info").await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["enabledFeatures"]["purchaseSuggestions"], false);
    assert_eq!(info["enabledFeatures"]["acquisitions"], true);

    let (status, body) = app.post_json("/api/v1/suggestions", &json!({ "title": "Hidden Book", "author": "Nobody" }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "feature_disabled");

    value["purchase_suggestions"] = json!(true);
    let (status, _) = app.put_json("/api/v1/admin/config/features", &json!({ "value": value }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn disabling_acquisitions_blocks_vendors_list() {
    let _guard = TEST_LOCK.lock().await;
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let mut value = serde_json::to_value(app.state.dynamic_config.read_features()).unwrap();
    value["acquisitions"] = json!(false);

    let (status, body) = app.put_json("/api/v1/admin/config/features", &json!({ "value": value }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK, "put features: {body}");

    let (status, body) = app.get_json_with_auth("/api/v1/acquisitions/vendors", &admin_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "feature_disabled");

    value["acquisitions"] = json!(true);
    let (status, _) = app.put_json("/api/v1/admin/config/features", &json!({ "value": value }), Some(&admin_token)).await;
    assert_eq!(status, StatusCode::OK);
}
