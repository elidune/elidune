//! Z39.50 client service for remote catalog searches
//!
//! Uses the YAZ toolkit's yaz-client for Z39.50 protocol communication.

use chrono::Utc;
use sqlx::Row;
use tokio::process::Command;

use crate::{
    api::z3950::{ImportSpecimen, Z3950SearchQuery},
    error::{AppError, AppResult},
    marc::{MarcFormat, MarcRecord, MarcTranslator, DataField, Subfield},
    models::item::{Item, ItemShort},
    repository::Repository,
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
    format: String,
    login: Option<String>,
    password: Option<String>,
}

#[derive(Clone)]
pub struct Z3950Service {
    repository: Repository,
}

impl Z3950Service {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Search remote catalogs via Z39.50
    pub async fn search(&self, query: &Z3950SearchQuery) -> AppResult<(Vec<ItemShort>, i32, String)> {
        tracing::info!("Z39.50 search started");
        tracing::debug!("Search params - ISBN: {:?}, ISSN: {:?}, Title: {:?}, Author: {:?}, Keywords: {:?}",
            query.isbn, query.issn, query.title, query.author, query.keywords);

        let pool = &self.repository.pool;

        // Get active Z39.50 servers
        let server_query = if let Some(server_id) = query.server_id {
            tracing::debug!("Searching specific server ID: {}", server_id);
            sqlx::query(
                "SELECT id, name, address, port, database, format, login, password FROM z3950servers WHERE id = $1 AND activated = 1"
            )
            .bind(server_id)
        } else {
            tracing::debug!("Searching all active servers");
            sqlx::query(
                "SELECT id, name, address, port, database, format, login, password FROM z3950servers WHERE activated = 1"
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
                format: row.get("format"),
                login: row.get("login"),
                password: row.get("password"),
            })
            .collect();

        tracing::info!("Found {} active Z39.50 servers: {:?}", 
            servers.len(), 
            servers.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        // Build PQF query
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
                        
                        // Translate MARC records (format is stored in each record)
                        let translator = MarcTranslator::new();

                        for (rec_idx, record) in records.iter().enumerate() {
                            tracing::debug!("Processing record {}/{} (format: {:?})", rec_idx + 1, records.len(), record.format);
                            
                            match self.cache_record(record, &translator, &server.name).await {
                                Ok(item_id) => {
                                    tracing::debug!("Cached record as remote_item id={}", item_id);
                                    // Fetch the cached item as ItemShort
                                    if let Ok(item) = self.get_cached_item_short(item_id).await {
                                        all_items.push(item);
                                    }
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

    /// Build a PQF (Prefix Query Format) query from search parameters
    fn build_pqf_query(&self, query: &Z3950SearchQuery) -> AppResult<String> {
        let mut terms = Vec::new();

        // ISBN: Bib-1 attribute 7
        if let Some(ref isbn) = query.isbn {
            let clean_isbn = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
            terms.push(format!("@attr 1=7 \"{}\"", clean_isbn));
        }

        // ISSN: Bib-1 attribute 8
        if let Some(ref issn) = query.issn {
            terms.push(format!("@attr 1=8 \"{}\"", issn));
        }

        // Title: Bib-1 attribute 4
        if let Some(ref title) = query.title {
            terms.push(format!("@attr 1=4 \"{}\"", title));
        }

        // Author: Bib-1 attribute 1003
        if let Some(ref author) = query.author {
            terms.push(format!("@attr 1=1003 \"{}\"", author));
        }

        // Keywords: Bib-1 attribute 21 (Subject)
        if let Some(ref keywords) = query.keywords {
            terms.push(format!("@attr 1=21 \"{}\"", keywords));
        }

        if terms.is_empty() {
            return Err(AppError::Validation("At least one search term required".to_string()));
        }

        // Combine terms with AND (@and)
        let mut result = terms.pop().unwrap();
        while let Some(term) = terms.pop() {
            result = format!("@and {} {}", term, result);
        }

        Ok(result)
    }

    /// URL-encode a database name for Z39.50 connection
    fn encode_database_name(name: &str) -> String {
        // Percent-encode special characters for URL
        let mut result = String::new();
        for byte in name.bytes() {
            match byte {
                // Safe characters that don't need encoding
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                // Everything else gets percent-encoded
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }

    /// Query a single Z39.50 server using yaz-client
    async fn query_server(
        &self,
        server: &Z3950Server,
        pqf_query: &str,
        max_results: usize,
    ) -> AppResult<Vec<MarcRecord>> {
        let encoded_db = Self::encode_database_name(&server.database);
        
        // Build connection URL: host:port/database
        let connect_url = format!("{}:{}/{}", server.address, server.port, encoded_db);
        
        // Build auth option if credentials available
        let auth_option = if let (Some(ref login), Some(ref password)) = (&server.login, &server.password) {
            format!(" -u {}/{}", login, password)
        } else {
            String::new()
        };
        
        let auth_option_log = if let (Some(ref login), Some(_)) = (&server.login, &server.password) {
            format!(" -u {}/***", login)
        } else {
            String::new()
        };

        tracing::info!("Z39.50 search starting on server: {}", server.name);
        tracing::debug!("Z39.50 connection: yaz-client {}{}", connect_url, auth_option_log);
        tracing::debug!("Z39.50 PQF query: {}", pqf_query);
        tracing::debug!("Z39.50 max results: {}", max_results);

        // Build yaz-client command script (no open needed, URL passed as argument)
        let yaz_script = format!(
            "format {}\nfind {}\nshow 1+{}\nquit\n",
            server.format, pqf_query, max_results
        );
        tracing::debug!("yaz-client script:\n{}", yaz_script);

        // Execute yaz-client with URL as argument and -u for auth
        let start_time = std::time::Instant::now();
        
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "echo '{}' | yaz-client {} -m -{}",
                    yaz_script.replace("'", "'\\''"),
                    connect_url,
                    auth_option
                ))
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
        )
        .await;

        let elapsed = start_time.elapsed();
        tracing::debug!("yaz-client execution time: {:?}", elapsed);

        let output = match result {
            Ok(Ok(output)) => {
                tracing::debug!("yaz-client exit status: {:?}", output.status);
                tracing::debug!("yaz-client stdout size: {} bytes", output.stdout.len());
                tracing::debug!("yaz-client stderr size: {} bytes", output.stderr.len());
                
                if !output.stderr.is_empty() {
                    let stderr_str = String::from_utf8_lossy(&output.stderr);
                    tracing::debug!("yaz-client stderr: {}", stderr_str);
                }
                
                if !output.stdout.is_empty() {
                    // Log first 500 chars of stdout for debugging
                    let stdout_preview = String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(500)]);
                    tracing::debug!("yaz-client stdout preview: {}", stdout_preview);
                }
                
                output
            }
            Ok(Err(e)) => {
                tracing::warn!("yaz-client execution failed: {}", e);
                return Err(AppError::Z3950("Failed to query Z39.50 server".to_string()));
            }
            Err(_) => {
                tracing::warn!("yaz-client timeout after 30s for {}", server.name);
                return Err(AppError::Z3950("Timeout querying Z39.50 server".to_string()));
            }
        };

        // Parse MARC records from output
        let records = self.parse_yaz_output(&output.stdout)?;
        tracing::info!("yaz-client returned {} MARC records from {}", records.len(), server.name);


        Ok(records)
    }

 

    /// Parse yaz-client text output to extract MARC records
    /// Detects record type from yaz-client output (e.g., "[TOUT-UTF8]Record type: Unimarc")
    fn parse_yaz_output(&self, output: &[u8]) -> AppResult<Vec<MarcRecord>> {
        tracing::debug!("Parsing yaz-client output, total size: {} bytes", output.len());
        
        let output_str = String::from_utf8_lossy(output);
        let mut records = Vec::new();
        let mut current_format = MarcFormat::Marc21;
        let mut current_record_lines: Vec<&str> = Vec::new();
        let mut in_record = false;
        
        for line in output_str.lines() {
            // Check for record type header: "[database]Record type: Unimarc"
            if line.contains("Record type:") {
                // Save previous record if any
                if !current_record_lines.is_empty() {
                    if let Some(record) = self.parse_marc_text_record(&current_record_lines, current_format) {
                        records.push(record);
                    }
                    current_record_lines.clear();
                }
                
                // Extract format
                if let Some(type_idx) = line.find("Record type:") {
                    let record_type = line[type_idx + 12..].trim();
                    current_format = MarcFormat::from_yaz_output(record_type);
                    tracing::debug!("Detected record format: {:?} from '{}'", current_format, record_type);
                }
                in_record = true;
                continue;
            }
            
            // Skip non-record lines
            if !in_record {
                continue;
            }
            
            // Empty line might indicate end of record
            if line.trim().is_empty() {
                if !current_record_lines.is_empty() {
                    if let Some(record) = self.parse_marc_text_record(&current_record_lines, current_format) {
                        records.push(record);
                    }
                    current_record_lines.clear();
                    in_record = false;
                }
                continue;
            }
            
            // Collect record lines
            current_record_lines.push(line);
        }
        
        // Don't forget last record
        if !current_record_lines.is_empty() {
            if let Some(record) = self.parse_marc_text_record(&current_record_lines, current_format) {
                records.push(record);
            }
        }

        tracing::info!("Parsed {} MARC records from yaz-client output", records.len());
        
        Ok(records)
    }
    
    /// Parse a single MARC record from yaz-client text format
    /// Format:
    /// ```
    /// 00709nas  2200241   450    <- leader (24 chars)
    /// 001 FRBNF371242740000009   <- control field
    /// 200 1  $a Title $b Other   <- data field with indicators and subfields
    /// ```
    fn parse_marc_text_record(&self, lines: &[&str], format: MarcFormat) -> Option<MarcRecord> {
        use std::collections::HashMap;
        
        if lines.is_empty() {
            return None;
        }
        
        // First line should be the leader (24 characters, possibly with trailing spaces)
        let leader_line = lines[0];
        let leader = if leader_line.len() >= 24 {
            leader_line[..24].to_string()
        } else {
            leader_line.to_string()
        };
        
        let mut control_fields = HashMap::new();
        let mut data_fields = Vec::new();
        
        for line in &lines[1..] {
            if line.len() < 3 {
                continue;
            }
            
            let tag = &line[..3];
            
            // Control fields (00X) don't have indicators
            if tag.starts_with("00") {
                let data = line[3..].trim().to_string();
                control_fields.insert(tag.to_string(), data);
            } else {
                // Data field: "TAG I1I2 $a data $b data..."
                // or "TAG I1 $a data..." (some fields have only one indicator shown)
                if line.len() < 4 {
                    continue;
                }
                
                // Find where subfields start
                let subfield_start = line.find("$");
                if subfield_start.is_none() {
                    continue;
                }
                let subfield_start = subfield_start.unwrap();
                
                // Extract indicators (between tag and first $)
                let indicator_part = line[3..subfield_start].trim();
                let (ind1, ind2) = if indicator_part.len() >= 2 {
                    (
                        indicator_part.chars().next().unwrap_or(' '),
                        indicator_part.chars().nth(1).unwrap_or(' ')
                    )
                } else if indicator_part.len() == 1 {
                    (indicator_part.chars().next().unwrap_or(' '), ' ')
                } else {
                    (' ', ' ')
                };
                
                // Parse subfields
                let subfields_str = &line[subfield_start..];
                let mut subfields = Vec::new();
                
                // Split by $ and parse each subfield
                for part in subfields_str.split('$').skip(1) {
                    if part.is_empty() {
                        continue;
                    }
                    let code = part.chars().next().unwrap_or(' ');
                    let data = part[1..].trim_start().to_string();
                    subfields.push(Subfield { code, data });
                }
                
                if !subfields.is_empty() {
                    data_fields.push(DataField {
                        tag: tag.to_string(),
                        ind1,
                        ind2,
                        subfields,
                    });
                }
            }
        }
        
        let record = MarcRecord {
            leader,
            control_fields,
            data_fields,
            format,
        };
        
        // Log title for debugging
        let title = match format {
            MarcFormat::Unimarc => record.get_subfield("200", 'a'),
            MarcFormat::Marc21 => record.get_subfield("245", 'a'),
        }.unwrap_or("<no title>");
        tracing::debug!("Parsed {:?} record: {}", format, title);
        
        Some(record)
    }

    /// Cache a MARC record in remote_items table
    async fn cache_record(
        &self,
        record: &MarcRecord,
        translator: &MarcTranslator,
        source_name: &str,
    ) -> AppResult<i32> {
        let pool = &self.repository.pool;
        let item = translator.translate(record);
        
        let now = Utc::now();

        // Check if already cached (by ISBN)
        if let Some(ref identification) = item.identification {
            let existing: Option<i32> = sqlx::query_scalar(
                "SELECT id FROM remote_items WHERE identification = $1"
            )
            .bind(identification)
            .fetch_optional(pool)
            .await?;

            if let Some(id) = existing {
                // Update existing record
                sqlx::query("UPDATE remote_items SET modif_date = $1, state = $2 WHERE id = $3")
                    .bind(now)
                    .bind(source_name)
                    .bind(id)
                    .execute(pool)
                    .await?;
                return Ok(id);
            }
        }

        // Insert new cached record
        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO remote_items (
                media_type, identification, title1, title2, title3, title4,
                publication_date, nb_pages, format, dewey, lang, lang_orig,
                genre, subject, public_type, abstract, notes, keywords,
                author1_ids, author1_functions,
                state, is_valid, crea_date, modif_date
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, 1, $22, $22
            ) RETURNING id
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.identification)
        .bind(&item.title1)
        .bind(&item.title2)
        .bind(&item.title3)
        .bind(&item.title4)
        .bind(&item.publication_date)
        .bind(&item.nb_pages)
        .bind(&item.format)
        .bind(&item.dewey)
        .bind(&item.lang)
        .bind(&item.lang_orig)
        .bind(&item.genre)
        .bind(&item.subject)
        .bind(&item.public_type)
        .bind(&item.abstract_)
        .bind(&item.notes)
        .bind(&item.keywords)
        .bind(None::<Vec<i32>>) // author1_ids - would need to create authors first
        .bind(None::<String>)   // author1_functions
        .bind(source_name)
        .bind(now)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    /// Get a cached item as ItemShort
    async fn get_cached_item_short(&self, id: i32) -> AppResult<ItemShort> {
        let pool = &self.repository.pool;
        
        let item = sqlx::query_as::<_, ItemShort>(
            r#"
            SELECT id, media_type, identification, title1 as title,
                   publication_date as date, 0::smallint as status,
                   0::smallint as is_local, is_archive, is_valid
            FROM remote_items
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(item)
    }

    /// Search in cached remote_items
    async fn search_cache(&self, query: &Z3950SearchQuery) -> AppResult<(Vec<ItemShort>, i32, String)> {
        let pool = &self.repository.pool;

        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<String> = Vec::new();

        if let Some(ref isbn) = query.isbn {
            let clean_isbn = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
            params.push(clean_isbn);
            conditions.push(format!("identification LIKE '%' || ${} || '%'", params.len()));
        }

        if let Some(ref title) = query.title {
            params.push(format!("%{}%", title.to_lowercase()));
            conditions.push(format!("LOWER(title1) LIKE ${}", params.len()));
        }

        if let Some(ref author) = query.author {
            // Search in authors - simplified
            params.push(format!("%{}%", author.to_lowercase()));
            conditions.push(format!(
                "(LOWER(state) LIKE ${} OR EXISTS (SELECT 1 FROM authors a WHERE a.id = ANY(author1_ids) AND LOWER(a.lastname) LIKE ${}))",
                params.len(), params.len()
            ));
        }

        if let Some(ref keywords) = query.keywords {
            params.push(format!("%{}%", keywords.to_lowercase()));
            conditions.push(format!("(LOWER(keywords) LIKE ${} OR LOWER(subject) LIKE ${})", params.len(), params.len()));
        }

        let where_clause = conditions.join(" AND ");
        let max_results = query.max_results.unwrap_or(50);

        let query_str = format!(
            r#"
            SELECT id, media_type, identification, title1 as title,
                   publication_date as date, 0::smallint as status,
                   0::smallint as is_local, is_archive, is_valid
            FROM remote_items
            WHERE {}
            ORDER BY title1
            LIMIT {}
            "#,
            where_clause, max_results
        );

        let mut query_builder = sqlx::query_as::<_, ItemShort>(&query_str);
        for param in &params {
            query_builder = query_builder.bind(param);
        }

        let items = query_builder.fetch_all(pool).await?;
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

        // Get remote item
        let remote_row = sqlx::query(
            "SELECT * FROM remote_items WHERE id = $1"
        )
        .bind(remote_item_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Remote item not found".to_string()))?;

        // Check if already imported
        let identification: Option<String> = remote_row.get("identification");
        if let Some(ref id) = identification {
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
            )
            SELECT
                media_type, identification, price, barcode, dewey, publication_date,
                lang, lang_orig, title1, title2, title3, title4,
                author1_ids, author1_functions, author2_ids, author2_functions,
                author3_ids, author3_functions, serie_id, serie_vol_number,
                collection_id, collection_number_sub, collection_vol_number,
                genre, subject, public_type, edition_id, edition_date,
                nb_pages, format, content, addon, abstract, notes, keywords,
                1, $1, $1
            FROM remote_items WHERE id = $2
            RETURNING id
            "#,
        )
        .bind(now)
        .bind(remote_item_id)
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

        // Mark remote item as imported
        sqlx::query("UPDATE remote_items SET is_archive = 1 WHERE id = $1")
            .bind(remote_item_id)
            .execute(pool)
            .await?;

        // Return the imported item
        self.repository.items.get_by_id(item_id).await
    }
}

