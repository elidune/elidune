//! OPAC (Online Public Access Catalog) — public, unauthenticated read-only endpoints
//!
//! Suitable for patron self-service kiosks, embedded catalog widgets, or public websites.
//! No authentication required. Only non-sensitive catalog data is exposed.

use axum::{
    extract::{Path, Query, State},
    Json,
};

use crate::{
    api::biblios::PaginatedResponse,
    error::{AppError, AppResult},
    models::{
        biblio::BiblioQuery,
        catalog_view::{BiblioPublicView, BiblioShortPublic},
    },
};

pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::get;
    axum::Router::new()
        .route("/opac/biblios", get(opac_search))
        .route("/opac/biblios/:id", get(opac_get_biblio))
        .route("/opac/biblios/:id/availability", get(opac_availability))
}

/// Public catalog search — no auth required
#[utoipa::path(
    get,
    path = "/opac/biblios",
    tag = "opac",
    params(
        ("title" = Option<String>, Query, description = "Search in title"),
        ("author" = Option<String>, Query, description = "Search by author"),
        ("isbn" = Option<String>, Query, description = "Search by ISBN"),
        ("freesearch" = Option<String>, Query, description = "Full-text search"),
        ("media_type" = Option<String>, Query, description = "Filter by media type"),
        ("serie" = Option<String>, Query, description = "Filter by series name (substring)"),
        ("serie_id" = Option<i64>, Query, description = "Filter by series ID (exact match)"),
        ("collection" = Option<String>, Query, description = "Filter by collection name (substring)"),
        ("collection_id" = Option<i64>, Query, description = "Filter by collection ID (exact match)"),
        ("include_without_active_items" = Option<bool>, Query, description = "If true, include biblios with no active items; default excludes them (patron catalogue)"),
        ("page" = Option<i64>, Query, description = "Page number (default 1)"),
        ("per_page" = Option<i64>, Query, description = "Items per page (default 20, max 50)")
    ),
    responses(
        (status = 200, description = "Catalog search results. Copies are `ItemShortPublic`. Lost, declared missing, weeding-in-progress, and withdrawn copies are omitted; the public count is the length of `items`. Each remaining copy has `availability` of `available`, `onLoan`, `onSiteOnly`, or `unavailable`, plus `canHoldItem`. List rows omit copy notes. No `holdFor`, borrower, login, `publicNote`, `internalNote`, bibliographic notes, or weeding fields.", body = PaginatedResponse<BiblioShortPublic>)
    )
)]
pub async fn opac_search(
    State(state): State<crate::AppState>,
    Query(mut query): Query<BiblioQuery>,
) -> AppResult<Json<PaginatedResponse<BiblioShortPublic>>> {
    // Cap per_page to prevent abuse on public endpoint
    let per_page = query.per_page.unwrap_or(20).min(50);
    let page = query.page.unwrap_or(1).max(1);
    query.per_page = Some(per_page);
    query.page = Some(page);
    query.exclude_fully_withdrawn = Some(true);

    let (biblios, total) = state.services.catalog.search_biblios(&query).await?;
    Ok(Json(PaginatedResponse::new(
        biblios.into_iter().map(BiblioShortPublic::from).collect(),
        total,
        page,
        per_page,
    )))
}

/// Get a single bibliographic record by ID — public
#[utoipa::path(
    get,
    path = "/opac/biblios/{id}",
    tag = "opac",
    params(("id" = i64, Path, description = "Biblio ID")),
    responses(
        (status = 200, description = "Bibliographic record details as `BiblioPublicView`. Lost, declared missing, weeding-in-progress, and withdrawn copies are omitted. Remaining copies use `available`, `onLoan`, `onSiteOnly`, or `unavailable`. The record includes `holdsCount` (pending holds still waiting for a copy; no patron). Each remaining copy omits `dueDate` (the key is absent, not null), includes `publicNote` (null when empty), and includes `onHold` (true only when this copy is `to_pick` or `ready`, or in transit for a hold). An unassigned `pending` title hold does not set `onHold`. A copy with `availability` `onLoan` includes `expectedReturnDate` (`YYYY-MM-DD` in the library timezone, no time) while that local date is today or later. The field is omitted when the local due date is before today, and for every other availability. The comparison uses `library_info.timezone`, not UTC. No `holdFor`, borrower, loan id, login, `internalNote`, bibliographic `notes`, or weeding fields.", body = BiblioPublicView),
        (status = 404, description = "Biblio not found", body = crate::error::ErrorResponse)
    )
)]
pub async fn opac_get_biblio(
    State(state): State<crate::AppState>,
    Path(biblio_id): Path<i64>,
) -> AppResult<Json<BiblioPublicView>> {
    let biblio = state.services.catalog.get_biblio(biblio_id).await?;
    if biblio.is_fully_withdrawn() {
        return Err(AppError::NotFound("Bibliographic record not found".into()));
    }
    Ok(Json(BiblioPublicView::from(biblio)))
}

/// Get availability for a bibliographic record (how many physical copies are available)
#[utoipa::path(
    get,
    path = "/opac/biblios/{id}/availability",
    tag = "opac",
    params(("id" = i64, Path, description = "Biblio ID")),
    responses(
        (status = 200, description = "Availability count", body = serde_json::Value)
    )
)]
pub async fn opac_availability(
    State(state): State<crate::AppState>,
    Path(biblio_id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    let biblio = state.services.catalog.get_biblio(biblio_id).await?;
    if biblio.is_fully_withdrawn() {
        return Err(AppError::NotFound("Bibliographic record not found".into()));
    }
    let active_loans = state
        .services
        .loans
        .count_active_for_biblio(biblio_id)
        .await?;
    let hold_count = state
        .services
        .holds
        .count_active_for_biblio(biblio_id)
        .await?;
    Ok(Json(serde_json::json!({
        "biblioId": biblio_id.to_string(),
        "activeLoans": active_loans,
        "holdCount": hold_count,
    })))
}
