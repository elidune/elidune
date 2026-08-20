//! Domain MCP tools backed by Elidune services (no LLM-generated SQL).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    models::{biblio::BiblioQuery, user::UserClaims},
    AppState,
};

use super::schema_memo;

const DEFAULT_LOANS_LIMIT: i64 = 25;
const DEFAULT_SEARCH_LIMIT: i64 = 15;
const MAX_LOANS_LIMIT: i64 = 50;
const MAX_SEARCH_LIMIT: i64 = 25;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMyLoansArgs {
    #[serde(default = "default_loans_limit")]
    pub limit: i64,
}

fn default_loans_limit() -> i64 {
    DEFAULT_LOANS_LIMIT
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMyHoldsArgs {
    #[serde(default = "default_active_only")]
    pub active_only: bool,
}

fn default_active_only() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBibliosArgs {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
}

fn default_search_limit() -> i64 {
    DEFAULT_SEARCH_LIMIT
}

pub fn schema_overview_value() -> Value {
    json!({
        "overview": schema_memo::overview(),
        "toolPolicy": schema_memo::tool_policy(),
    })
}

pub async fn list_my_loans(state: &AppState, claims: &UserClaims, arguments: Value) -> AppResult<Value> {
    let args: ListMyLoansArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid list_my_loans arguments: {e}")))?;
    let limit = args.limit.clamp(1, MAX_LOANS_LIMIT);
    let (loans, total) = state.services.loans.get_user_loans(claims.user_id, 1, limit).await?;
    Ok(json!({
        "loans": loans,
        "total": total,
        "returned": loans.len(),
        "truncated": total > loans.len() as i64,
    }))
}

pub async fn list_my_holds(state: &AppState, claims: &UserClaims, arguments: Value) -> AppResult<Value> {
    let args: ListMyHoldsArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid list_my_holds arguments: {e}")))?;
    let holds = if args.active_only {
        let (rows, total) = state.services.holds.list_for_user_paginated(claims.user_id, 1, MAX_LOANS_LIMIT, true).await?;
        return Ok(json!({
            "holds": rows,
            "total": total,
            "activeOnly": true,
        }));
    } else {
        state.services.holds.get_for_user(claims.user_id).await?
    };
    Ok(json!({
        "holds": holds,
        "total": holds.len(),
        "activeOnly": false,
    }))
}

pub async fn search_biblios(state: &AppState, arguments: Value) -> AppResult<Value> {
    let args: SearchBibliosArgs = serde_json::from_value(arguments).map_err(|e| AppError::Validation(format!("Invalid search_biblios arguments: {e}")))?;
    let q = args.q.trim();
    if q.is_empty() {
        return Err(AppError::Validation("q must not be empty".into()));
    }
    let limit = args.limit.clamp(1, MAX_SEARCH_LIMIT);
    let query = BiblioQuery {
        freesearch: Some(q.to_string()),
        page: Some(1),
        per_page: Some(limit),
        include_without_active_items: Some(false),
        media_type: None,
        isbn: None,
        barcode: None,
        author: None,
        title: None,
        editor: None,
        lang: None,
        subject: None,
        content: None,
        keywords: None,
        audience_type: None,
        archive: None,
        serie: None,
        serie_id: None,
        collection: None,
        collection_id: None,
    };
    let (biblios, total) = state.services.catalog.search_biblios(&query).await?;
    Ok(json!({
        "biblios": biblios,
        "total": total,
        "returned": biblios.len(),
        "truncated": total > biblios.len() as i64,
    }))
}
