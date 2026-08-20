//! Minimal MCP Streamable HTTP JSON-RPC (stateless). Authenticated via Elidune JWT.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::AuthenticatedUser;
use crate::error::AppError;
use crate::models::user::UserClaims;
use crate::AppState;

use super::tools;

const PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub async fn handle_post(State(state): State<AppState>, AuthenticatedUser(claims): AuthenticatedUser, body: BytesOrJson) -> Response {
    if state.mcp_pool.is_none() {
        return json_rpc_http_error(StatusCode::SERVICE_UNAVAILABLE, "MCP is disabled or the MCP database role is unavailable");
    }

    let raw = body.0;
    if raw.is_null() {
        return json_rpc_http_error(StatusCode::BAD_REQUEST, "Empty JSON-RPC body");
    }

    if let Some(arr) = raw.as_array() {
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            match serde_json::from_value::<JsonRpcRequest>(item.clone()) {
                Ok(req) => out.push(dispatch(&state, &claims, req).await),
                Err(e) => out.push(error_response(None, -32700, format!("Parse error: {e}"))),
            }
        }
        return json_response(json!(out));
    }

    match serde_json::from_value::<JsonRpcRequest>(raw) {
        Ok(req) => {
            if req.id.is_none() {
                // Notification: process, no body.
                let _ = dispatch(&state, &claims, req).await;
                return StatusCode::ACCEPTED.into_response();
            }
            let resp = dispatch(&state, &claims, req).await;
            json_response(serde_json::to_value(resp).unwrap_or(json!({})))
        }
        Err(e) => json_rpc_http_error(StatusCode::BAD_REQUEST, &format!("Parse error: {e}")),
    }
}

pub async fn handle_get() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({
            "error": "SSE GET is not used; this MCP server is stateless Streamable HTTP (POST JSON-RPC)"
        })),
    )
}

pub async fn handle_delete() -> impl IntoResponse {
    StatusCode::METHOD_NOT_ALLOWED
}

async fn dispatch(state: &AppState, claims: &UserClaims, req: JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => ok(req.id, initialize_result()),
        "notifications/initialized" | "notifications/cancelled" => ok(req.id, json!({})),
        "ping" => ok(req.id, json!({})),
        "tools/list" => ok(req.id, json!({ "tools": tools::tool_defs() })),
        "tools/call" => match call_params(&req.params) {
            Ok((name, args)) => match tools::call_tool(state, claims, &name, args).await {
                Ok(value) => ok(
                    req.id,
                    json!({
                        "content": [{ "type": "text", "text": value.to_string() }],
                        "structuredContent": value,
                        "isError": false
                    }),
                ),
                Err(e) => tool_error(req.id, e),
            },
            Err(msg) => error_response(req.id, -32602, msg),
        },
        other => error_response(req.id, -32601, format!("Method not found: {other}")),
    }
}

fn call_params(params: &Value) -> Result<(String, Value), String> {
    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| "tools/call requires params.name".to_string())?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    Ok((name.to_string(), args))
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "elidune",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Read-only SQL over the Elidune library database. Use list_tables / describe_table / query. Patron accounts cannot see other users' personal data. Passwords and TOTP secrets are never exposed."
    })
}

fn ok(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(id: Option<Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError { code, message, data: None }),
    }
}

fn tool_error(id: Option<Value>, err: AppError) -> JsonRpcResponse {
    let (_status, _code, message) = err.audit_http_fields();
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        })),
        error: None,
    }
}

fn json_response(body: Value) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

fn json_rpc_http_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32000, "message": message },
            "id": null
        })),
    )
        .into_response()
}

/// Accept a raw JSON value (object or batch array).
pub struct BytesOrJson(pub Value);

#[axum::async_trait]
impl<S> axum::extract::FromRequest<S> for BytesOrJson
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, Json(json!({"error": "Failed to read body"}))))?;
        if bytes.is_empty() {
            return Ok(Self(Value::Null));
        }
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"jsonrpc":"2.0","error":{"code":-32700,"message":e.to_string()},"id":null}))))
    }
}
