//! Lenders and incoming deposit batches, including MARC import in deposit mode.

use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::{
    error::{error_code, AppError, AppResult},
    models::{
        biblio::Biblio,
        deposit::{
            CreateDepositBatch, CreateLender, DepositBatch, DepositBatchDetail, DepositBatchItem, DepositBatchStatus, DepositCopyState, DepositLoss, Lender, NotifiedOn, UpdateDepositBatch,
            UpdateLender,
        },
        dto::deposit_import::{
            DepositDuplicateAction, DepositDuplicateConflict, DepositDuplicateConflictKind, DepositDuplicatePolicyRuntime, DepositDuplicateScanReport,
        },
    },
    repository::Repository,
    services::{
        audit::{self, AuditLogMeta, AuditService},
        catalog::CatalogService,
    },
};
use validator::Validate;

#[derive(Clone)]
pub struct DepositsService {
    repository: Repository,
    catalog: CatalogService,
    audit: AuditService,
}

/// One cached MARC record imported into a deposit batch.
#[derive(Debug, Default)]
pub struct DepositMarcRecordOutcome {
    /// Set when at least one copy was stored, or the record had no copies.
    pub biblio_id: Option<i64>,
    /// `true` when this import created the bibliographic record.
    pub created: bool,
    pub copies: DepositImportCopyReport,
}

#[derive(Debug, Default)]
pub struct DepositImportCopyReport {
    pub created: Vec<DepositImportCopyLine>,
    pub reactivated: Vec<DepositImportCopyLine>,
    pub relabeled: Vec<DepositImportRelabeledLine>,
    pub overwritten: Vec<DepositImportCopyLine>,
    pub refused: Vec<DepositImportRefusedLine>,
}

#[derive(Debug, Clone)]
pub struct DepositImportCopyLine {
    pub item_id: i64,
    pub lender_barcode: String,
    pub barcode: String,
}

#[derive(Debug, Clone)]
pub struct DepositImportRelabeledLine {
    pub item_id: i64,
    pub lender_barcode: String,
    pub local_barcode: String,
}

#[derive(Debug, Clone)]
pub struct DepositImportRefusedLine {
    pub lender_barcode: String,
    pub code: &'static str,
    pub item_id: Option<i64>,
}

/// Resolved `POST /biblios/import-marc-batch` target.
#[derive(Debug, Clone)]
pub struct PreparedMarcImport {
    pub site_id: i64,
    pub deposit_batch_id: Option<i64>,
}

impl DepositsService {
    pub fn new(repository: Repository, catalog: CatalogService, audit: AuditService) -> Self {
        Self { repository, catalog, audit }
    }

    pub async fn list_lenders(&self, include_archived: bool) -> AppResult<Vec<Lender>> {
        self.repository.lenders_list(include_archived).await
    }

    pub async fn create_lender(&self, data: &CreateLender) -> AppResult<Lender> {
        data.validate().map_err(|err| AppError::Validation(err.to_string()))?;
        if data.name.trim().is_empty() {
            return Err(AppError::Validation("Lender name is required".into()));
        }
        reject_non_positive_rate(data.replacement_rate_cents)?;
        self.repository.lenders_create(data).await
    }

    pub async fn update_lender(&self, id: i64, data: &UpdateLender) -> AppResult<Lender> {
        data.validate().map_err(|err| AppError::Validation(err.to_string()))?;
        if data.name.as_ref().is_some_and(|name| name.trim().is_empty()) {
            return Err(AppError::Validation("Lender name is required".into()));
        }
        reject_non_positive_rate(data.replacement_rate_cents.flatten())?;
        self.repository.lenders_update(id, data).await
    }

    pub async fn list_deposit_losses(&self, lender_id: Option<i64>, notified: bool) -> AppResult<Vec<DepositLoss>> {
        if let Some(lender_id) = lender_id {
            self.repository.lenders_get(lender_id).await?;
        }
        self.repository.deposit_losses_list(lender_id, notified).await
    }

