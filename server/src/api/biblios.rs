//! Biblio (catalog) and Item (physical copy) endpoints

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult},
    models::catalog_view::{project_borrower, sees_internal_catalog, BiblioPublicView, BiblioShortPublic, BiblioShortStaff, BiblioStaffView, ItemPublicView, ItemStaffView},
    models::task::TaskKind,
    models::user::Rights,
    models::{
        biblio::{Biblio, BiblioQuery},
        import_report::ImportReport,
        item::Item,
    },
    models::dto::deposit_import::{DepositDuplicateAction, DepositDuplicatePolicy, DepositDuplicateScanReport, ScanDepositDuplicatesQuery},
    services::{
        audit::{self},
        marc::{EnqueueResult, MarcBatchInfo},
    },
};

use super::{tasks::TaskAcceptedResponse, AuthenticatedUser, ClientIp, ValidatedJson};

/// Build biblio routes (list/create items under a biblio live here; update/delete copy via [`crate::api::items`]).
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/biblios", get(list_biblios).post(create_biblio))
        .route("/biblios/:id", get(get_biblio).put(update_biblio).delete(delete_biblio))
        .route("/biblios/:id/items", get(list_items).post(create_item))
        .route("/biblios/export.csv", get(export_biblios_csv))
        .route("/biblios/load-marc", post(load_marc))
        .route("/biblios/import-marc-batch/scan-deposit-duplicates", post(scan_deposit_marc_duplicates))
        .route("/biblios/import-marc-batch", post(import_marc_batch))
        .route("/biblios/list-marc-batches", get(list_marc_batches))
        .route("/biblios/marc-batch/:batch_id", get(load_marc_batch))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetBiblioQuery {
    /// If true, include the full MARC record (marc_record JSONB) in the response
    #[serde(default)]
    pub full_record: bool,
}

/// Generic paginated response wrapper returned by list endpoints.
///
/// All list endpoints return this envelope so clients have a consistent way
/// to read pagination metadata without inspecting headers.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T>
where
    T: for<'a> ToSchema<'a>,
{
    /// Page contents
    pub items: Vec<T>,
    /// Total number of matching records across all pages
    pub total: i64,
    /// Current 1-based page number
    pub page: i64,
    /// Maximum records per page
    pub per_page: i64,
    /// Total number of pages (`ceil(total / per_page)`)
    pub page_count: i64,
}

/// Staff or reader bibliographic record. The handler selects the variant with
/// [`crate::models::catalog_view::sees_internal_catalog`].
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum BiblioReadResponse {
    Staff(BiblioStaffView),
    Public(BiblioPublicView),
}

/// Staff or reader catalog page.
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum BiblioSearchResponse {
    Staff(PaginatedResponse<BiblioShortStaff>),
    Public(PaginatedResponse<BiblioShortPublic>),
}

/// Staff or reader copy list.
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum ItemListResponse {
    Staff(Vec<ItemStaffView>),
    Public(Vec<ItemPublicView>),
}

/// Staff or reader series/collection bibliography.
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum BiblioShortListResponse {
    Staff(Vec<BiblioShortStaff>),
    Public(Vec<BiblioShortPublic>),
}

impl<T: for<'a> ToSchema<'a>> PaginatedResponse<T> {
    /// Construct a paginated response, calculating `page_count` automatically.
    pub fn new(items: Vec<T>, total: i64, page: i64, per_page: i64) -> Self {
        let page_count = if per_page > 0 { (total + per_page - 1) / per_page } else { 0 };
        Self {
            items,
            total,
            page,
            per_page,
            page_count,
        }
    }
}

