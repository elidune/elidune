//! Items repository for database operations.
//!
//! Uses marc-rs types (Leader, MarcFormat, etc.) where applicable; DB serialization
//! uses the associated char or int (e.g. media_type string from Leader record_type).

use chrono::Utc;
use sqlx::{Pool, Postgres, Row};
use z3950_rs::marc_rs::{Leader, MarcFormat};

use crate::{
    error::{AppError, AppResult},
    models::{
        author::AuthorWithFunction,
        item::{Collection, Edition, Item, ItemQuery, ItemShort, Serie},
        specimen::{CreateSpecimen, Specimen},
    },
};

// --- MARC type → DB (char/int) conversion helpers ---

/// Converts MARC Leader record type (position 6) to DB media_type string.
/// Uses marc-rs MarcFormat for MARC21 vs UNIMARC mapping.
pub fn media_type_from_leader_for_db(leader: &Leader, format: MarcFormat) -> String {
    let record_type = leader.record_type;
    record_type_to_media_type_db(record_type, format)
}

/// Converts record type char (Leader position 6) to DB media_type string.
pub fn record_type_to_media_type_db(record_type: char, format: MarcFormat) -> String {
    let is_marc21 = format == MarcFormat::Marc21;
    if is_marc21 {
        match record_type {
            'a' | 't' => "b",
            'c' | 'd' => "bc",
            'g' => "v",
            'i' | 'j' => "a",
            'm' => "c",
            'k' => "i",
            _ => "u",
        }
    } else {
        match record_type {
            'a' | 'b' => "b",
            'c' | 'd' => "bc",
            'g' => "v",
            'i' | 'j' => "a",
            'm' => "c",
            'k' => "i",
            _ => "u",
        }
    }
    .to_string()
}

/// Strip all non-alphanumeric characters from ISBN (keep letters, digits, spaces)
fn sanitize_isbn(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
}

/// Generate a normalized key from a string (for series/collections lookup)
fn normalize_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => '_',
        })
        .collect::<String>()
        .replace("__", "_")
        .trim_matches('_')
        .to_string()
}

#[derive(Clone)]
pub struct ItemsRepository {
    pool: Pool<Postgres>,
}