    /// Set `items.deposit_loss_notified_at` and write `deposit_loss_notified`
    /// in the same transaction.
    ///
    /// A copy that is already notified returns `deposit_loss_already_notified`
    /// (409). The stored timestamp is left unchanged and no second audit row
    /// is written.
    pub async fn mark_deposit_loss_notified(&self, item_id: i64, notified_on: Option<NotifiedOn>, actor: Option<i64>, client_ip: Option<String>) -> AppResult<DepositLoss> {
        let zone = self.repository.library_zone().await?;
        let notified_at = resolve_notified_at(zone, notified_on, Utc::now())?;
        let mut tx = self.repository.pool().begin().await?;
        let lender_id = self.repository.deposit_loss_lock_for_notify(&mut tx, item_id).await?;
        self.repository.deposit_loss_set_notified(&mut tx, item_id, notified_at).await?;
        self.audit
            .log_in_tx(
                &mut tx,
                audit::event::DEPOSIT_LOSS_NOTIFIED,
                actor,
                Some("item"),
                Some(item_id),
                client_ip,
                Some(serde_json::json!({
                    "lenderId": lender_id.to_string(),
                    "itemId": item_id.to_string(),
                })),
                AuditLogMeta::success(),
            )
            .await?;
        let loss = self.repository.deposit_loss_by_item_tx(&mut tx, item_id).await?;
        tx.commit().await?;
        Ok(loss)
    }

    pub async fn archive_lender(&self, id: i64) -> AppResult<Lender> {
        self.repository.lenders_archive(id).await
    }

    pub async fn list_batches(&self, lender_id: Option<i64>, status: Option<DepositBatchStatus>) -> AppResult<Vec<DepositBatch>> {
        if let Some(lender_id) = lender_id {
            self.repository.lenders_get(lender_id).await?;
        }
        self.repository.deposit_batches_list(lender_id, status).await
    }

    pub async fn get_batch(&self, id: i64) -> AppResult<DepositBatchDetail> {
        let batch = self.repository.deposit_batches_get(id).await?;
        let counts = self.repository.deposit_batch_counts(id).await?;
        Ok(DepositBatchDetail { batch, counts })
    }

    pub async fn create_batch(&self, data: &CreateDepositBatch) -> AppResult<DepositBatch> {
        data.validate().map_err(|err| AppError::Validation(err.to_string()))?;
        if data.reference.trim().is_empty() {
            return Err(AppError::Validation("Deposit batch reference is required".into()));
        }
        if let Some(return_due_on) = data.return_due_on {
            if return_due_on < data.arrived_on {
                return Err(AppError::Validation("returnDueOn must be on or after arrivedOn".into()));
            }
        }
        self.repository.lenders_get(data.lender_id).await?;
        self.require_site(data.site_id).await?;
        self.repository.deposit_batches_create(data).await
    }

    pub async fn update_batch(&self, id: i64, data: &UpdateDepositBatch) -> AppResult<DepositBatch> {
        data.validate().map_err(|err| AppError::Validation(err.to_string()))?;
        if let Some(site_id) = data.site_id {
            self.require_site(site_id).await?;
        }
        self.repository.deposit_batches_update(id, data).await
    }

    pub async fn start_return(&self, id: i64) -> AppResult<DepositBatch> {
        self.repository.deposit_batches_start_return(id).await
    }

    pub async fn cancel_return(&self, id: i64) -> AppResult<DepositBatch> {
        self.repository.deposit_batches_cancel_return(id).await
    }

    pub async fn scan_out(&self, id: i64, barcode: &str) -> AppResult<crate::models::deposit::DepositScanOut> {
        self.repository.deposit_batches_scan_out(id, barcode).await
    }

    pub async fn scan_out_by_barcode(&self, barcode: &str) -> AppResult<crate::models::deposit::DepositGlobalScanOut> {
        self.repository.deposit_batches_scan_out_by_barcode(barcode).await
    }

    pub async fn close_batch(&self, id: i64) -> AppResult<crate::models::deposit::DepositCloseReport> {
        self.repository.deposit_batches_close(id).await
    }