/// List biblios with search and pagination
#[utoipa::path(
    get,
    path = "/biblios",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("mediaType" = Option<String>, Query, description = "Filter by media type"),
        ("title" = Option<String>, Query, description = "Search in title"),
        ("author" = Option<String>, Query, description = "Search by author"),
        ("isbn" = Option<String>, Query, description = "Search by ISBN/ISSN"),
        ("freesearch" = Option<String>, Query, description = "Full-text search"),
        ("serie" = Option<String>, Query, description = "Filter by series name (substring)"),
        ("serieId" = Option<i64>, Query, description = "Filter by series ID (exact match)"),
        ("collection" = Option<String>, Query, description = "Filter by collection name (substring)"),
        ("collectionId" = Option<i64>, Query, description = "Filter by collection ID (exact match)"),
        ("includeWithoutActiveItems" = Option<bool>, Query, description = "If true, include biblios with no active (non-archived) items; default excludes them"),
        ("page" = Option<i64>, Query, description = "Page number (default: 1)"),
        ("perPage" = Option<i64>, Query, description = "Items per page (default: 20)")
    ),
    responses(
        (status = 200, description = "List of bibliographic records. Callers with internal catalog rights receive `BiblioShortStaff` (full availability, `circulationStatus`, weeding, hold patron, real copy total). Catalog read alone receives `BiblioShortPublic`: lost, declared missing, weeding-in-progress, and withdrawn copies are omitted. Remaining copies use `available`, `onLoan`, `onSiteOnly`, or `unavailable`. List rows omit copy notes. No borrower, `holdFor`, login, `publicNote`, `internalNote`, bibliographic notes, or weeding fields.", body = BiblioSearchResponse),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn list_biblios(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Query(mut query): Query<BiblioQuery>) -> AppResult<Json<BiblioSearchResponse>> {
    claims.require_read_items()?;
    if !sees_internal_catalog(&claims) {
        query.exclude_fully_withdrawn = Some(true);
    }

    let (biblios, total) = state.services.catalog.search_biblios(&query).await?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    let body = if sees_internal_catalog(&claims) {
        BiblioSearchResponse::Staff(PaginatedResponse::new(super::catalog_hold::staff_biblio_shorts(&state, &claims, biblios).await?, total, page, per_page))
    } else {
        BiblioSearchResponse::Public(PaginatedResponse::new(biblios.into_iter().map(BiblioShortPublic::from).collect(), total, page, per_page))
    };
    Ok(Json(body))
}

/// Get biblio details by ID
#[utoipa::path(
    get,
    path = "/biblios/{id}",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Biblio ID"),
        ("full_record" = Option<bool>, Query, description = "If true, include full MARC record data")
    ),
    responses(
        (status = 200, description = "Bibliographic record details. Callers with internal catalog rights receive `BiblioStaffView` (every copy, with `circulationStatus`, `publicNote`, and `internalNote`). Catalog read alone receives `BiblioPublicView`: lost, declared missing, weeding-in-progress, and withdrawn copies are omitted. Remaining copies use public availability (`available`, `onLoan`, `onSiteOnly`, `unavailable`) and include `publicNote` only. The record includes `holdsCount` (pending holds still waiting for a copy; `to_pick`, `ready`, and in-transit holds are excluded). Staff copies include `dueDate` (active loan `expiry_at`, or null). Public copies omit `dueDate` entirely (the key is absent, not null) and omit `internalNote`. Every remaining copy includes `onHold` (true only when this copy is `to_pick` or `ready`, or in transit for a hold). A public copy with `availability` `onLoan` includes `expectedReturnDate` (`YYYY-MM-DD` in the library timezone, no time) while that local date is today or later. The field is omitted when the local due date is before today, for every other availability, and on the staff view. The comparison uses `library_info.timezone`, not UTC. An unassigned `pending` title hold does not set `onHold`. `holdFor` is set on a staff `onHold` copy when the caller has `users_rights` or `loans_rights` read or write. `users_rights` returns `UserShortStaff`. `loans_rights` without users read returns `{ id, cardNumber, initials }`. Patrons, the OPAC, and callers with neither right omit `holdFor`. Callers with `loans_rights` read or write also receive `borrower` on a copy that is on loan: `{ id, cardNumber, initials }` unless `users_rights` is read or write, which returns `UserShortStaff`.", body = BiblioReadResponse),
        (status = 404, description = "Biblio not found")
    )
)]
pub async fn get_biblio(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
    Query(_query): Query<GetBiblioQuery>,
) -> AppResult<Json<BiblioReadResponse>> {
    claims.require_read_items()?;

    let biblio = state.services.catalog.get_biblio(id).await?;
    if !sees_internal_catalog(&claims) && biblio.is_fully_withdrawn() {
        return Err(crate::error::AppError::NotFound("Bibliographic record not found".into()));
    }
    let body = if sees_internal_catalog(&claims) {
        let mut view = super::catalog_hold::staff_biblio_view(&state, &claims, biblio).await?;
        attach_notice_borrowers(&state, &claims, &mut view.items).await?;
        BiblioReadResponse::Staff(view)
    } else {
        BiblioReadResponse::Public(BiblioPublicView::from(biblio))
    };
    Ok(Json(body))
}

