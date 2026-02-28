//! Z39.50 client service for remote catalog searches
//!
//! Uses the z3950-rs crate for Z39.50 protocol communication.

use chrono::Utc;
use sqlx::Row;
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use redis::AsyncCommands;

use z3950_rs::marc_rs::{Encoding, MarcFormat, Record as MarcRecord};
use z3950_rs::{Client, QueryLanguage};
use crate::{
    api::z3950::{ImportSpecimen, Z3950SearchQuery},
    error::{AppError, AppResult},
    models::{
        import_report::{ImportAction, ImportReport},
        Item, ItemRemote, ItemRemoteShort,
    },
    repository::Repository,
    services::redis::RedisService,
};

/// Z39.50 server configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Z3950Server {
    id: i32,
    name: String,
    address: String,
    port: i32,
    database: String,
    login: Option<String>,
    password: Option<String>,
    format: MarcFormat,
    encoding: Encoding,
}

#[derive(Clone)]
pub struct Z3950Service {
    repository: Repository,
    redis: RedisService,
    cache_ttl_seconds: u64,
}

impl Z3950Service {
    pub fn new(repository: Repository, redis: RedisService, cache_ttl_seconds: u64) -> Self {
        Self { repository, redis, cache_ttl_seconds }
    }

    /// Search remote catalogs via Z39.50
    pub async fn search(&self, query: &Z3950SearchQuery) -> AppResult<(Vec<ItemRemoteShort>, i32, String)> {
        tracing::info!("Z39.50 search started");
        tracing::debug!("Search params - query: {}", query.query);

        let pool = &self.repository.pool;

        // Get active Z39.50 servers
        let server_query = if let Some(server_id) = query.server_id {
            tracing::debug!("Searching specific server ID: {}", server_id);
            sqlx::query(
                "SELECT id, name, address, port, database, format, login, password, encoding FROM z3950servers WHERE id = $1 AND activated = 1"
            )
            .bind(server_id)
        } else {
            tracing::debug!("Searching all active servers");
            sqlx::query(
                "SELECT id, name, address, port, database, format, login, password, encoding FROM z3950servers WHERE activated = 1"
            )
        };

        let server_rows = server_query.fetch_all(pool).await?;

        if server_rows.is_empty() {
            tracing::warn!("No active Z39.50 servers found in database");
            return Err(AppError::Z3950("No active Z39.50 servers configured".to_string()));
        }

        let servers: Vec<Z3950Server> = server_rows
            .iter()
            .map(|row| Z3950Server {
                id: row.get("id"),
                name: row.get("name"),
                address: row.get("address"),
                port: row.get("port"),
                database: row.get("database"),
                format: row.get::<&str, _>("format").into(),
                login: row.get("login"),
                password: row.get("password"),
                encoding: row.get::<&str, _>("encoding").into(),
            })
            .collect();

        tracing::info!("Found {} active Z39.50 servers: {:?}", 
            servers.len(), 
            servers.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        // Build PQF query string
        let max_results = query.max_results.unwrap_or(50) as usize;
        

        let mut all_items = Vec::new();
        let mut sources = Vec::new();
        let search_start = std::time::Instant::now();

        // Query each server
        for (idx, server) in servers.iter().enumerate() {
            tracing::info!("Querying server {}/{}: {}", idx + 1, servers.len(), server.name);
            
            match self.query_server(server, &query).await {
                Ok(records) => {
                    tracing::info!("Server {} returned {} records", server.name, records.len());
                    
                    if !records.is_empty() {
                        sources.push(server.name.clone());
                        
                        for (rec_idx, record) in records.iter().enumerate() {
                            tracing::debug!("Processing record {}/{}", rec_idx + 1, records.len());
                            
                            match self.upsert_cache_record(record, &server.name).await {
                                Ok(item_remote_short) => {
                                    tracing::debug!("Cached record as remote_item id={}", item_remote_short.id);
                                    all_items.push(item_remote_short);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to cache record {}: {}", rec_idx + 1, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to query server {}: {}", server.name, e);
                }
            }

            // Stop if we have enough results
            if all_items.len() >= max_results {
                tracing::debug!("Reached max results ({}), stopping server queries", max_results);
                break;
            }
        }

        let search_elapsed = search_start.elapsed();
        tracing::info!("Z39.50 live search completed in {:?}, found {} items", search_elapsed, all_items.len());

       
        let total = all_items.len() as i32;
        let source = if sources.is_empty() {
            "cache".to_string()
        } else {
            sources.join(", ")
        };

        tracing::info!("Z39.50 search complete: {} results from {}", total, source);
        Ok((all_items, total, source))
    }

  

    /// Query a single Z39.50 server using z3950-rs
    async fn query_server(
        &self,
        server: &Z3950Server,
        query: &Z3950SearchQuery,
    ) -> AppResult<Vec<MarcRecord>> {
        // Build connection address: host:port
        let addr = format!("{}:{}", server.address, server.port);
        
        tracing::info!("Z39.50 search starting on server: {}", server.name);
        tracing::debug!("Z39.50 connection: {} (database: {})", addr, server.database);
        tracing::debug!("Z39.50 query: {:?}", query);

        // Connect to server
        let credentials = if let (Some(ref login), Some(ref password)) = (&server.login, &server.password) {
            Some((login.as_str(), password.as_str()))
        } else {
            None
        };

        let mut client = if let Some((login, password)) = credentials {
            Client::connect_with_credentials(&addr, Some((login, password))).await
                .map_err(|e| {
                    tracing::warn!("Failed to connect to Z39.50 server {}: {}", server.name, e);
                    AppError::Z3950(format!("Failed to connect to Z39.50 server: {}", e))
                })?
        } else {
            Client::connect(&addr).await
                .map_err(|e| {
                    tracing::warn!("Failed to connect to Z39.50 server {}: {}", server.name, e);
                    AppError::Z3950(format!("Failed to connect to Z39.50 server: {}", e))
                })?
        };

        // Search
        let databases = if server.database.is_empty() {
            &["default" as &str]
        } else {
            &[server.database.as_str()]
        };

        let search_response = client.search(databases, QueryLanguage::CQL(query.query.clone())).await
            .map_err(|e| {
                tracing::warn!("Z39.50 search failed on {}: {}", server.name, e);
                AppError::Z3950(format!("Z39.50 search failed: {}", e))
            })?;

        // Convert Integer to usize
        let hits = usize::try_from(&search_response.result_count)
            .unwrap_or_else(|_| search_response.result_count.to_string().parse::<usize>().unwrap_or(0));
        tracing::debug!("Z39.50 search returned {} hits on {}", hits, server.name);

        if hits == 0 {
            let _ = client.close().await;
            return Ok(Vec::new());
        }

        // Present records
        let count = std::cmp::min(hits, query.max_results.unwrap_or(50) as usize);
        let records = client.present_marc(1, count as i64).await
            .map_err(|e| {
                tracing::warn!("Z39.50 present failed on {}: {}", server.name, e);
                AppError::Z3950(format!("Z39.50 present failed: {}", e))
            })?;

        // Close connection
        let _ = client.close().await;

        // z3950_rs::MarcRecord is already a marc_rs::Record
        tracing::info!("z3950-rs returned {} MARC records from {}", records.len(), server.name);
        Ok(records)
    }

    /// Generate a stable ID from ISBN string
    fn generate_id_from_isbn(isbn: &str) -> i32 {
        let mut hasher = DefaultHasher::new();
        isbn.hash(&mut hasher);
        let hash = hasher.finish();
        // Convert to i32, using absolute value to ensure positive
        (hash as i32).abs()
    }

    /// Get Redis key for a cached item
    fn get_redis_key(isbn: &str) -> String {
        format!("z3950:item:{}", isbn)
    }

    /// Get Redis key for ID mapping
    fn get_id_mapping_key(id: i32) -> String {
        format!("z3950:id:{}", id)
    }

    /// Upsert a MARC record in Redis cache and return ItemRemoteShort
    async fn upsert_cache_record(
        &self,
        record: &MarcRecord,
        source_name: &str,
    ) -> AppResult<ItemRemoteShort> {

        let mut item_remote: ItemRemote = record.clone().into();
        let now = Utc::now();
        
        // Use ISBN as key, or generate a temporary one if missing
        let isbn_key = item_remote.isbn.clone()
            .unwrap_or_else(|| format!("temp:{}", now.timestamp_micros()));
        
        let redis_key = Self::get_redis_key(&isbn_key);
        
        // Update item with source and timestamp
        item_remote.state = Some(source_name.to_string());
        item_remote.modif_date = Some(now);
        if item_remote.crea_date.is_none() {
            item_remote.crea_date = Some(now);
        }
        item_remote.is_valid = Some(1);
        item_remote.is_archive = Some(0);
        
        // Generate ID from ISBN for compatibility
        let id = Self::generate_id_from_isbn(&isbn_key);
        item_remote.id = Some(id);
        
        // Serialize to JSON and store in Redis
        let json_value = serde_json::to_value(&item_remote)
            .map_err(|e| AppError::Internal(format!("Failed to serialize item for Redis: {}", e)))?;
        let json_str = serde_json::to_string(&json_value)
            .map_err(|e| AppError::Internal(format!("Failed to serialize item to JSON: {}", e)))?;
        
        let mut conn = self.redis.get_connection().await?;
        
        // Store item by ISBN
        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(self.cache_ttl_seconds)
            .arg(&json_str)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store item in Redis: {}", e)))?;
        
        // Store ID -> ISBN mapping for import_record compatibility
        let id_mapping_key = Self::get_id_mapping_key(id);
        redis::cmd("SETEX")
            .arg(&id_mapping_key)
            .arg(self.cache_ttl_seconds)
            .arg(&isbn_key)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store ID mapping in Redis: {}", e)))?;
        
        tracing::debug!("Cached item in Redis with key: {}, ID mapping: {}, TTL: {}s", redis_key, id_mapping_key, self.cache_ttl_seconds);
        
        // Convert to ItemRemoteShort
        Ok(item_remote.into())
    }

    /// Search in cached items from Redis


  

    /// Import a record from Z39.50 cache into local catalog.
    /// Applies ISBN deduplication: merge, replace or create depending on context.
    pub async fn import_record(
        &self,
        remote_item_id: i32,
        specimens: Option<Vec<ImportSpecimen>>,
        confirm_replace_existing_id: Option<i32>,
    ) -> AppResult<(Item, ImportReport)> {
        let pool = &self.repository.pool;
        let mut conn = self.redis.get_connection().await?;

        // Get ISBN from ID mapping
        let id_mapping_key = Self::get_id_mapping_key(remote_item_id);
        let isbn_key: Option<String> = conn
            .get(&id_mapping_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get ID mapping from Redis: {}", e)))?;

        let isbn_key = isbn_key
            .ok_or_else(|| AppError::NotFound("Remote item not found in cache".to_string()))?;

        // Get remote item from Redis
        let redis_key = Self::get_redis_key(&isbn_key);
        let json_str: Option<String> = conn
            .get(&redis_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get item from Redis: {}", e)))?;

        let item_remote: ItemRemote = serde_json::from_str(
            &json_str.ok_or_else(|| AppError::NotFound("Remote item not found in cache".to_string()))?
        )
        .map_err(|e| AppError::Internal(format!("Failed to deserialize item from Redis: {}", e)))?;

        // ISBN deduplication decision
        let (item, report) = if let Some(ref isbn) = item_remote.isbn {
            match self.repository.items_find_by_isbn_for_import(isbn).await? {
                Some(dup) if dup.specimen_count > 0 => {
                    tracing::info!(
                        "Import: merging bibliographic data into existing item id={} ({} active specimens)",
                        dup.item_id, dup.specimen_count
                    );
                    let item = self.repository.items_update_bibliographic_from_remote(dup.item_id, &item_remote).await?;
                    let report = ImportReport {
                        action: ImportAction::MergedBibliographic,
                        existing_id: Some(dup.item_id),
                        warnings: vec![],
                        message: Some(format!(
                            "Bibliographic data merged into existing item id={}. {} specimen(s) preserved.",
                            dup.item_id, dup.specimen_count
                        )),
                    };
                    (item, report)
                }
                Some(dup) if dup.archived_at.is_some() => {
                    tracing::info!(
                        "Import: replacing archived item id={} (no active specimens)",
                        dup.item_id
                    );
                    let item = self.repository.items_update_bibliographic_from_remote(dup.item_id, &item_remote).await?;
                    let report = ImportReport {
                        action: ImportAction::ReplacedArchived,
                        existing_id: Some(dup.item_id),
                        warnings: vec![],
                        message: Some(format!(
                            "Replaced archived item id={} with new bibliographic data.",
                            dup.item_id
                        )),
                    };
                    (item, report)
                }
                Some(dup) => {
                    if confirm_replace_existing_id == Some(dup.item_id) {
                        tracing::info!(
                            "Import: confirmed replacement of item id={} (no specimens, not archived)",
                            dup.item_id
                        );
                        let item = self.repository.items_update_bibliographic_from_remote(dup.item_id, &item_remote).await?;
                        let report = ImportReport {
                            action: ImportAction::ReplacedConfirmed,
                            existing_id: Some(dup.item_id),
                            warnings: vec![],
                            message: Some(format!(
                                "Replaced item id={} after confirmation.",
                                dup.item_id
                            )),
                        };
                        (item, report)
                    } else {
                        return Err(AppError::DuplicateNeedsConfirmation {
                            existing_id: dup.item_id,
                            message: format!(
                                "An item with ISBN {} already exists (id={}). \
                                 It has no specimens and is not archived. \
                                 Resend with confirm_replace_existing_id={} to replace it.",
                                isbn, dup.item_id, dup.item_id
                            ),
                        });
                    }
                }
                None => {
                    self.create_new_item(&item_remote, specimens, pool).await?
                }
            }
        } else {
            let mut warnings = vec![
                "No ISBN on imported record — duplicate check skipped. This may create silent duplicates.".to_string(),
            ];
            tracing::warn!("Import: no ISBN on remote item, skipping dedup");
            let (item, mut report) = self.create_new_item(&item_remote, specimens, pool).await?;
            report.warnings.append(&mut warnings);
            (item, report)
        };

        // Remove from Redis cache (item is now imported)
        let _: () = conn
            .del(&redis_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete item from Redis: {}", e)))?;
        let _: () = conn
            .del(&id_mapping_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete ID mapping from Redis: {}", e)))?;

        Ok((item, report))
    }

    /// Create a brand-new item from a remote record (no duplicate found).
    async fn create_new_item(
        &self,
        item_remote: &ItemRemote,
        specimens: Option<Vec<ImportSpecimen>>,
        pool: &sqlx::Pool<sqlx::Postgres>,
    ) -> AppResult<(Item, ImportReport)> {
        let now = Utc::now();

        let item_id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO items (
                media_type, isbn, price, barcode, publication_date,
                lang, lang_orig, title,
                series_id, series_volume_number,
                collection_id, collection_sequence_number, collection_volume_number,
                genre, subject, audience_type, edition_id,
                page_extent, format, table_of_contents, accompanying_material,
                abstract, notes, keywords, is_valid, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15, $16, $17,
                $18, $19, $20, $21, $22, $23, $24, $25, $26, $27
            )
            RETURNING id
            "#,
        )
        .bind(&item_remote.media_type)
        .bind(&item_remote.isbn)
        .bind(&item_remote.price)
        .bind(&item_remote.barcode)
        .bind(&item_remote.publication_date)
        .bind(&item_remote.lang)
        .bind(&item_remote.lang_orig)
        .bind(&item_remote.title1)
        .bind(&item_remote.serie_id)
        .bind(&item_remote.serie_vol_number)
        .bind(&item_remote.collection_id)
        .bind(&item_remote.collection_number_sub)
        .bind(&item_remote.collection_vol_number)
        .bind(&item_remote.genre)
        .bind(&item_remote.subject)
        .bind(&item_remote.public_type)
        .bind(&item_remote.edition_id)
        .bind(&item_remote.nb_pages)
        .bind(&item_remote.format)
        .bind(&item_remote.content)
        .bind(&item_remote.addon)
        .bind(&item_remote.abstract_)
        .bind(&item_remote.notes)
        .bind(&item_remote.keywords)
        .bind(&item_remote.is_valid.unwrap_or(1))
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await?;

        if let Some(ref json) = item_remote.authors1_json {
            if let Ok(authors) = serde_json::from_value::<Vec<crate::models::author::AuthorWithFunction>>(json.clone()) {
                for (idx, author) in authors.iter().enumerate() {
                    if let Some(ref lastname) = author.lastname {
                        let author_id: i32 = sqlx::query_scalar(
                            "SELECT id FROM authors WHERE lastname = $1 AND firstname IS NOT DISTINCT FROM $2"
                        )
                        .bind(lastname)
                        .bind(&author.firstname)
                        .fetch_optional(pool)
                        .await?
                        .unwrap_or(0);

                        let author_id = if author_id == 0 {
                            sqlx::query_scalar::<_, i32>(
                                "INSERT INTO authors (lastname, firstname) VALUES ($1, $2) RETURNING id"
                            )
                            .bind(lastname)
                            .bind(&author.firstname)
                            .fetch_one(pool)
                            .await?
                        } else {
                            author_id
                        };

                        let _ = sqlx::query(
                            "INSERT INTO item_authors (item_id, author_id, role, author_type, position) VALUES ($1, $2, $3, 0, $4) ON CONFLICT DO NOTHING"
                        )
                        .bind(item_id)
                        .bind(author_id)
                        .bind(&author.function)
                        .bind((idx + 1) as i16)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }

        if let Some(specimens) = specimens {
            let default_source_id = self.repository.sources_get_default().await?.map(|s| s.id);

            for specimen in specimens {
                let source_id = specimen.source_id.or(default_source_id);

                sqlx::query(
                    r#"
                    INSERT INTO specimens (item_id, barcode, call_number, place, borrow_status, notes, price, source_id, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
                    "#,
                )
                .bind(item_id)
                .bind(&specimen.barcode)
                .bind(&specimen.call_number)
                .bind(&specimen.place)
                .bind(specimen.status.as_ref().and_then(|s| s.parse::<i16>().ok()).unwrap_or(98))
                .bind(&specimen.notes)
                .bind(&specimen.price)
                .bind(&source_id)
                .bind(now)
                .execute(pool)
                .await?;
            }
        }

        let item = self.repository.items_get_by_id_or_isbn(&item_id.to_string()).await?;
        let record = crate::marc::MarcRecord::from(&item);
        self.repository.items_save_marc_record(item_id, &record).await?;

        let report = ImportReport {
            action: ImportAction::Created,
            existing_id: None,
            warnings: vec![],
            message: None,
        };

        Ok((item, report))
    }
}

/// Parse format from yaz-client output string (e.g., "Unimarc", "USmarc", "MARC21")
fn from_yaz_output_format(s: &str) -> MarcFormat {
    let s_lower = s.to_lowercase();
    if s_lower.contains("unimarc") {
        MarcFormat::Unimarc
    } else if s_lower.contains("marcxml") {
        MarcFormat::MarcXml
    } else {
        MarcFormat::Marc21
    }
}