    pub async fn list_batch_items(&self, id: i64, state: Option<&str>) -> AppResult<Vec<DepositBatchItem>> {
        let state = match state.map(str::trim).filter(|value| !value.is_empty()) {
            None => None,
            Some(value) => Some(DepositCopyState::from_filter(value).ok_or_else(|| AppError::Validation("state must be onLoan, onHold, inTransit, onShelf, returned, or missing".into()))?),
        };
        self.repository.deposit_batch_items(id, state).await
    }

    /// Check import options before the background task starts.
    pub async fn prepare_marc_import(
        &self,
        deposit_batch_id: Option<i64>,
        site_id: Option<i64>,
        allow_duplicate_isbn: bool,
        confirm_replace_existing_id: Option<i64>,
    ) -> AppResult<PreparedMarcImport> {
        let Some(deposit_batch_id) = deposit_batch_id else {
            let site_id = site_id.ok_or_else(|| AppError::Validation("site_id is required".into()))?;
            return Ok(PreparedMarcImport { site_id, deposit_batch_id: None });
        };
        if allow_duplicate_isbn || confirm_replace_existing_id.is_some() {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_IMPORT_OPTIONS_CONFLICT,
                "allow_duplicate_isbn and confirm_replace_existing_id cannot be used with a deposit batch",
            ));
        }
        let batch = self.require_active_batch(deposit_batch_id).await?;
        if let Some(site_id) = site_id {
            if site_id != batch.site_id {
                return Err(AppError::coded(422, error_code::DEPOSIT_SITE_MISMATCH, "site_id must be the deposit batch site"));
            }
        }
        Ok(PreparedMarcImport {
            site_id: batch.site_id,
            deposit_batch_id: Some(batch.id),
        })
    }

    pub async fn require_active_batch(&self, id: i64) -> AppResult<DepositBatch> {
        let batch = self.repository.deposit_batches_get(id).await?;
        if batch.status != DepositBatchStatus::Active {
            return Err(AppError::coded(422, error_code::DEPOSIT_BATCH_NOT_ACTIVE, "Deposit copies can only be imported into an active batch"));
        }
        Ok(batch)
    }

    /// Scan cached MARC copies for lender-barcode conflicts before import.
    pub async fn scan_marc_duplicates(
        &self,
        batch_id: i64,
        batch: &DepositBatch,
        records: &[(String, Biblio)],
    ) -> AppResult<DepositDuplicateScanReport> {
        let mut conflicts = Vec::new();
        for (record_key, biblio) in records {
            conflicts.extend(self.scan_biblio_duplicates(record_key, biblio, batch).await?);
        }
        Ok(DepositDuplicateScanReport { batch_id, conflicts })
    }

    async fn scan_biblio_duplicates(&self, record_key: &str, biblio: &Biblio, batch: &DepositBatch) -> AppResult<Vec<DepositDuplicateConflict>> {
        let mut out = Vec::new();
        for item in &biblio.items {
            let Some(lender_barcode) = item.barcode.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                continue;
            };
            if let Some(info) = self.detect_lender_barcode_duplicate(batch, lender_barcode).await? {
                out.push(DepositDuplicateConflict {
                    record_key: record_key.to_string(),
                    lender_barcode: lender_barcode.to_string(),
                    kind: info.kind,
                    existing_item_id: info.existing_item_id,
                    existing_barcode: info.existing_barcode,
                });
            }
        }
        Ok(out)
    }

    async fn detect_lender_barcode_duplicate(&self, batch: &DepositBatch, lender_barcode: &str) -> AppResult<Option<DuplicateInfo>> {
        if let Some(archived) = self.repository.deposit_archived_copy_for_lender_barcode(batch.lender_id, lender_barcode).await? {
            if archived.lost_billed {
                return Ok(Some(DuplicateInfo {
                    kind: DepositDuplicateConflictKind::WasLost,
                    existing_item_id: archived.item_id,
                    existing_barcode: None,
                }));
            }
            if archived.deposit_returned_at.is_some() {
                return Ok(Some(DuplicateInfo {
                    kind: DepositDuplicateConflictKind::Reactivation,
                    existing_item_id: archived.item_id,
                    existing_barcode: None,
                }));
            }
        }
        if let Some(item_id) = self.repository.items_get_active_id_by_barcode(lender_barcode).await? {
            return Ok(Some(DuplicateInfo {
                kind: DepositDuplicateConflictKind::ActiveBarcode,
                existing_item_id: item_id,
                existing_barcode: Some(lender_barcode.to_string()),
            }));
        }
        Ok(None)
    }

    fn resolve_duplicate_action(policy: &mut Option<DepositDuplicatePolicyRuntime>) -> DepositDuplicateAction {
        match policy {
            Some(runtime) => runtime.resolve(),
            None => DepositDuplicateAction::Relabel,
        }
    }

    /// Attach copies to the active record with the same ISBN, or create the record.
    ///
    /// The bibliographic record is not modified when the ISBN already exists.
    pub async fn import_marc_record(
        &self,
        mut biblio: Biblio,
        batch: &DepositBatch,
        policy: &mut Option<DepositDuplicatePolicyRuntime>,
    ) -> AppResult<DepositMarcRecordOutcome> {
        use super::item_barcode::allocate_local_item_barcode;

        let mut report = DepositImportCopyReport::default();
        let mut pending = Vec::new();
        for mut item in std::mem::take(&mut biblio.items) {
            item.deposit_batch_id = Some(batch.id);
            item.site_id = Some(batch.site_id);
            item.site_name = None;
            item.id = None;
            item.biblio_id = None;
            let lender_barcode = item.barcode.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_string);
            let Some(lender_barcode) = lender_barcode else {
                pending.push(DepositImportPending {
                    item,
                    lender_barcode: String::new(),
                    action: PendingCopyAction::CreateNew { relabel_local: None },
                });
                continue;
            };
            item.lender_barcode = Some(lender_barcode.clone());

            let duplicate = self.detect_lender_barcode_duplicate(batch, &lender_barcode).await?;
            if let Some(ref info) = duplicate {
                let action = Self::resolve_duplicate_action(policy);
                match (info.kind, action) {
                    (_, DepositDuplicateAction::Ignore) => continue,
                    (DepositDuplicateConflictKind::WasLost, _) => {
                        report.refused.push(DepositImportRefusedLine {
                            lender_barcode: lender_barcode.clone(),
                            code: error_code::DEPOSIT_ITEM_WAS_LOST,
                            item_id: Some(info.existing_item_id),
                        });
                        continue;
                    }
                    (DepositDuplicateConflictKind::Reactivation, DepositDuplicateAction::Overwrite) => {
                        let desk = lender_barcode.clone();
                        item.barcode = Some(desk.clone());
                        pending.push(DepositImportPending {
                            item,
                            lender_barcode: desk.clone(),
                            action: PendingCopyAction::Reactivate {
                                item_id: info.existing_item_id,
                                desk_barcode: desk,
                            },
                        });
                        continue;
                    }
                    (DepositDuplicateConflictKind::Reactivation, DepositDuplicateAction::Relabel) => {
                        let desk = if self.repository.items_barcode_exists_active(&lender_barcode, Some(info.existing_item_id)).await? {
                            allocate_local_item_barcode(&self.repository).await?
                        } else {
                            lender_barcode.clone()
                        };
                        item.barcode = Some(desk.clone());
                        pending.push(DepositImportPending {
                            item,
                            lender_barcode,
                            action: PendingCopyAction::Reactivate {
                                item_id: info.existing_item_id,
                                desk_barcode: desk,
                            },
                        });
                        continue;
                    }
                    (DepositDuplicateConflictKind::ActiveBarcode, DepositDuplicateAction::Overwrite) => {
                        pending.push(DepositImportPending {
                            item,
                            lender_barcode,
                            action: PendingCopyAction::OverwriteActive {
                                item_id: info.existing_item_id,
                            },
                        });
                        continue;
                    }
                    (DepositDuplicateConflictKind::ActiveBarcode, DepositDuplicateAction::Relabel) => {
                        let local = allocate_local_item_barcode(&self.repository).await?;
                        item.barcode = Some(local.clone());
                        pending.push(DepositImportPending {
                            item,
                            lender_barcode,
                            action: PendingCopyAction::CreateNew { relabel_local: Some(local) },
                        });
                        continue;
                    }
                }
            }

            if self.repository.items_barcode_exists_active(&lender_barcode, None).await? {
                let local = allocate_local_item_barcode(&self.repository).await?;
                item.barcode = Some(local.clone());
                pending.push(DepositImportPending {
                    item,
                    lender_barcode: lender_barcode.clone(),
                    action: PendingCopyAction::CreateNew { relabel_local: Some(local) },
                });
            } else {
                item.barcode = Some(lender_barcode.clone());
                pending.push(DepositImportPending {
                    item,
                    lender_barcode,
                    action: PendingCopyAction::CreateNew { relabel_local: None },
                });
            }
        }

        if pending.is_empty() && report_has_entries(&report) {
            return Ok(DepositMarcRecordOutcome {
                biblio_id: None,
                created: false,
                copies: report,
            });
        }

        let existing_id = match biblio.isbn.as_ref() {
            Some(isbn) if !isbn.as_str().is_empty() => self.repository.biblios_find_active_by_isbn(isbn.as_str(), None).await?,
            _ => None,
        };

        let (biblio_id, created) = if let Some(id) = existing_id {
            (id, false)
        } else {
            self.repository.biblios_create(&mut biblio).await?;
            let id = biblio.id.ok_or_else(|| AppError::Internal("Created bibliographic record is missing an id".into()))?;
            (id, true)
        };

        let mut stored = Vec::new();
        for row in pending {
            match row.action {
                PendingCopyAction::OverwriteActive { item_id } => {
                    match self
                        .repository
                        .items_deposit_overwrite_from_import(item_id, biblio_id, batch.id, batch.site_id, &row.lender_barcode, &row.item)
                        .await
                    {
                        Ok(item) => {
                            let barcode = item.barcode.clone().unwrap_or_default();
                            report.overwritten.push(DepositImportCopyLine {
                                item_id: item.id.unwrap_or(item_id),
                                lender_barcode: row.lender_barcode,
                                barcode,
                            });
                            stored.push(item);
                        }
                        Err(error) => return Err(error),
                    }
                    continue;
                }
                PendingCopyAction::Reactivate { item_id, desk_barcode } => {
                    match self
                        .repository
                        .items_reactivate_deposit_copy(item_id, biblio_id, batch.id, batch.site_id, &desk_barcode, &row.lender_barcode, &row.item)
                        .await
                    {
                        Ok(item) => {
                            let resolved_id = item.id.unwrap_or(item_id);
                            self.audit.log(
                                audit::event::DEPOSIT_ITEM_REACTIVATED,
                                None,
                                Some("item"),
                                Some(resolved_id),
                                None,
                                Some(serde_json::json!({
                                    "batchId": batch.id.to_string(),
                                    "lenderId": batch.lender_id.to_string(),
                                    "lenderBarcode": row.lender_barcode,
                                })),
                                AuditLogMeta::success(),
                            );
                            if desk_barcode != row.lender_barcode {
                                report.relabeled.push(DepositImportRelabeledLine {
                                    item_id: resolved_id,
                                    lender_barcode: row.lender_barcode.clone(),
                                    local_barcode: desk_barcode.clone(),
                                });
                            } else {
                                report.reactivated.push(DepositImportCopyLine {
                                    item_id: resolved_id,
                                    lender_barcode: row.lender_barcode,
                                    barcode: desk_barcode,
                                });
                            }
                            stored.push(item);
                        }
                        Err(AppError::Coded { code, .. }) if code == error_code::ITEM_REACTIVATION_BARCODE_TAKEN => {
                            report.refused.push(DepositImportRefusedLine {
                                lender_barcode: row.lender_barcode,
                                code: error_code::ITEM_REACTIVATION_BARCODE_TAKEN,
                                item_id: Some(item_id),
                            });
                        }
                        Err(error) => return Err(error),
                    }
                    continue;
                }
                PendingCopyAction::CreateNew { relabel_local } => {
                    let mut item = row.item;
                    item.biblio_id = Some(biblio_id);
                    if item.barcode.is_none() {
                        item.barcode = item.lender_barcode.clone();
                    }
                    match self.repository.biblios_create_item(biblio_id, &item).await {
                        Ok(created_item) => {
                            let item_id = created_item.id.unwrap_or(0);
                            if let Some(local) = relabel_local {
                                report.relabeled.push(DepositImportRelabeledLine {
                                    item_id,
                                    lender_barcode: row.lender_barcode,
                                    local_barcode: local,
                                });
                            } else {
                                report.created.push(DepositImportCopyLine {
                                    item_id,
                                    lender_barcode: row.lender_barcode,
                                    barcode: created_item.barcode.clone().unwrap_or_default(),
                                });
                            }
                            stored.push(created_item);
                        }
                        Err(error) if is_unique_violation(&error) => {
                            return Err(AppError::Internal("Unexpected barcode collision while importing a deposit copy".into()));
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }

        if created && stored.is_empty() {
            self.repository.biblios_delete(biblio_id, false).await?;
            self.catalog.sync_index(biblio_id).await;
            return Ok(DepositMarcRecordOutcome {
                biblio_id: None,
                created: false,
                copies: report,
            });
        }

        if created {
            biblio.id = Some(biblio_id);
            biblio.items = stored;
            self.repository.biblios_update_marc_record(&mut biblio).await?;
        }
        self.catalog.sync_index(biblio_id).await;

        Ok(DepositMarcRecordOutcome {
            biblio_id: Some(biblio_id),
            created,
            copies: report,
        })
    }

    async fn require_site(&self, site_id: i64) -> AppResult<()> {
        if self.repository.sources_exists(site_id).await? {
            Ok(())
        } else {
            Err(AppError::NotFound(format!("Source {site_id} not found")))
        }
    }
}

fn resolve_notified_at(zone: Tz, value: Option<NotifiedOn>, now: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
    match value {
        None => Ok(now),
        Some(NotifiedOn::Timestamp(timestamp)) => Ok(timestamp),
        Some(NotifiedOn::Date(date)) => {
            let naive = date.and_hms_opt(0, 0, 0).ok_or_else(|| AppError::Validation("notifiedOn is not a valid date".into()))?;
            zone.from_local_datetime(&naive)
                .earliest()
                .map(|local| local.with_timezone(&Utc))
                .ok_or_else(|| AppError::Validation("notifiedOn is not a valid local date".into()))
        }
    }
}

fn reject_non_positive_rate(rate: Option<crate::models::fine::Cents>) -> AppResult<()> {
    if rate.is_some_and(|cents| cents.0 <= 0) {
        return Err(AppError::Validation("replacementRateCents must be greater than 0".into()));
    }
    Ok(())
}

struct DuplicateInfo {
    kind: DepositDuplicateConflictKind,
    existing_item_id: i64,
    existing_barcode: Option<String>,
}

enum PendingCopyAction {
    CreateNew { relabel_local: Option<String> },
    Reactivate { item_id: i64, desk_barcode: String },
    OverwriteActive { item_id: i64 },
}

struct DepositImportPending {
    item: crate::models::item::Item,
    lender_barcode: String,
    action: PendingCopyAction,
}

fn report_has_entries(report: &DepositImportCopyReport) -> bool {
    !report.created.is_empty()
        || !report.reactivated.is_empty()
        || !report.relabeled.is_empty()
        || !report.overwritten.is_empty()
        || !report.refused.is_empty()
}

fn is_unique_violation(error: &AppError) -> bool {
    match error {
        AppError::Database(inner) => match inner.as_ref() {
            sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
            _ => false,
        },
        _ => false,
    }
}