/// Borrower on each on-loan copy for staff who can read loans.
///
/// `users_rights` read or write selects the staff profile. Loans rights alone
/// select `{ id, cardNumber, initials }`. Anyone else keeps `borrower` omitted.
async fn attach_notice_borrowers(state: &crate::AppState, claims: &crate::models::user::UserClaims, items: &mut [ItemStaffView]) -> AppResult<()> {
    if claims.rights.loans_rights.rank() < Rights::Read.rank() {
        return Ok(());
    }
    let ids: Vec<i64> = items.iter().filter_map(|item| item.id).collect();
    let borrowers = state.services.loans.active_borrowers_by_item_ids(&ids).await?;
    for item in items.iter_mut() {
        let Some(id) = item.id else {
            continue;
        };
        let Some(user) = borrowers.get(&id) else {
            continue;
        };
        item.borrower = Some(project_borrower(user.clone(), claims, false));
    }
    Ok(())
}

/// Query params for create biblio
#[serde_as]
#[derive(Debug, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBiblioQuery {
    /// If true, allow creating a biblio even when another has the same ISBN
    #[serde(default)]
    pub allow_duplicate_isbn: bool,
    /// Set to the existing biblio ID to confirm replacement of a duplicate
    pub confirm_replace_existing_id: Option<i64>,
}

/// Response body for biblio creation (biblio + optional dedup report)
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateBiblioResponse {
    pub biblio: Biblio,
    pub import_report: ImportReport,
}

/// Query params for UNIMARC upload
#[derive(Debug, Deserialize)]
pub struct UploadUnimarcQuery {}

/// Query params for MARC batch import
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportMarcBatchQuery {
    /// Site for the imported copies. Optional when `deposit_batch_id` is set.
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    /// Batch identifier returned by upload_unimarc
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    /// Optional record id inside the batch (e.g. "1", "2", ...)
    pub record_id: Option<usize>,
    /// If true, allow creating a biblio even when another has the same ISBN
    #[serde(default)]
    pub allow_duplicate_isbn: bool,
    /// Set to the existing biblio ID to confirm replacement of a duplicate
    pub confirm_replace_existing_id: Option<i64>,
    /// Active incoming deposit batch.
    ///
    /// Official query name is `deposit_batch_id`, same as `batch_id` and
    /// `source_id` on this route. `depositBatchId` is accepted as an alias.
    #[serde(default, rename = "deposit_batch_id", alias = "depositBatchId")]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub deposit_batch_id: Option<i64>,
    /// When set with a deposit batch, how to resolve lender-barcode conflicts detected at import time.
    #[serde(default)]
    pub deposit_duplicate_action: Option<DepositDuplicateAction>,
    /// Apply [`Self::deposit_duplicate_action`] to every conflict in the batch (default: false = first conflict only).
    #[serde(default)]
    pub deposit_duplicate_apply_all: bool,
}

fn deposit_duplicate_policy_from_query(params: &ImportMarcBatchQuery) -> Option<DepositDuplicatePolicy> {
    params.deposit_duplicate_action.map(|action| DepositDuplicatePolicy {
        action,
        apply_to_remaining: params.deposit_duplicate_apply_all,
    })
}