impl ItemsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Get item by ID with all related data
    /// Get item by numeric ID or by ISBN.
    pub async fn get_by_id_or_isbn(&self, id_or_isbn: &str) -> AppResult<Item> {
        let mut item = if let Ok(id) = id_or_isbn.parse::<i32>() {
            sqlx::query_as::<_, Item>(
                r#"
                SELECT id, media_type, isbn, price, barcode, dewey,
                       publication_date, lang, lang_orig, title1, title2, title3, title4,
                       genre, subject, public_type, nb_pages, format, content, addon,
                       abstract as abstract_, notes, keywords, state,
                       serie_id, serie_vol_number, edition_id,
                       collection_id, collection_number_sub, collection_vol_number,
                       is_archive, is_valid, lifecycle_status, crea_date, modif_date, archived_date
                FROM items
                WHERE id = $1 AND lifecycle_status != 2
                "#,
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))?
        } else {
            sqlx::query_as::<_, Item>(
                r#"
                SELECT id, media_type, isbn, price, barcode, dewey,
                       publication_date, lang, lang_orig, title1, title2, title3, title4,
                       genre, subject, public_type, nb_pages, format, content, addon,
                       abstract as abstract_, notes, keywords, state,
                       serie_id, serie_vol_number, edition_id,
                       collection_id, collection_number_sub, collection_vol_number,
                       is_archive, is_valid, lifecycle_status, crea_date, modif_date, archived_date
                FROM items
                WHERE isbn = $1 AND lifecycle_status != 2
                "#,
            )
            .bind(id_or_isbn)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Item with ISBN {} not found", id_or_isbn)))?
        };

        let id = item.id.ok_or_else(|| AppError::Internal("Item id is null".to_string()))?;

        // Load authors
        item.authors1 = self.get_item_authors(id, "author1_ids", "author1_functions").await?;
        item.authors2 = self.get_item_authors(id, "author2_ids", "author2_functions").await?;
        item.authors3 = self.get_item_authors(id, "author3_ids", "author3_functions").await?;

        // Load serie
        item.serie = sqlx::query_as::<_, Serie>("SELECT id, key, name, issn FROM series WHERE id = $1")
            .bind(item.serie_id)
            .fetch_optional(&self.pool)
            .await?;

        // Load collection
        item.collection = sqlx::query_as::<_, Collection>("SELECT id, key, title1, title2, title3, issn FROM collections WHERE id = $1")
            .bind(item.collection_id)
            .fetch_optional(&self.pool)
            .await?;

        // Load edition
        let mut edition: Option<Edition> = sqlx::query_as::<_, Edition>("SELECT id, name, place FROM editions WHERE id = $1")
            .bind(item.edition_id)
            .fetch_optional(&self.pool)
            .await?;
        
        // Load edition_date from items table if edition exists
        if let Some(ref mut ed) = edition {
            let edition_date: Option<String> = sqlx::query_scalar("SELECT edition_date FROM items WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
            ed.date = edition_date;
        }
        
        item.edition = edition;

        // Load specimens
        item.specimens = self.get_specimens(id).await?;

        Ok(item)
    }

    /// Get authors for an item
    async fn get_item_authors(
        &self,
        item_id: i32,
        ids_col: &str,
        functions_col: &str,
    ) -> AppResult<Vec<AuthorWithFunction>> {
        let row = sqlx::query(&format!(
            "SELECT {}, {} FROM items WHERE id = $1",
            ids_col, functions_col
        ))
        .bind(item_id)
        .fetch_one(&self.pool)
        .await?;

        let author_ids: Option<Vec<i32>> = row.get(ids_col);
        let functions: Option<String> = row.get(functions_col);

        let mut authors = Vec::new();
        
        if let Some(ids) = author_ids {
            let func_list: Vec<&str> = functions
                .as_deref()
                .map(|f| f.split(',').collect())
                .unwrap_or_default();

            for (i, id) in ids.iter().enumerate() {
                if let Some(author) = sqlx::query_as::<_, crate::models::author::Author>(
                    "SELECT * FROM authors WHERE id = $1"
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await? {
                    authors.push(AuthorWithFunction {
                        id: author.id,
                        lastname: author.lastname,
                        firstname: author.firstname,
                        bio: author.bio,
                        notes: author.notes,
                        function: func_list.get(i).map(|s| s.to_string()),
                    });
                }
            }
        }

        Ok(authors)
    }

    /// Search items with pagination
    pub async fn search(&self, query: &ItemQuery) -> AppResult<(Vec<ItemShort>, i64)> {
        let page = query.page.unwrap_or(1);
        let per_page = query.per_page.unwrap_or(20);
        let offset = (page - 1) * per_page;

        let mut conditions = vec!["1=1".to_string()];

        if let Some(ref media_type) = query.media_type {
            conditions.push(format!("media_type = '{}'", media_type));
        }

        if let Some(ref isbn) = query.isbn {
            conditions.push(format!("isbn = '{}'", isbn));
        }

        if let Some(ref title) = query.title {
            conditions.push(format!(
                "(LOWER(title1) LIKE '%{}%' OR LOWER(title2) LIKE '%{}%')",
                title.to_lowercase(),
                title.to_lowercase()
            ));
        }

        if let Some(ref keywords) = query.keywords {
            conditions.push(format!("LOWER(keywords) LIKE '%{}%'", keywords.to_lowercase()));
        }

        if let Some(ref freesearch) = query.freesearch {
            conditions.push(format!(
                "search_vector @@ plainto_tsquery('french', '{}')",
                freesearch
            ));
        }

        if let Some(archive) = query.archive {
            conditions.push(format!("is_archive = {}", if archive { 1 } else { 0 }));
        } else {
            conditions.push("(is_archive = 0 OR is_archive IS NULL)".to_string());
        }
        
        // Exclude deleted items
        conditions.push("lifecycle_status != 2".to_string());

        let where_clause = conditions.join(" AND ");

        // Count total
        let count_query = format!("SELECT COUNT(*) FROM items WHERE {}", where_clause);
        let total: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&self.pool)
            .await?;

        // Fetch items
        let select_query = format!(
            r#"
            SELECT i.id, i.media_type, i.isbn, i.title1 as title, 
                   i.publication_date as date, 0::smallint as status,
                   1::smallint as is_local, i.is_archive, i.is_valid,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s
                       WHERE s.id_item = i.id
                         AND s.lifecycle_status != 2
                   ), 0::smallint)::smallint as nb_specimens,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s
                       WHERE s.id_item = i.id
                         AND s.lifecycle_status != 2
                         AND NOT EXISTS (
                             SELECT 1 FROM loans l
                             WHERE l.specimen_id = s.id
                               AND l.returned_date IS NULL
                         )
                   ), 0::smallint)::smallint as nb_available
            FROM items i
            WHERE {}
            ORDER BY i.title1
            LIMIT {} OFFSET {}
            "#,
            where_clause, per_page, offset
        );

        let items = sqlx::query_as::<_, ItemShort>(&select_query)
            .fetch_all(&self.pool)
            .await?;

        Ok((items, total))
    }

    /// Create a new item
    pub async fn create(&self, item: &Item) -> AppResult<Item> {
        let now = Utc::now();

        // Handle authors
        let (author1_ids, author1_functions) = self.process_authors(&item.authors1).await?;
        let (author2_ids, author2_functions) = self.process_authors(&item.authors2).await?;
        let (author3_ids, author3_functions) = self.process_authors(&item.authors3).await?;

        // Handle serie
        let serie_id = self.process_serie(&item.serie).await?;

        // Handle collection
        let collection_id = self.process_collection(&item.collection).await?;

        // Handle edition
        let edition_id = self.process_edition(&item.edition).await?;

        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO items (
                media_type, isbn, price, barcode, dewey, publication_date,
                lang, lang_orig, title1, title2, title3, title4, genre, subject,
                public_type, nb_pages, format, content, addon, abstract, notes,
                keywords, is_valid, author1_ids, author1_functions, author2_ids,
                author2_functions, author3_ids, author3_functions, serie_id,
                serie_vol_number, collection_id, collection_number_sub,
                collection_vol_number, edition_id, edition_date, crea_date, modif_date
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26,
                $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38
            ) RETURNING id
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.isbn.as_ref().map(|s| sanitize_isbn(s)))
        .bind(&item.price)
        .bind(&item.barcode)
        .bind(&item.dewey)
        .bind(&item.publication_date)
        .bind(&item.lang)
        .bind(&item.lang_orig)
        .bind(&item.title1)
        .bind(&item.title2)
        .bind(&item.title3)
        .bind(&item.title4)
        .bind(&item.genre)
        .bind(&item.subject)
        .bind(&item.public_type)
        .bind(&item.nb_pages)
        .bind(&item.format)
        .bind(&item.content)
        .bind(&item.addon)
        .bind(&item.abstract_)
        .bind(&item.notes)
        .bind(&item.keywords)
        .bind(&item.is_valid)
        .bind(&author1_ids)
        .bind(&author1_functions)
        .bind(&author2_ids)
        .bind(&author2_functions)
        .bind(&author3_ids)
        .bind(&author3_functions)
        .bind(serie_id)
        .bind(&item.serie_vol_number)
        .bind(collection_id)
        .bind(&item.collection_number_sub)
        .bind(&item.collection_vol_number)
        .bind(edition_id)
        .bind(item.edition.as_ref().and_then(|e| e.date.clone()))
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        self.get_by_id_or_isbn(&id.to_string()).await
    }

    /// Process authors and return IDs and functions
    async fn process_authors(
        &self,
        authors: &Vec<AuthorWithFunction>,
    ) -> AppResult<(Option<Vec<i32>>, Option<String>)> {
        if authors.is_empty() {
            return Ok((None, None));
        }

        let mut ids = Vec::new();
        let mut functions = Vec::new();

        for author in authors {
            let id = if author.id != 0 {
                author.id
            } else if let Some(ref lastname) = author.lastname {
                // Insert or get existing author
                let existing: Option<i32> = sqlx::query_scalar(
                    "SELECT id FROM authors WHERE lastname = $1 AND firstname = $2"
                )
                .bind(lastname)
                .bind(&author.firstname)
                .fetch_optional(&self.pool)
                .await?;

                if let Some(id) = existing {
                    id
                } else {
                    sqlx::query_scalar::<_, i32>(
                        "INSERT INTO authors (lastname, firstname) VALUES ($1, $2) RETURNING id"
                    )
                    .bind(lastname)
                    .bind(&author.firstname)
                    .fetch_one(&self.pool)
                    .await?
                }
            } else {
                continue;
            };

            ids.push(id);
            functions.push(author.function.clone().unwrap_or_default());
        }

        if ids.is_empty() {
            Ok((None, None))
        } else {
            Ok((Some(ids), Some(functions.join(","))))
        }
    }

    /// Process serie and return ID
    async fn process_serie(&self, serie: &Option<Serie>) -> AppResult<Option<i32>> {
        let Some(serie) = serie else {
            return Ok(None);
        };

        if let Some(id) = serie.id {
            return Ok(Some(id));
        }

        let Some(ref name) = serie.name else {
            return Ok(None);
        };

        let key = normalize_key(name);

        // Insert or get existing (check by key first, then by name for backward compatibility)
        let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM series WHERE key = $1 OR name = $2")
            .bind(&key)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO series (key, name, issn) VALUES ($1, $2, $3) RETURNING id"
            )
            .bind(&key)
            .bind(name)
            .bind(&serie.issn)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    /// Process collection and return ID
    async fn process_collection(&self, collection: &Option<Collection>) -> AppResult<Option<i32>> {
        let Some(collection) = collection else {
            return Ok(None);
        };

        if let Some(id) = collection.id {
            return Ok(Some(id));
        }

        let Some(ref title1) = collection.title1 else {
            return Ok(None);
        };

        let key = normalize_key(title1);

        // Insert or get existing (check by key first, then by title1 for backward compatibility)
        let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM collections WHERE key = $1 OR title1 = $2")
            .bind(&key)
            .bind(title1)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO collections (key, title1, title2, title3, issn) VALUES ($1, $2, $3, $4, $5) RETURNING id"
            )
            .bind(&key)
            .bind(title1)
            .bind(&collection.title2)
            .bind(&collection.title3)
            .bind(&collection.issn)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    /// Process edition and return ID
    async fn process_edition(&self, edition: &Option<Edition>) -> AppResult<Option<i32>> {
        let Some(edition) = edition else {
            return Ok(None);
        };

        if let Some(id) = edition.id {
            if id != 0 {
                return Ok(Some(id));
            }
            return Ok(None); // 0 is not a valid FK
        }

        let Some(ref name) = edition.name else {
            return Ok(None);
        };

        // Insert or get existing
        let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM editions WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO editions (name, place) VALUES ($1, $2) RETURNING id"
            )
            .bind(name)
            .bind(&edition.place)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    /// Update an existing item
    pub async fn update(&self, id: i32, item: &Item) -> AppResult<Item> {
        let now = Utc::now();

        // Handle authors
        let (author1_ids, author1_functions) = self.process_authors(&item.authors1).await?;
        let (author2_ids, author2_functions) = self.process_authors(&item.authors2).await?;
        let (author3_ids, author3_functions) = self.process_authors(&item.authors3).await?;

        // Handle serie
        let serie_id = self.process_serie(&item.serie).await?;

        // Handle collection
        let collection_id = self.process_collection(&item.collection).await?;

        // Handle edition
        let edition_id = self.process_edition(&item.edition).await?;

        sqlx::query(
            r#"
            UPDATE items SET
                media_type = COALESCE($1, media_type),
                isbn = COALESCE($2, isbn),
                title1 = COALESCE($3, title1),
                title2 = COALESCE($4, title2),
                author1_ids = COALESCE($5, author1_ids),
                author1_functions = COALESCE($6, author1_functions),
                author2_ids = COALESCE($7, author2_ids),
                author2_functions = COALESCE($8, author2_functions),
                author3_ids = COALESCE($9, author3_ids),
                author3_functions = COALESCE($10, author3_functions),
                serie_id = $11,
                serie_vol_number = $12,
                collection_id = $13,
                collection_number_sub = $14,
                collection_vol_number = $15,
                edition_id = $16,
                modif_date = $17
            WHERE id = $18
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.isbn.as_ref().map(|s| sanitize_isbn(s)))
        .bind(&item.title1)
        .bind(&item.title2)
        .bind(&author1_ids)
        .bind(&author1_functions)
        .bind(&author2_ids)
        .bind(&author2_functions)
        .bind(&author3_ids)
        .bind(&author3_functions)
        .bind(serie_id)
        .bind(&item.serie_vol_number)
        .bind(collection_id)
        .bind(&item.collection_number_sub)
        .bind(&item.collection_vol_number)
        .bind(edition_id)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id_or_isbn(&id.to_string()).await
    }

    /// Delete an item (soft delete - sets lifecycle_status to Deleted)
    pub async fn delete(&self, id: i32, force: bool) -> AppResult<()> {
        let now = Utc::now();
        
        // Check for borrowed specimens
        let borrowed: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            WHERE s.id_item = $1 AND l.returned_date IS NULL
            "#
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if borrowed > 0 && !force {
            return Err(AppError::BusinessRule(
                "Item has borrowed specimens. Use force=true to delete anyway.".to_string()
            ));
        }

        // Soft delete specimens first
        sqlx::query(
            "UPDATE specimens SET lifecycle_status = 2, archive_date = $1, modif_date = $1 WHERE id_item = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Soft delete item
        sqlx::query(
            "UPDATE items SET lifecycle_status = 2, archived_date = $1, modif_date = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get specimens for an item (excludes deleted specimens)
    pub async fn get_specimens(&self, item_id: i32) -> AppResult<Vec<Specimen>> {
        let specimens = sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.*, so.name as source_name,
                   (SELECT COUNT(*) FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) as availability
            FROM specimens s
            LEFT JOIN sources so ON s.source_id = so.id
            WHERE s.id_item = $1 AND s.lifecycle_status != 2
            ORDER BY s.barcode
            "#,
        )
        .bind(item_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(specimens)
    }

    /// Create a specimen
    pub async fn create_specimen(&self, item_id: i32, specimen: &CreateSpecimen) -> AppResult<Specimen> {
        let now = Utc::now();

        // Handle source
        let source_id = if let Some(id) = specimen.source_id {
            Some(id)
        } else if let Some(ref name) = specimen.source_name {
            let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM sources WHERE name = $1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?;
            
            if let Some(id) = existing {
                Some(id)
            } else {
                Some(sqlx::query_scalar::<_, i32>(
                    "INSERT INTO sources (name) VALUES ($1) RETURNING id"
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?)
            }
        } else {
            None
        };

        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO specimens (
                id_item, barcode, call_number, place, status, notes, price, source_id, crea_date, modif_date
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.place)
        .bind(&specimen.status.unwrap_or(98)) // Default: borrowable
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;


        // Return enriched specimen (with source_name and availability)
        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.*, so.name as source_name,
                   (SELECT COUNT(*) FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) as availability
            FROM specimens s
            LEFT JOIN sources so ON s.source_id = so.id
            WHERE s.id = $1
            "#
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Update a specimen
    pub async fn update_specimen(&self, id: i32, specimen: &crate::models::specimen::UpdateSpecimen) -> AppResult<Specimen> {
        let now = Utc::now();

        // Handle source if provided
        let source_id = if let Some(sid) = specimen.source_id {
            Some(sid)
        } else {
            None
        };

        let lifecycle_status = specimen.lifecycle_status.map(|s| s as i16);

        sqlx::query(
            r#"
            UPDATE specimens SET
                barcode = COALESCE($1, barcode),
                call_number = COALESCE($2, call_number),
                place = COALESCE($3, place),
                status = COALESCE($4, status),
                notes = COALESCE($5, notes),
                price = COALESCE($6, price),
                source_id = COALESCE($7, source_id),
                lifecycle_status = COALESCE($8, lifecycle_status),
                modif_date = $9
            WHERE id = $10
            "#
        )
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.place)
        .bind(&specimen.status)
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(lifecycle_status)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;


        // Return enriched specimen
        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.*, so.name as source_name,
                   (SELECT COUNT(*) FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) as availability
            FROM specimens s
            LEFT JOIN sources so ON s.source_id = so.id
            WHERE s.id = $1
            "#
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Delete a specimen (soft delete - sets lifecycle_status to Deleted)
    pub async fn delete_specimen(&self, id: i32, force: bool) -> AppResult<()> {
        let now = Utc::now();
        
        // Check if borrowed
        let borrowed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE specimen_id = $1 AND returned_date IS NULL"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        if borrowed > 0 && !force {
            return Err(AppError::BusinessRule(
                "Specimen is currently borrowed. Use force=true to delete anyway.".to_string()
            ));
        }

        // Soft delete specimen
        sqlx::query(
            "UPDATE specimens SET lifecycle_status = 2, archive_date = $1, modif_date = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if specimen barcode already exists (exclude_id used on update to allow keeping same barcode)
    pub async fn specimen_barcode_exists(
        &self,
        barcode: &str,
        exclude_specimen_id: Option<i32>,
    ) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_specimen_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM specimens WHERE barcode = $1 AND id != $2)")
                .bind(barcode)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM specimens WHERE barcode = $1)")
                .bind(barcode)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists)
    }

    /// Get specimen id and lifecycle_status by barcode, if any.
    pub async fn get_specimen_by_barcode(&self, barcode: &str) -> AppResult<Option<(i32, i16)>> {
        let row: Option<(i32, i16)> = sqlx::query_as(
            "SELECT id, lifecycle_status FROM specimens WHERE barcode = $1",
        )
        .bind(barcode)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Reactivate a specimen (lifecycle_status=0, archive_date=NULL) and update its fields from create payload.
    pub async fn reactivate_specimen(
        &self,
        specimen_id: i32,
        item_id: i32,
        specimen: &CreateSpecimen,
    ) -> AppResult<Specimen> {
        let now = Utc::now();
        let source_id = if let Some(id) = specimen.source_id {
            Some(id)
        } else if let Some(ref name) = specimen.source_name {
            let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM sources WHERE name = $1")
                .bind(name)
                .fetch_optional(&self.pool)
                .await?;
            if let Some(id) = existing {
                Some(id)
            } else {
                Some(
                    sqlx::query_scalar::<_, i32>("INSERT INTO sources (name) VALUES ($1) RETURNING id")
                        .bind(name)
                        .fetch_one(&self.pool)
                        .await?,
                )
            }
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE specimens SET
                id_item = $1, barcode = $2, call_number = $3, place = $4, status = $5,
                notes = $6, price = $7, source_id = $8,
                lifecycle_status = 0, archive_date = NULL,
                modif_date = $9
            WHERE id = $10
            "#,
        )
        .bind(item_id)
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.place)
        .bind(specimen.status.unwrap_or(98))
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(now)
        .bind(specimen_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.*, so.name as source_name,
                   (SELECT COUNT(*) FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) as availability
            FROM specimens s
            LEFT JOIN sources so ON s.source_id = so.id
            WHERE s.id = $1
            "#,
        )
        .bind(specimen_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Check if ISBN already exists
    pub async fn isbn_exists(&self, isbn: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE isbn = $1 AND id != $2)")
                .bind(isbn)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE isbn = $1)")
                .bind(isbn)
                .fetch_one(&self.pool)
                .await?
        };

        Ok(exists)
    }
}


