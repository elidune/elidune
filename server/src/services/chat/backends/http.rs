//! Shared HTTP client helpers for LLM backends.

use reqwest::Client;

pub fn llm_client(timeout_secs: u64) -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub fn trim_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}