/// Create a new bibliographic record (with ISBN deduplication)
#[utoipa::path(
    post,
    path = "/biblios",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("allow_duplicate_isbn" = Option<bool>, Query, description = "Allow duplicate ISBN (default: false)"),
        ("replace_existing" = Option<bool>, Query, description = "Replace existing biblio")
    ),
    request_body = Biblio,
    responses(
        (status = 201, description = "Biblio created or merged. Nested copies use `publicNote` and `internalNote` (at most 4000 Unicode characters each).", body = CreateBiblioResponse),
        (status = 400, description = "Validation error. Body is `{ code, error, message }` with `code` `validation_error`. A nested item that includes `notes` is rejected and nothing is written.", body = crate::error::ErrorResponse),
        (status = 409, description = "Duplicate ISBN requires confirmation", body = crate::models::import_report::DuplicateConfirmationRequired)
    )
)]
pub async fn create_biblio(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Query(query): Query<CreateBiblioQuery>,
    Json(biblio): Json<Biblio>,
) -> AppResult<(StatusCode, Json<CreateBiblioResponse>)> {
    claims.require_write_items()?;
    match state.services.catalog.create_biblio(biblio, query.allow_duplicate_isbn, query.confirm_replace_existing_id).await {
        Ok((biblio, import_report)) => {
            state.services.audit.log(
                audit::event::BIBLIO_CREATED,
                Some(claims.user_id),
                Some("biblio"),
                biblio.id,
                ip.clone(),
                Some(&biblio),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(CreateBiblioResponse { biblio, import_report })))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::BIBLIO_CREATED,
                Some(claims.user_id),
                Some("biblio"),
                None,
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoadMarcQuery {
    /// Optional; defaults to UTF-8 for binary ISO 2709 uploads.
    encoding: Option<String>,
}

/// Upload a UNIMARC file and return parsed biblios with linked items (995/952).
#[utoipa::path(
    post,
    path = "/biblios/load-marc",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("site_id" = i64, Query, description = "Site ID associated to this MARC batch"),
        ("encoding" = Option<String>, Query, description = "Default ISO 2709 file encoding when not declared in the record: utf8 (default), marc8, iso5426, iso8859-2, iso8859-3, iso8859-5")
    ),
    responses(
        (status = 200, description = "Batch id and per-record previews (BiblioShort + validationIssues from marc-rs)", body = EnqueueResult),
        (status = 400, description = "Missing file or invalid UNIMARC"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn load_marc(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Query(query): Query<LoadMarcQuery>,
    mut multipart: Multipart,
) -> AppResult<Json<EnqueueResult>> {
    claims.require_read_items()?;

    let mut data = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(format!("Multipart error: {}", e)))? {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| AppError::BadRequest(format!("Failed to read field: {}", e)))?;
            data = bytes.to_vec();
            break;
        }
    }
    if data.is_empty() {
        return Err(AppError::BadRequest("Missing 'file' field in multipart form".to_string()));
    }

    let file_encoding = crate::services::marc::resolve_marc_upload_encoding(query.encoding.as_deref())?;
    let enqueue_result = state
        .services
        .marc
        .enqueue_unimarc_batch(&data, file_encoding)
        .await?;

    state.services.audit.log(
        audit::event::IMPORT_MARC_UPLOAD,
        Some(claims.user_id),
        None,
        None,
        ip,
        Some(serde_json::json!({
            "batch_id": enqueue_result.batch_id,
            "record_count": enqueue_result.previews.len(),
            "file_bytes": data.len(),
        })),
        audit::AuditLogMeta::success(),
    );

    Ok(Json(enqueue_result))
}

/// Scan a cached MARC batch for deposit copy barcode conflicts before import.
#[utoipa::path(
    post,
    path = "/biblios/import-marc-batch/scan-deposit-duplicates",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("batch_id" = String, Query, description = "MARC batch identifier"),
        ("deposit_batch_id" = String, Query, description = "Active deposit batch"),
        ("record_id" = Option<usize>, Query, description = "Optional single record index")
    ),
    responses(
        (status = 200, description = "Conflict scan result", body = DepositDuplicateScanReport),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 422, description = "Deposit batch is not active", body = crate::error::ErrorResponse)
    )
)]
pub async fn scan_deposit_marc_duplicates(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(params): Query<ScanDepositDuplicatesQuery>,
) -> AppResult<Json<DepositDuplicateScanReport>> {
    claims.require_write_items()?;
    let report = state
        .services
        .marc
        .scan_deposit_duplicates(params.batch_id, params.deposit_batch_id, params.record_id)
        .await?;
    Ok(Json(report))
}

