//! API integration tests

use reqwest::Client;
use serde_json::{json, Value};

const BASE_URL: &str = "http://localhost:8080/api/v1";

/// Helper to get an authenticated client
async fn get_auth_token(client: &Client) -> String {
    let response = client
        .post(format!("{}/auth/login", BASE_URL))
        .json(&json!({
            "username": "admin",
            "password": "admin"
        }))
        .send()
        .await
        .expect("Failed to send login request");

    let body: Value = response.json().await.expect("Failed to parse login response");
    body["token"].as_str().expect("No token in response").to_string()
}

#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_health_check() {
    let client = Client::new();
    
    let response = client
        .get(format!("{}/health", BASE_URL))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
#[ignore]
async fn test_login() {
    let client = Client::new();
    
    let response = client
        .post(format!("{}/auth/login", BASE_URL))
        .json(&json!({
            "username": "admin",
            "password": "admin"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert!(body["token"].is_string());
    assert_eq!(body["token_type"], "Bearer");
}

#[tokio::test]
#[ignore]
async fn test_login_invalid_credentials() {
    let client = Client::new();
    
    let response = client
        .post(format!("{}/auth/login", BASE_URL))
        .json(&json!({
            "username": "admin",
            "password": "wrong"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}

#[tokio::test]
#[ignore]
async fn test_get_current_user() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .get(format!("{}/auth/me", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["username"], "admin");
}

#[tokio::test]
#[ignore]
async fn test_list_items() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .get(format!("{}/items", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert!(body["items"].is_array());
    assert!(body["total"].is_number());
}

#[tokio::test]
#[ignore]
async fn test_create_and_delete_item() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    // Create item
    let response = client
        .post(format!("{}/items", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "title1": "Test Book",
            "media_type": "b",
            "identification": "978-0-00-000000-0"
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 201);
    
    let body: Value = response.json().await.expect("Failed to parse response");
    let item_id = body["id"].as_i64().expect("No item ID");

    // Delete item
    let response = client
        .delete(format!("{}/items/{}?force=true", BASE_URL, item_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 204);
}

#[tokio::test]
#[ignore]
async fn test_list_users() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .get(format!("{}/users", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert!(body["items"].is_array());
}

#[tokio::test]
#[ignore]
async fn test_create_user() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .post(format!("{}/users", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({
            "login": "testuser",
            "password": "testpass",
            "firstname": "Test",
            "lastname": "User",
            "account_type_id": 2
        }))
        .send()
        .await
        .expect("Failed to send request");

    if response.status().is_success() {
        let body: Value = response.json().await.expect("Failed to parse response");
        let user_id = body["id"].as_i64().expect("No user ID");

        // Cleanup: delete the user
        let _ = client
            .delete(format!("{}/users/{}?force=true", BASE_URL, user_id))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await;
    }
}

#[tokio::test]
#[ignore]
async fn test_get_stats() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .get(format!("{}/stats", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert!(body["items"]["total"].is_number());
    assert!(body["users"]["total"].is_number());
    assert!(body["loans"]["active"].is_number());
}

#[tokio::test]
#[ignore]
async fn test_get_settings() {
    let client = Client::new();
    let token = get_auth_token(&client).await;
    
    let response = client
        .get(format!("{}/settings", BASE_URL))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("Failed to send request");

    assert!(response.status().is_success());
    
    let body: Value = response.json().await.expect("Failed to parse response");
    assert!(body["loan_settings"].is_array());
    assert!(body["z3950_servers"].is_array());
}

#[tokio::test]
#[ignore]
async fn test_unauthorized_access() {
    let client = Client::new();
    
    let response = client
        .get(format!("{}/items", BASE_URL))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 401);
}


