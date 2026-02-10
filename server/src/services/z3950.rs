//! Z39.50 client service for remote catalog searches
//!
//! Uses the z3950-rs crate for Z39.50 protocol communication.

use chrono::Utc;
use sqlx::Row;
use serde_json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use redis::AsyncCommands;

use marc_rs::{Encoding, MarcFormat, Record as MarcRecord};
use z3950_rs::Client;
use crate::{
    api::z3950::{ImportSpecimen, Z3950SearchQuery},
    error::{AppError, AppResult},
    models::{Item, ItemRemote, ItemRemoteShort},
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
        tracing::debug!("Search params - ISBN: {:?}, ISSN: {:?}, Title: {:?}, Author: {:?}, Keywords: {:?}",
            query.isbn, query.issn, query.title, query.author, query.keywords);

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
        let pqf_query = self.build_pqf_query(query)?;
        let max_results = query.max_results.unwrap_or(50) as usize;
        
        tracing::info!("PQF query: {}", pqf_query);

        let mut all_items = Vec::new();
        let mut sources = Vec::new();
        let search_start = std::time::Instant::now();

        // Query each server
        for (idx, server) in servers.iter().enumerate() {
            tracing::info!("Querying server {}/{}: {}", idx + 1, servers.len(), server.name);
            
            match self.query_server(server, &pqf_query, max_results).await {
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

        // If no results from live search, try cache
        if all_items.is_empty() {
            tracing::info!("No live results, falling back to cache search");
            return self.search_cache(query).await;
        }

        let total = all_items.len() as i32;
        let source = if sources.is_empty() {
            "cache".to_string()
        } else {
            sources.join(", ")
        };

        tracing::info!("Z39.50 search complete: {} results from {}", total, source);
        Ok((all_items, total, source))
    }

    /// Build a PQF (Prefix Query Format) query string from search parameters
    fn build_pqf_query(&self, query: &Z3950SearchQuery) -> AppResult<String> {
        let mut terms = Vec::new();

        // ISBN: Bib-1 attribute 7
        if let Some(ref isbn) = query.isbn {
            let clean_isbn = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
            terms.push(format!("@attr 1=7 {}", clean_isbn));
        }

        // ISSN: Bib-1 attribute 8
        if let Some(ref issn) = query.issn {
            terms.push(format!("@attr 1=8 {}", issn));
        }

        // Title: Bib-1 attribute 4
        if let Some(ref title) = query.title {
            terms.push(format!("@attr 1=4 {}", title));
        }

        // Author: Bib-1 attribute 1003
        if let Some(ref author) = query.author {
            terms.push(format!("@attr 1=1003 {}", author));
        }

        // Keywords: Bib-1 attribute 21 (Subject)
        if let Some(ref keywords) = query.keywords {
            terms.push(format!("@attr 1=21 {}", keywords));
        }

        if terms.is_empty() {
            return Err(AppError::Validation("At least one search term required".to_string()));
        }

        // Combine terms with AND operations: @and term1 term2
        let pqf_query = if terms.len() == 1 {
            terms[0].clone()
        } else {
            format!("@and {}", terms.join(" "))
        };

        Ok(pqf_query)
    }


    /// Query a single Z39.50 server using z3950-rs
    async fn query_server(
        &self,
        server: &Z3950Server,
        pqf_query: &str,
        max_results: usize,
    ) -> AppResult<Vec<MarcRecord>> {
        // Build connection address: host:port
        let addr = format!("{}:{}", server.address, server.port);
        
        tracing::info!("Z39.50 search starting on server: {}", server.name);
        tracing::debug!("Z39.50 connection: {} (database: {})", addr, server.database);
        tracing::debug!("Z39.50 max results: {}", max_results);

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

        let search_response = client.search(databases, z3950_rs::query_languages::QueryLanguage::CQL(pqf_query.to_string())).await
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
        let count = std::cmp::min(hits, max_results);
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

    /// Generate a stable ID from identification string
    fn generate_id_from_identification(identification: &str) -> i32 {
        let mut hasher = DefaultHasher::new();
        identification.hash(&mut hasher);
        let hash = hasher.finish();
        // Convert to i32, using absolute value to ensure positive
        (hash as i32).abs()
    }

    /// Get Redis key for a cached item
    fn get_redis_key(identification: &str) -> String {
        format!("z3950:item:{}", identification)
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
        
        // Use identification as key, or generate a temporary one if missing
        let identification = item_remote.identification.clone()
            .unwrap_or_else(|| format!("temp:{}", now.timestamp_micros()));
        
        let redis_key = Self::get_redis_key(&identification);
        
        // Update item with source and timestamp
        item_remote.state = Some(source_name.to_string());
        item_remote.modif_date = Some(now);
        if item_remote.crea_date.is_none() {
            item_remote.crea_date = Some(now);
        }
        item_remote.is_valid = Some(1);
        item_remote.is_archive = Some(0);
        
        // Generate ID from identification for compatibility
        let id = Self::generate_id_from_identification(&identification);
        item_remote.id = Some(id);
        
        // Serialize to JSON and store in Redis
        let json_value = serde_json::to_value(&item_remote)
            .map_err(|e| AppError::Internal(format!("Failed to serialize item for Redis: {}", e)))?;
        let json_str = serde_json::to_string(&json_value)
            .map_err(|e| AppError::Internal(format!("Failed to serialize item to JSON: {}", e)))?;
        
        let mut conn = self.redis.get_connection().await?;
        
        // Store item by identification
        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(self.cache_ttl_seconds)
            .arg(&json_str)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store item in Redis: {}", e)))?;
        
        // Store ID -> identification mapping for import_record compatibility
        let id_mapping_key = Self::get_id_mapping_key(id);
        redis::cmd("SETEX")
            .arg(&id_mapping_key)
            .arg(self.cache_ttl_seconds)
            .arg(&identification)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to store ID mapping in Redis: {}", e)))?;
        
        tracing::debug!("Cached item in Redis with key: {}, ID mapping: {}, TTL: {}s", redis_key, id_mapping_key, self.cache_ttl_seconds);
        
        // Convert to ItemRemoteShort
        Ok(item_remote.into())
    }

    /// Search in cached items from Redis


    /// Search in cached items from Redis
    async fn search_cache(&self, query: &Z3950SearchQuery) -> AppResult<(Vec<ItemRemoteShort>, i32, String)> {
        let mut conn = self.redis.get_connection().await?;
        
        // Get all keys matching the pattern
        let pattern = "z3950:item:*";
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get keys from Redis: {}", e)))?;
        
        tracing::debug!("Found {} cached items in Redis", keys.len());
        
        let max_results = query.max_results.unwrap_or(50) as usize;
        let mut items = Vec::new();
        
        // Fetch and filter items
        for key in keys {
            let json_str: Option<String> = conn
                .get(&key)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to get item from Redis: {}", e)))?;
            
            if let Some(json) = json_str {
                let item_remote: ItemRemote = serde_json::from_str(&json)
                    .map_err(|e| AppError::Internal(format!("Failed to deserialize item from Redis: {}", e)))?;
                
                // Apply filters
                let mut matches = true;
                
                if let Some(ref isbn) = query.isbn {
                    let clean_isbn = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
                    if let Some(ref ident) = item_remote.identification {
                        let clean_ident = ident.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
                        if !clean_ident.to_lowercase().contains(&clean_isbn.to_lowercase()) {
                            matches = false;
                        }
                    } else {
                        matches = false;
                    }
                }
                
                if matches && query.title.is_some() {
                    if let Some(ref title) = query.title {
                        if let Some(ref item_title) = item_remote.title1 {
                            if !item_title.to_lowercase().contains(&title.to_lowercase()) {
                                matches = false;
                            }
                        } else {
                            matches = false;
                        }
                    }
                }
                
                if matches && query.author.is_some() {
                    if let Some(ref author) = query.author {
                        let author_lower = author.to_lowercase();
                        let mut found = false;
                        
                        // Check in state (source name)
                        if let Some(ref state) = item_remote.state {
                            if state.to_lowercase().contains(&author_lower) {
                                found = true;
                            }
                        }
                        
                        // Check in authors JSON
                        if !found {
                            for json_field in [&item_remote.authors1_json, &item_remote.authors2_json, &item_remote.authors3_json] {
                                if let Some(json) = json_field {
                                    let json_str = json.to_string();
                                    if json_str.to_lowercase().contains(&author_lower) {
                                        found = true;
                                        break;
                                    }
                                }
                            }
                        }
                        
                        if !found {
                            matches = false;
                        }
                    }
                }
                
                if matches && query.keywords.is_some() {
                    if let Some(ref keywords) = query.keywords {
                        let keywords_lower = keywords.to_lowercase();
                        let mut found = false;
                        
                        if let Some(ref kw) = item_remote.keywords {
                            if kw.to_lowercase().contains(&keywords_lower) {
                                found = true;
                            }
                        }
                        
                        if !found {
                            if let Some(ref subject) = item_remote.subject {
                                if subject.to_lowercase().contains(&keywords_lower) {
                                    found = true;
                                }
                            }
                        }
                        
                        if !found {
                            matches = false;
                        }
                    }
                }
                
                if matches {
                    let item_short: ItemRemoteShort = item_remote.into();
                    items.push(item_short);
                    
                    if items.len() >= max_results {
                        break;
                    }
                }
            }
        }
        
        // Sort by title
        items.sort_by(|a, b| {
            let a_title = a.title.as_deref().unwrap_or("");
            let b_title = b.title.as_deref().unwrap_or("");
            a_title.cmp(b_title)
        });
        
        let total = items.len() as i32;
        Ok((items, total, "cache".to_string()))
    }

    /// Import a record from Z39.50 cache into local catalog
    pub async fn import_record(
        &self,
        remote_item_id: i32,
        specimens: Option<Vec<ImportSpecimen>>,
    ) -> AppResult<Item> {
        let pool = &self.repository.pool;
        let mut conn = self.redis.get_connection().await?;

        // Get identification from ID mapping
        let id_mapping_key = Self::get_id_mapping_key(remote_item_id);
        let identification: Option<String> = conn
            .get(&id_mapping_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get ID mapping from Redis: {}", e)))?;
        
        let identification = identification
            .ok_or_else(|| AppError::NotFound("Remote item not found in cache".to_string()))?;
        
        // Get remote item from Redis
        let redis_key = Self::get_redis_key(&identification);
        let json_str: Option<String> = conn
            .get(&redis_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to get item from Redis: {}", e)))?;
        
        let item_remote: ItemRemote = serde_json::from_str(
            &json_str.ok_or_else(|| AppError::NotFound("Remote item not found in cache".to_string()))?
        )
        .map_err(|e| AppError::Internal(format!("Failed to deserialize item from Redis: {}", e)))?;

        // Check if already imported
        if let Some(ref id) = item_remote.identification {
            let existing: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM items WHERE identification = $1)"
            )
            .bind(id)
            .fetch_one(pool)
            .await?;

            if existing {
                return Err(AppError::Conflict("Item already exists in local catalog".to_string()));
            }
        }

        // Copy to items table
        let now = Utc::now();

        let item_id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO items (
                media_type, identification, price, barcode, dewey, publication_date,
                lang, lang_orig, title1, title2, title3, title4,
                author1_ids, author1_functions, author2_ids, author2_functions,
                author3_ids, author3_functions, serie_id, serie_vol_number,
                collection_id, collection_number_sub, collection_vol_number,
                genre, subject, public_type, edition_id, edition_date,
                nb_pages, format, content, addon, abstract, notes, keywords,
                is_valid, crea_date, modif_date
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27,
                $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
                1, $41, $41
            )
            RETURNING id
            "#,
        )
        .bind(&item_remote.media_type)
        .bind(&item_remote.identification)
        .bind(&item_remote.price)
        .bind(&item_remote.barcode)
        .bind(&item_remote.dewey)
        .bind(&item_remote.publication_date)
        .bind(&item_remote.lang)
        .bind(&item_remote.lang_orig)
        .bind(&item_remote.title1)
        .bind(&item_remote.title2)
        .bind(&item_remote.title3)
        .bind(&item_remote.title4)
        .bind(&item_remote.author1_ids)
        .bind(&item_remote.author1_functions)
        .bind(&item_remote.author2_ids)
        .bind(&item_remote.author2_functions)
        .bind(&item_remote.author3_ids)
        .bind(&item_remote.author3_functions)
        .bind(&item_remote.serie_id)
        .bind(&item_remote.serie_vol_number)
        .bind(&item_remote.collection_id)
        .bind(&item_remote.collection_number_sub)
        .bind(&item_remote.collection_vol_number)
        .bind(&item_remote.genre)
        .bind(&item_remote.subject)
        .bind(&item_remote.public_type)
        .bind(&item_remote.edition_id)
        .bind(&item_remote.edition_date)
        .bind(&item_remote.nb_pages)
        .bind(&item_remote.format)
        .bind(&item_remote.content)
        .bind(&item_remote.addon)
        .bind(&item_remote.abstract_)
        .bind(&item_remote.notes)
        .bind(&item_remote.keywords)
        .bind(now)
        .fetch_one(pool)
        .await?;

        // Create specimens if provided
        if let Some(specimens) = specimens {
            for specimen in specimens {
                sqlx::query(
                    r#"
                    INSERT INTO specimens (id_item, identification, cote, status, crea_date, modif_date)
                    VALUES ($1, $2, $3, $4, $5, $5)
                    "#,
                )
                .bind(item_id)
                .bind(&specimen.identification)
                .bind(&specimen.cote)
                .bind(specimen.status.as_ref().and_then(|s| s.parse::<i16>().ok()).unwrap_or(98))
                .bind(now)
                .execute(pool)
                .await?;
            }

            // Update specimen count
            sqlx::query(
                "UPDATE items SET nb_specimens = (SELECT COUNT(*) FROM specimens WHERE id_item = $1) WHERE id = $1"
            )
            .bind(item_id)
            .execute(pool)
            .await?;
        }

        // Remove from Redis cache (item is now imported)
        let _: () = conn
            .del(&redis_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete item from Redis: {}", e)))?;
        let _: () = conn
            .del(&id_mapping_key)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to delete ID mapping from Redis: {}", e)))?;

        // Return the imported item
        self.repository.items.get_by_id(item_id).await
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