/// Import cached MARC records from a batch into the catalog.
///
/// Returns `202 Accepted` immediately with a `taskId`.  Poll `GET /tasks/:id`
/// until `status` is `completed` or `failed`.  The `result` field of the
/// completed task contains a `MarcBatchImportReport`.
#[utoipa::path(
    post,
    path = "/biblios/import-marc-batch",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("batch_id" = String, Query, description = "MARC batch identifier returned by load-marc"),
        ("site_id" = Option<String>, Query, description = "Site to attach to imported copies. Required unless deposit_batch_id is set. In deposit mode it must equal the batch site."),
        ("record_id" = Option<usize>, Query, description = "Optional single record index; if omitted, all records in the batch are imported"),
        ("allow_duplicate_isbn" = Option<bool>, Query, description = "Allow creating a biblio even when another has the same ISBN (default: false). Refused with a deposit batch."),
        ("confirm_replace_existing_id" = Option<i64>, Query, description = "Confirm replacement of an existing biblio by its ID. Refused with a deposit batch."),
        ("deposit_batch_id" = Option<String>, Query, description = "Active deposit batch. Official name is deposit_batch_id; depositBatchId is accepted as an alias. Copies attach to the existing record with the same ISBN, or create the record when there is no match. The lender barcode is kept.")
    ),
    responses(
        (status = 202, description = "Import task accepted", body = TaskAcceptedResponse),
        (status = 400, description = "site_id missing outside deposit mode", body = crate::error::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse),
        (status = 422, description = "Deposit batch is not active, the site does not match, or duplicate-ISBN options were sent with the batch", body = crate::error::ErrorResponse)
    )
)]
pub async fn import_marc_batch(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Query(params): Query<ImportMarcBatchQuery>,
) -> AppResult<(StatusCode, Json<TaskAcceptedResponse>)> {
    claims.require_write_items()?;

    let prepared = state
        .services
        .deposits
        .prepare_marc_import(params.deposit_batch_id, params.site_id, params.allow_duplicate_isbn, params.confirm_replace_existing_id)
        .await?;

    let marc = state.services.marc.clone();
    let audit = state.services.audit.clone();
    let p = params.clone();
    let site_id = prepared.site_id;
    let deposit_batch_id = prepared.deposit_batch_id;
    let deposit_duplicate_policy = deposit_duplicate_policy_from_query(&params);

    let task_id = state
        .services
        .tasks
        .spawn_task(TaskKind::MarcBatchImport, claims.user_id, move |handle| async move {
            match marc
                .import_from_batch(
                    p.batch_id,
                    site_id,
                    p.record_id,
                    p.allow_duplicate_isbn,
                    p.confirm_replace_existing_id,
                    deposit_batch_id,
                    deposit_duplicate_policy,
                    Some(handle.clone()),
                )
                .await
            {
                Ok(report) => {
                    audit.log(
                        audit::event::IMPORT_MARC_BATCH,
                        Some(claims.user_id),
                        None,
                        None,
                        ip.clone(),
                        Some(serde_json::json!({
                            "params": &p,
                            "report": &report,
                        })),
                        audit::AuditLogMeta::success(),
                    );
                    audit.log(
                        audit::event::SYSTEM_TASK_COMPLETED,
                        Some(claims.user_id),
                        None,
                        None,
                        None,
                        Some(serde_json::json!({
                            "task_id": handle.id,
                            "task_kind": "marc_batch_import",
                            "batch_id": p.batch_id,
                        })),
                        audit::AuditLogMeta::success(),
                    );
                    let result = serde_json::to_value(&report).unwrap_or_default();
                    handle.complete(result).await;
                }
                Err(e) => {
                    audit.log(
                        audit::event::IMPORT_MARC_BATCH,
                        Some(claims.user_id),
                        None,
                        None,
                        ip.clone(),
                        Some(&p),
                        audit::AuditLogMeta::from_app_error(&e),
                    );
                    audit.log(
                        audit::event::SYSTEM_TASK_FAILED,
                        Some(claims.user_id),
                        None,
                        None,
                        None,
                        Some(serde_json::json!({
                            "task_id": handle.id,
                            "task_kind": "marc_batch_import",
                            "batch_id": p.batch_id,
                            "error": e.to_string(),
                        })),
                        audit::AuditLogMeta::failure_background("task_failed", e.to_string()),
                    );
                    handle.fail(e.to_string()).await;
                }
            }
        })
        .await;

    Ok((StatusCode::ACCEPTED, Json(TaskAcceptedResponse { task_id })))
}

