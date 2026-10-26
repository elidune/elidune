//! MARC batch import service.
//!
//! This service allows reading raw UNIMARC data, caching individual MARC
//! records in Redis under marc:record keys, and importing
//! them into the local catalog.

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;
use z3950_rs::marc_rs::{encoding::Encoding as MarcFileEncoding, MarcReader};

use crate::{
    error::{AppError, AppResult},
    marc::{MarcImportPreview, MarcRecord},
    models::{
        biblio::Biblio,
        dto::deposit_import::{
            DepositDuplicatePolicy, DepositDuplicatePolicyRuntime, DepositDuplicateScanReport,
        },
    },
};

use super::{
    catalog::CatalogService, deposits::DepositsService, redis::RedisService,
    task_manager::TaskHandle,
};

/// Default ISO 2709 decoding when the client omits `encoding` on `POST /biblios/load-marc`.
pub fn default_marc_upload_encoding() -> MarcFileEncoding {
    MarcFileEncoding::Utf8
}

/// Map `load-marc` query values to marc-rs encodings.
pub fn marc_upload_encoding_from_param(value: &str) -> AppResult<MarcFileEncoding> {
    match value.trim().to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Ok(MarcFileEncoding::Utf8),
        "marc8" | "marc-8" => Ok(MarcFileEncoding::Marc8),
        "iso5426" | "iso-5426" => Ok(MarcFileEncoding::Iso5426),
        "iso8859-2" | "iso-8859-2" => Ok(MarcFileEncoding::Other(encoding_rs::ISO_8859_2)),
        "iso8859-3" | "iso-8859-3" => Ok(MarcFileEncoding::Other(encoding_rs::ISO_8859_3)),
        "iso8859-5" | "iso-8859-5" => Ok(MarcFileEncoding::Other(encoding_rs::ISO_8859_5)),
        other => Err(AppError::Validation(format!("encoding must be utf8, marc8, iso5426, iso8859-2, iso8859-3, or iso8859-5 (got {other})"))),
    }
}

pub fn resolve_marc_upload_encoding(param: Option<&str>) -> AppResult<MarcFileEncoding> {
    match param.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(default_marc_upload_encoding()),
        Some(value) => marc_upload_encoding_from_param(value),
    }
}

/// Internal value stored in Redis for a MARC record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMarcRecord {
    /// Site identifier chosen by caller (e.g. Z39.50 server, file source).
    pub site_id: i64,
    /// Raw MARC record as parsed by marc-rs.
    pub record: MarcRecord,
}

/// Error information for a single record during batch import.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcBatchImportError {
    pub key: String,
    /// Human-readable error message.
    pub error: String,
    /// ID of the existing biblio when the failure is a duplicate ISBN conflict.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_id: Option<i64>,
    /// Stable code. `barcode_already_exists` skips that copy and leaves the rest of the batch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Copy that already uses this barcode. Set with [`Self::code`] `barcode_already_exists`.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub existing_item_id: Option<i64>,
}

/// A deposit-mode record attached to an existing bibliographic record.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcBatchAttached {
    pub key: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
}

/// Report returned after importing one or more records from a batch.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcBatchImportReport {
    /// Batch identifier.
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    /// Successfully imported records. Union of [`Self::created`] and [`Self::attached`].
    pub imported: Vec<String>,
    /// Records whose bibliographic notice was created. Empty outside deposit mode.
    #[serde(default)]
    pub created: Vec<String>,
    /// Records whose copies were attached to an existing ISBN. Empty outside deposit mode.
    #[serde(default)]
    pub attached: Vec<MarcBatchAttached>,
    /// Detailed list of records that failed to import.
    pub failed: Vec<MarcBatchImportError>,
    /// Deposit mode: copies created with the lender barcode as desk barcode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies_created: Vec<MarcDepositCopyLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies_reactivated: Vec<MarcDepositCopyLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies_relabeled: Vec<MarcDepositRelabeledLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies_refused: Vec<MarcDepositRefusedLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies_overwritten: Vec<MarcDepositCopyLine>,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcDepositCopyLine {
    pub record_key: String,
    pub lender_barcode: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: String,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcDepositRelabeledLine {
    pub record_key: String,
    pub lender_barcode: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub local_barcode: String,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcDepositRefusedLine {
    pub record_key: String,
    pub lender_barcode: String,
    pub code: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
}

/// Summary of a MARC batch cached in Redis.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarcBatchInfo {
    /// Unique batch identifier (Snowflake ID).
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    /// Number of records in this batch.
    pub record_count: usize,
    /// Remaining TTL in seconds (-1 if no expiry, -2 if key gone).
    pub ttl_seconds: i64,
}