/// Update an existing bibliographic record
#[utoipa::path(
    put,
    path = "/biblios/{id}",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Biblio ID"),
        ("allow_duplicate_isbn" = Option<bool>, Query, description = "Allow duplicate ISBN (default: false)")
    ),
    request_body = Biblio,
    responses(
        (status = 200, description = "Biblio updated", body = Biblio),
        (status = 404, description = "Biblio not found"),
        (status = 409, description = "Duplicate ISBN requires confirmation")
    )
)]
pub async fn update_biblio(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Query(query): Query<UpdateBiblioQuery>,
    Json(biblio): Json<Biblio>,
) -> AppResult<Json<Biblio>> {
    claims.require_write_items()?;
    match state.services.catalog.update_biblio(id, biblio, query.allow_duplicate_isbn).await {
        Ok(updated) => {
            state.services.audit.log(
                audit::event::BIBLIO_UPDATED,
                Some(claims.user_id),
                Some("biblio"),
                Some(id),
                ip.clone(),
                Some((id, &updated)),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(updated))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::BIBLIO_UPDATED,
                Some(claims.user_id),
                Some("biblio"),
                Some(id),
                ip,
                None::<()>,
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBiblioQuery {
    #[serde(default)]
    pub allow_duplicate_isbn: bool,
}

/// Delete a bibliographic record
#[utoipa::path(
    delete,
    path = "/biblios/{id}",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Biblio ID"),
        ("force" = Option<bool>, Query, description = "Force delete even if physical items are borrowed")
    ),
    responses(
        (status = 204, description = "Biblio deleted"),
        (status = 404, description = "Biblio not found"),
        (status = 409, description = "Biblio has borrowed items")
    )
)]
pub async fn delete_biblio(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    Query(params): Query<DeleteBiblioParams>,
) -> AppResult<StatusCode> {
    claims.require_write_items()?;
    let force = params.force.unwrap_or(false);
    match state.services.catalog.delete_biblio(id, force).await {
        Ok(()) => {
            state.services.audit.log(
                audit::event::BIBLIO_DELETED,
                Some(claims.user_id),
                Some("biblio"),
                Some(id),
                ip.clone(),
                Some(serde_json::json!({ "id": id, "force": force })),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::BIBLIO_DELETED,
                Some(claims.user_id),
                Some("biblio"),
                Some(id),
                ip,
                Some(serde_json::json!({ "id": id, "force": force })),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBiblioParams {
    pub force: Option<bool>,
}

/// List physical items for a bibliographic record
#[utoipa::path(
    get,
    path = "/biblios/{id}/items",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Biblio ID")
    ),
    responses(
        (status = 200, description = "List of physical items. Callers with internal catalog rights receive every copy as `ItemStaffView`, including `circulationStatus`, `publicNote`, and `internalNote`. Catalog read alone receives `ItemPublicView` with lost, declared missing, weeding-in-progress, and withdrawn copies omitted, and `publicNote` only. Staff copies include `dueDate` (active loan `expiry_at`, or null). Public copies omit `dueDate` (the key is absent, not null) and omit `internalNote`. Remaining copies include `onHold` and public availability. A public copy with `availability` `onLoan` includes `expectedReturnDate` (`YYYY-MM-DD` in the library timezone, no time) while that local date is today or later. The field is omitted when the local due date is before today, for every other availability, and on the staff view. The comparison uses `library_info.timezone`, not UTC. Users or loans read receive `holdFor` on an `onHold` copy (same rule as `GET /biblios/{id}`). The public view omits `holdFor`. This list does not include `borrower`.", body = ItemListResponse),
        (status = 404, description = "Biblio not found")
    )
)]
pub async fn list_items(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(biblio_id): Path<i64>) -> AppResult<Json<ItemListResponse>> {
    claims.require_read_items()?;

    let items = state.services.catalog.get_items(biblio_id).await?;
    if !sees_internal_catalog(&claims) && !items.is_empty() && items.iter().all(|item| item.weeding_status == crate::models::item::WeedingStatus::Withdrawn) {
        return Err(crate::error::AppError::NotFound("Bibliographic record not found".into()));
    }
    let body = if sees_internal_catalog(&claims) {
        ItemListResponse::Staff(super::catalog_hold::staff_item_views(&state, &claims, items).await?)
    } else {
        ItemListResponse::Public(items.into_iter().filter(crate::models::item::Item::included_in_public_catalog).map(ItemPublicView::from).collect())
    };
    Ok(Json(body))
}

/// Create a new physical item for a bibliographic record
#[utoipa::path(
    post,
    path = "/biblios/{id}/items",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("id" = i64, Path, description = "Biblio ID")
    ),
    request_body = Item,
    responses(
        (status = 201, description = "Physical item created. Body is `Item` with `publicNote` and `internalNote` (at most 4000 Unicode characters each).", body = Item),
        (status = 400, description = "Validation error. Body is `{ code, error, message }` with `code` `validation_error`. `notes` is rejected and nothing is written.", body = crate::error::ErrorResponse),
        (status = 404, description = "Biblio not found", body = crate::error::ErrorResponse),
        (status = 409, description = "An item with this barcode already exists", body = crate::models::import_report::DuplicateItemBarcodeRequired)
    )
)]
pub async fn create_item(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(biblio_id): Path<i64>,
    ValidatedJson(item): ValidatedJson<Item>,
) -> AppResult<(StatusCode, Json<Item>)> {
    claims.require_write_items()?;
    let created = state.services.catalog.create_item(biblio_id, item).await?;

    state.services.audit.log(
        audit::event::ITEM_CREATED,
        Some(claims.user_id),
        Some("item"),
        created.id,
        ip,
        Some((biblio_id, &created)),
        audit::AuditLogMeta::success(),
    );

    Ok((StatusCode::CREATED, Json(created)))
}

/// List all MARC batches currently cached in Redis.
#[utoipa::path(
    get,
    path = "/biblios/list-marc-batches",
    tag = "biblios",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of cached MARC batches", body = Vec<MarcBatchInfo>),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn list_marc_batches(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser) -> AppResult<Json<Vec<MarcBatchInfo>>> {
    claims.require_read_items()?;
    let batches = state.services.marc.list_marc_batches().await?;
    Ok(Json(batches))
}

/// Reload a cached MARC batch from Redis by its batch ID.
#[utoipa::path(
    get,
    path = "/biblios/marc-batch/{batch_id}",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("batch_id" = String, Path, description = "MARC batch identifier (Snowflake ID as string)")
    ),
    responses(
        (status = 200, description = "Same shape as load-marc: batchId + previews (BiblioShort + validationIssues)", body = EnqueueResult),
        (status = 404, description = "Batch not found or expired"),
        (status = 401, description = "Not authenticated")
    )
)]
pub async fn load_marc_batch(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Path(batch_id): Path<i64>) -> AppResult<Json<EnqueueResult>> {
    claims.require_read_items()?;
    let result = state.services.marc.load_marc_batch(batch_id).await?;
    Ok(Json(result))
}

/// Export catalog as CSV
///
/// Returns a UTF-8 CSV file with all bibliographic records matching the query.
/// Streams all pages — does not paginate. Use the same query params as `GET /biblios`.
#[utoipa::path(
    get,
    path = "/biblios/export.csv",
    tag = "biblios",
    security(("bearer_auth" = [])),
    params(
        ("title" = Option<String>, Query, description = "Filter by title"),
        ("author" = Option<String>, Query, description = "Filter by author"),
        ("media_type" = Option<String>, Query, description = "Filter by media type")
    ),
    responses(
        (status = 200, description = "CSV file", content_type = "text/csv"),
        (status = 401, description = "Not authenticated", body = crate::error::ErrorResponse)
    )
)]
pub async fn export_biblios_csv(State(state): State<crate::AppState>, AuthenticatedUser(claims): AuthenticatedUser, Query(mut query): Query<BiblioQuery>) -> AppResult<axum::response::Response> {
    claims.require_read_catalog()?;

    query.page = Some(1);
    query.per_page = Some(10_000);

    let (biblios, _) = state.services.catalog.search_biblios(&query).await?;

    let mut csv = String::from("id,isbn,title,author,media_type,date,items\n");
    for biblio in &biblios {
        let author_name = biblio
            .author
            .as_ref()
            .map(|a| format!("{} {}", a.firstname.as_deref().unwrap_or(""), a.lastname.as_deref().unwrap_or("")).trim().to_string())
            .unwrap_or_default();
        csv.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            biblio.id,
            escape_csv(biblio.isbn.as_ref().map(|i| i.as_str()).unwrap_or("")),
            escape_csv(biblio.title.as_deref().unwrap_or("")),
            escape_csv(&author_name),
            escape_csv(biblio.media_type.as_db_str()),
            escape_csv(biblio.date.as_deref().unwrap_or("")),
            biblio.items.len(),
        ));
    }

    use axum::http::header;
    Ok((
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=\"catalog.csv\"")],
        csv,
    )
        .into_response())
}

fn escape_csv(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