/// Service handling MARC batch parsing, caching and import.
#[derive(Clone)]
pub struct MarcService {
    catalog: CatalogService,
    deposits: DepositsService,
    redis: RedisService,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueResult {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    pub previews: Vec<MarcImportPreview>,
}

impl MarcService {
    pub fn new(catalog: CatalogService, redis: RedisService, deposits: DepositsService) -> Self {
        Self {
            catalog,
            deposits,
            redis,
        }
    }

    const BATCH_TTL_SECS: usize = 60 * 60 * 24;
    const BATCHES_SET_KEY: &'static str = "marc:batches";

    fn redis_key(batch_id: i64, record_id: usize) -> String {
        format!("marc:record:{}:{}", batch_id, record_id)
    }

    fn batch_index_key(batch_id: i64) -> String {
        format!("marc:batch:{}:index", batch_id)
    }

    fn item_key_from_record_key(record_key: &str) -> String {
        record_key.rsplit(':').next().unwrap().to_string()
    }

    /// Read UNIMARC binary data, cache each record in Redis and return a
    /// preview as `ItemShort`.
    ///
    /// - `data`: raw UNIMARC bytes (ISO 2709, UTF-8).
    /// - `source_id`: identifier of the logical source to attach to this batch.
    ///
    /// Returns the generated `batch_id` and the list of preview items.
    #[tracing::instrument(skip(self), err)]
    pub async fn enqueue_unimarc_batch(
        &self,
        data: &[u8],
        file_encoding: MarcFileEncoding,
    ) -> AppResult<EnqueueResult> {
        let records = MarcReader::from_bytes(data.to_vec())
            .map_err(|e| AppError::Validation(format!("UNIMARC parse error: {}", e)))?
            .with_encoding(file_encoding)
            .into_records()
            .map_err(|e| AppError::Validation(format!("UNIMARC parse error: {}", e)))?;

        let batch_id: i64 = snowflaked::Generator::new(1).generate::<i64>();

        let mut conn = self.redis.get_connection().await?;

        let mut previews = Vec::with_capacity(records.len());
        let mut index = 0;

        for record in records.into_iter() {
            for (idx, _) in record.local.items.iter().enumerate() {
                let mut record = record.clone();

                record.local.items.swap(0, idx);
                record.local.items.truncate(1);

                let json_str: String = serde_json::to_string(&record).map_err(|e| {
                    AppError::Internal(format!("Failed to serialize MARC record: {}", e))
                })?;

                let record_key = Self::redis_key(batch_id, index);

                // Store record
                redis::cmd("SETEX")
                    .arg(&record_key)
                    .arg(Self::BATCH_TTL_SECS)
                    .arg(&json_str)
                    .query_async::<_, ()>(&mut conn)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Failed to store MARC record in Redis: {}", e))
                    })?;

                let index_key = Self::batch_index_key(batch_id);
                redis::cmd("SADD")
                    .arg(&index_key)
                    .arg(&record_key)
                    .query_async::<_, ()>(&mut conn)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Failed to index MARC record in Redis: {}", e))
                    })?;

                if index == 0 {
                    redis::cmd("SADD")
                        .arg(Self::BATCHES_SET_KEY)
                        .arg(batch_id.to_string())
                        .query_async::<_, ()>(&mut conn)
                        .await
                        .map_err(|e| {
                            AppError::Internal(format!(
                                "Failed to register MARC batch in Redis: {}",
                                e
                            ))
                        })?;
                }

                redis::cmd("EXPIRE")
                    .arg(&index_key)
                    .arg(Self::BATCH_TTL_SECS)
                    .query_async::<_, ()>(&mut conn)
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!(
                            "Failed to set MARC batch index TTL in Redis: {}",
                            e
                        ))
                    })?;

                let mut preview = MarcImportPreview::from(record);
                preview.biblio.id = index as i64;
                previews.push(preview);

                index += 1;
            }
        }

        Ok(EnqueueResult { batch_id, previews })
    }

    /// List all MARC batches currently cached in Redis.
    ///
    /// Reads batch IDs from `marc:batches` and record counts from each
    /// `marc:batch:{id}:index` set.
    #[tracing::instrument(skip(self), err)]
    pub async fn list_marc_batches(&self) -> AppResult<Vec<MarcBatchInfo>> {
        let mut conn = self.redis.get_connection().await?;

        let batch_ids: Vec<String> = conn.smembers(Self::BATCHES_SET_KEY).await.map_err(|e| {
            AppError::Internal(format!("Failed to list MARC batches in Redis: {}", e))
        })?;

        let mut batches = Vec::with_capacity(batch_ids.len());
        for batch_id_str in batch_ids {
            let Ok(batch_id) = batch_id_str.parse::<i64>() else {
                continue;
            };

            let index_key = Self::batch_index_key(batch_id);
            let record_keys: Vec<String> = conn.smembers(&index_key).await.map_err(|e| {
                AppError::Internal(format!("Failed to list MARC batch index in Redis: {}", e))
            })?;

            if record_keys.is_empty() {
                continue;
            }

            let ttl_seconds: i64 = redis::cmd("TTL")
                .arg(&index_key)
                .query_async::<_, i64>(&mut conn)
                .await
                .unwrap_or(-2);

            batches.push(MarcBatchInfo {
                batch_id,
                record_count: record_keys.len(),
                ttl_seconds,
            });
        }

        // Sort newest first (higher Snowflake ID = newer).
        batches.sort_by(|a, b| b.batch_id.cmp(&a.batch_id));

        Ok(batches)
    }

    /// Reload a cached MARC batch from Redis and return it as an [`EnqueueResult`].
    ///
    /// Useful to re-display a batch that was uploaded earlier without re-uploading
    /// the file.
    #[tracing::instrument(skip(self), err)]
    pub async fn load_marc_batch(&self, batch_id: i64) -> AppResult<EnqueueResult> {
        let mut conn = self.redis.get_connection().await?;

        let mut keys: Vec<String> = conn
            .smembers(Self::batch_index_key(batch_id))
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to list MARC batch keys in Redis: {}", e))
            })?;

        if keys.is_empty() {
            return Err(AppError::NotFound(format!(
                "MARC batch {} not found in cache",
                batch_id
            )));
        }

        // Sort by record index so the preview order matches the original upload.
        keys.sort_by_key(|k| {
            k.rsplit(':')
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        });

        let mut previews = Vec::with_capacity(keys.len());

        for key in &keys {
            let record_idx: usize = key
                .rsplit(':')
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let json_str: Option<String> = conn.get(key).await.map_err(|e| {
                AppError::Internal(format!("Failed to get MARC record from Redis: {}", e))
            })?;

            let Some(json_str) = json_str else {
                continue;
            };

            let record: MarcRecord = serde_json::from_str(&json_str).map_err(|e| {
                AppError::Internal(format!("Failed to deserialize MARC record: {}", e))
            })?;

            let mut preview = MarcImportPreview::from(record);
            preview.biblio.id = record_idx as i64;
            previews.push(preview);
        }

        Ok(EnqueueResult { batch_id, previews })
    }

    /// Cache already-parsed records under a new batch id.
    ///
    /// Same Redis layout as the UNIMARC upload. Records are not split: each
    /// value is one bibliographic record and may carry several copies.
    pub async fn cache_parsed_records(&self, records: Vec<MarcRecord>) -> AppResult<i64> {
        let batch_id: i64 = snowflaked::Generator::new(1).generate::<i64>();
        let mut conn = self.redis.get_connection().await?;
        for (index, record) in records.into_iter().enumerate() {
            let json_str = serde_json::to_string(&record).map_err(|e| {
                AppError::Internal(format!("Failed to serialize MARC record: {}", e))
            })?;
            let record_key = Self::redis_key(batch_id, index);
            redis::cmd("SETEX")
                .arg(&record_key)
                .arg(Self::BATCH_TTL_SECS)
                .arg(&json_str)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to store MARC record in Redis: {}", e))
                })?;
            let index_key = Self::batch_index_key(batch_id);
            redis::cmd("SADD")
                .arg(&index_key)
                .arg(&record_key)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to index MARC record in Redis: {}", e))
                })?;
            redis::cmd("EXPIRE")
                .arg(&index_key)
                .arg(Self::BATCH_TTL_SECS)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::Internal(format!(
                        "Failed to set MARC batch index TTL in Redis: {}",
                        e
                    ))
                })?;
        }
        Ok(batch_id)
    }

    /// Import MARC records from a cached batch into the catalog.
    ///
    /// - If `record_id` is `None`, imports all records for the `batch_id`.
    /// - If `record_id` is `Some`, imports only the specified record.
    ///
    /// On error for a given record, the error is captured in the report and
    /// processing continues with the next record.
    ///
    /// When `deposit_batch_id` is set, copies are attached to the active record
    /// with the same ISBN, or the record is created when there is no match.
    /// A barcode that already exists skips that copy only.
    ///
    /// List lender-barcode conflicts for a cached batch before deposit import.
    #[tracing::instrument(skip(self), err)]
    pub async fn scan_deposit_duplicates(
        &self,
        batch_id: i64,
        deposit_batch_id: i64,
        record_id: Option<usize>,
    ) -> AppResult<DepositDuplicateScanReport> {
        let batch = self.deposits.require_active_batch(deposit_batch_id).await?;
        let records = self.load_batch_biblios(batch_id, record_id, true).await?;
        self.deposits
            .scan_marc_duplicates(batch_id, &batch, &records)
            .await
    }

    async fn load_batch_biblios(
        &self,
        batch_id: i64,
        record_id: Option<usize>,
        deposit_mode: bool,
    ) -> AppResult<Vec<(String, Biblio)>> {
        let mut conn = self.redis.get_connection().await?;
        let mut keys: Vec<String> = if let Some(rid) = record_id {
            vec![Self::redis_key(batch_id, rid)]
        } else {
            conn.smembers(Self::batch_index_key(batch_id))
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to list MARC batch keys in Redis: {}", e))
                })?
        };
        if deposit_mode {
            keys.sort_by_key(|key| {
                key.rsplit(':')
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0)
            });
        }
        let mut out = Vec::new();
        for key in keys {
            let json_str: Option<String> = conn.get(&key).await.map_err(|e| {
                AppError::Internal(format!("Failed to get MARC record from Redis: {}", e))
            })?;
            let Some(json_str) = json_str else { continue };
            let record: MarcRecord = serde_json::from_str(&json_str)
                .map_err(|e| AppError::Internal(format!("JSON decode error: {}", e)))?;
            let biblio: Biblio = record.into();
            out.push((Self::item_key_from_record_key(&key), biblio));
        }
        Ok(out)
    }

    /// When `task_handle` is provided, per-record progress is reported via
    /// [`TaskHandle::set_progress`].
    #[tracing::instrument(skip(self, task_handle), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn import_from_batch(
        &self,
        batch_id: i64,
        site_id: i64,
        record_id: Option<usize>,
        allow_duplicate_isbn: bool,
        confirm_replace_existing_id: Option<i64>,
        deposit_batch_id: Option<i64>,
        deposit_duplicate_policy: Option<DepositDuplicatePolicy>,
        task_handle: Option<TaskHandle>,
    ) -> AppResult<MarcBatchImportReport> {
        let deposit_batch = match deposit_batch_id {
            Some(id) => Some(self.deposits.require_active_batch(id).await?),
            None => None,
        };
        let mut conn = self.redis.get_connection().await?;

        let mut keys: Vec<String> = if let Some(rid) = record_id {
            vec![Self::redis_key(batch_id, rid)]
        } else {
            conn.smembers(Self::batch_index_key(batch_id))
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to list MARC batch keys in Redis: {}", e))
                })?
        };
        if deposit_batch.is_some() {
            keys.sort_by_key(|key| {
                key.rsplit(':')
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0)
            });
        }

        let mut imported = Vec::new();
        let mut created = Vec::new();
        let mut attached = Vec::new();
        let mut failed = Vec::new();
        let mut copies_created = Vec::new();
        let mut copies_reactivated = Vec::new();
        let mut copies_relabeled = Vec::new();
        let mut copies_refused = Vec::new();
        let mut copies_overwritten = Vec::new();
        let mut deposit_policy = deposit_duplicate_policy.map(DepositDuplicatePolicyRuntime::new);
        let total = keys.len();

        for (idx, key) in keys.iter().enumerate() {
            let key = key.clone();

            let json_str: Option<String> = conn.get(&key).await.map_err(|e| {
                AppError::Internal(format!("Failed to get MARC record from Redis: {}", e))
            })?;

            let Some(json_str) = json_str else {
                failed.push(MarcBatchImportError {
                    key: Self::item_key_from_record_key(&key),
                    error: "Record not found in Redis".to_string(),
                    existing_id: None,
                    code: None,
                    existing_item_id: None,
                });
                continue;
            };

            let record: MarcRecord = match serde_json::from_str(&json_str) {
                Ok(v) => v,
                Err(e) => {
                    failed.push(MarcBatchImportError {
                        key: Self::item_key_from_record_key(&key),
                        error: format!("JSON decode error: {}", e),
                        existing_id: None,
                        code: None,
                        existing_item_id: None,
                    });
                    continue;
                }
            };

            let mut biblio: Biblio = record.into();
            let record_key = Self::item_key_from_record_key(&key);
            if let Some(batch) = deposit_batch.as_ref() {
                match self
                    .deposits
                    .import_marc_record(biblio, batch, &mut deposit_policy)
                    .await
                {
                    Ok(outcome) => {
                        if let Some(biblio_id) = outcome.biblio_id {
                            if outcome.created {
                                created.push(record_key.clone());
                            } else {
                                attached.push(MarcBatchAttached {
                                    key: record_key.clone(),
                                    biblio_id,
                                });
                            }
                            imported.push(record_key.clone());
                        }
                        for line in outcome.copies.created {
                            copies_created.push(MarcDepositCopyLine {
                                record_key: record_key.clone(),
                                lender_barcode: line.lender_barcode,
                                item_id: line.item_id,
                                barcode: line.barcode,
                            });
                        }
                        for line in outcome.copies.reactivated {
                            copies_reactivated.push(MarcDepositCopyLine {
                                record_key: record_key.clone(),
                                lender_barcode: line.lender_barcode,
                                item_id: line.item_id,
                                barcode: line.barcode,
                            });
                        }
                        for line in outcome.copies.relabeled {
                            copies_relabeled.push(MarcDepositRelabeledLine {
                                record_key: record_key.clone(),
                                lender_barcode: line.lender_barcode,
                                item_id: line.item_id,
                                local_barcode: line.local_barcode,
                            });
                        }
                        for line in outcome.copies.overwritten {
                            copies_overwritten.push(MarcDepositCopyLine {
                                record_key: record_key.clone(),
                                lender_barcode: line.lender_barcode,
                                item_id: line.item_id,
                                barcode: line.barcode,
                            });
                        }
                        for line in outcome.copies.refused {
                            copies_refused.push(MarcDepositRefusedLine {
                                record_key: record_key.clone(),
                                lender_barcode: line.lender_barcode,
                                code: line.code.to_string(),
                                item_id: line.item_id,
                            });
                            failed.push(MarcBatchImportError {
                                key: record_key.clone(),
                                error: format!("Deposit copy refused ({})", line.code),
                                existing_id: None,
                                code: Some(line.code.to_string()),
                                existing_item_id: line.item_id,
                            });
                        }
                    }
                    Err(error) => {
                        failed.push(MarcBatchImportError {
                            key: record_key,
                            error: format!("Failed to import deposit copy: {error}"),
                            existing_id: None,
                            code: None,
                            existing_item_id: None,
                        });
                    }
                }
            } else {
                for item in &mut biblio.items {
                    item.site_id = Some(site_id);
                }

                match self
                    .catalog
                    .create_biblio(biblio, allow_duplicate_isbn, confirm_replace_existing_id)
                    .await
                {
                    Ok((_biblio, _report)) => {
                        imported.push(record_key);
                    }
                    Err(AppError::DuplicateNeedsConfirmation {
                        existing_id,
                        message,
                        ..
                    }) => {
                        failed.push(MarcBatchImportError {
                            key: record_key,
                            error: message,
                            existing_id: Some(existing_id),
                            code: None,
                            existing_item_id: None,
                        });
                        continue;
                    }
                    Err(e) => {
                        failed.push(MarcBatchImportError {
                            key: record_key,
                            error: format!("Failed to create item: {}", e),
                            existing_id: None,
                            code: None,
                            existing_item_id: None,
                        });
                        continue;
                    }
                }
            }
            if let Some(ref handle) = task_handle {
                handle
                    .set_progress(
                        idx + 1,
                        total,
                        Some(serde_json::json!({
                            "imported": imported,
                            "failed": failed,
                        })),
                    )
                    .await;
            }
        }

        Ok(MarcBatchImportReport {
            batch_id,
            imported,
            created,
            attached,
            failed,
            copies_created,
            copies_reactivated,
            copies_relabeled,
            copies_refused,
            copies_overwritten,
        })
    }
}

#[cfg(test)]
mod encoding_param_tests {
    use super::{
        default_marc_upload_encoding, marc_upload_encoding_from_param, resolve_marc_upload_encoding,
    };
    use z3950_rs::marc_rs::encoding::Encoding as MarcFileEncoding;

    #[test]
    fn default_is_utf8() {
        assert!(matches!(
            default_marc_upload_encoding(),
            MarcFileEncoding::Utf8
        ));
        assert!(matches!(
            resolve_marc_upload_encoding(None).unwrap(),
            MarcFileEncoding::Utf8
        ));
    }

    #[test]
    fn maps_known_params() {
        assert!(matches!(
            marc_upload_encoding_from_param("marc8").unwrap(),
            MarcFileEncoding::Marc8
        ));
        assert!(matches!(
            marc_upload_encoding_from_param("iso8859-2").unwrap(),
            MarcFileEncoding::Other(_)
        ));
    }

    #[test]
    fn rejects_unknown_param() {
        assert!(marc_upload_encoding_from_param("latin1").is_err());
    }
}
