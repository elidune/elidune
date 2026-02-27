//! Items repository for database operations.
//!
//! Uses marc-rs types (Leader, MarcFormat, etc.) where applicable; DB serialization
//! uses the associated char or int (e.g. media_type string from Leader record_type).

use chrono::Utc;
use sqlx::{Pool, Postgres, Row};
use z3950_rs::marc_rs::MarcFormat;

use crate::{
    error::{AppError, AppResult},
    models::{
        author::AuthorWithFunction,
        item::{Collection, Edition, Item, ItemQuery, ItemShort, Serie},
        specimen::{CreateSpecimen, Specimen},
    },
};

// --- MARC type → DB (char/int) conversion helpers ---

/// Converts record type char (Leader position 6) to DB media_type string.
pub fn record_type_to_media_type_db(record_type: char) -> String {
        match record_type {
            'a' | 't' => "b",
            'c' | 'd' => "bc",
            'g' => "v",
            'i' | 'j' => "a",
            'm' => "c",
            'k' => "i",
            _ => "u",
        }
    
    .to_string()
}

fn sanitize_isbn(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
}

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

    // =========================================================================
    // READ
    // =========================================================================

    /// Get item by numeric ID or by ISBN.
    pub async fn get_by_id_or_isbn(&self, id_or_isbn: &str, with_marc_record: bool) -> AppResult<Item> {



        let query = if with_marc_record {
            r#"
            SELECT id, marc_record, media_type, isbn, price, barcode, call_number,
                   publication_date, lang, lang_orig, title,
                   genre, subject, audience_type, page_extent, format,
                   table_of_contents, accompanying_material,
                   abstract as abstract_, notes, keywords, state,
                   series_id, series_volume_number, edition_id,
                   collection_id, collection_sequence_number, collection_volume_number,
                   is_valid, status,
                   created_at, updated_at, archived_at
            FROM items
            WHERE (id = $1 OR isbn = $2) AND archived_at IS NULL
            "#
            } else {
                r#"
            SELECT id, media_type, isbn, price, barcode, call_number,
                   publication_date, lang, lang_orig, title,
                   genre, subject, audience_type, page_extent, format,
                   table_of_contents, accompanying_material,
                   abstract as abstract_, notes, keywords, state,
                   series_id, series_volume_number, edition_id,
                   collection_id, collection_sequence_number, collection_volume_number,
                   is_valid, status,
                   created_at, updated_at, archived_at
            FROM items
            WHERE (id = $1 OR isbn = $2) AND archived_at IS NULL
            "#
            };
        // query id and isbn in the same query
        let mut item = sqlx::query_as::<_, Item>(query)
        .bind(id_or_isbn.parse::<i32>().unwrap_or(0))
        .bind(id_or_isbn)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id_or_isbn)))?;

 

        let id = item.id.ok_or_else(|| AppError::Internal("Item id is null".to_string()))?;

        item.authors = self.get_item_authors(id).await?;

        item.series = sqlx::query_as::<_, Serie>(
            "SELECT id, key, name, issn, created_at, updated_at FROM series WHERE id = $1",
        )
        .bind(item.series_id)
        .fetch_optional(&self.pool)
        .await?;

        item.collection = sqlx::query_as::<_, Collection>(
            "SELECT id, key, primary_title, secondary_title, tertiary_title, issn, created_at, updated_at FROM collections WHERE id = $1",
        )
        .bind(item.collection_id)
        .fetch_optional(&self.pool)
        .await?;

        item.edition = sqlx::query_as::<_, Edition>(
            "SELECT id, publisher_name, place_of_publication, date, created_at, updated_at FROM editions WHERE id = $1",
        )
        .bind(item.edition_id)
        .fetch_optional(&self.pool)
        .await?;

        item.specimens = self.get_specimens(id).await?;

        Ok(item)
    }



    /// Load all authors for an item via the item_authors junction table
    async fn get_item_authors(&self, item_id: i32) -> AppResult<Vec<AuthorWithFunction>> {
        let rows = sqlx::query(
            r#"
            SELECT a.id, a.lastname, a.firstname, a.bio, a.notes, ia.role as function
            FROM item_authors ia
            JOIN authors a ON a.id = ia.author_id
            WHERE ia.item_id = $1
            ORDER BY ia.position
            "#,
        )
        .bind(item_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| AuthorWithFunction {
                id: r.get("id"),
                lastname: r.get("lastname"),
                firstname: r.get("firstname"),
                bio: r.get("bio"),
                notes: r.get("notes"),
                function: r.get("function"),
            })
            .collect())
    }

    // =========================================================================
    // SEARCH
    // =========================================================================

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
                "LOWER(title) LIKE '%{}%'",
                title.to_lowercase()
            ));
        }

        if let Some(ref keywords) = query.keywords {
            conditions.push(format!("LOWER(keywords) LIKE '%{}%'", keywords.to_lowercase()));
        }

        if let Some(ref freesearch) = query.freesearch {
            let term = freesearch.to_lowercase();
            conditions.push(format!(
                "(LOWER(title) LIKE '%{t}%' OR LOWER(isbn) LIKE '%{t}%' OR LOWER(subject) LIKE '%{t}%' \
                 OR LOWER(keywords) LIKE '%{t}%' OR LOWER(call_number) LIKE '%{t}%' \
                 OR EXISTS (SELECT 1 FROM item_authors ia JOIN authors a ON a.id = ia.author_id \
                            WHERE ia.item_id = i.id AND (LOWER(a.lastname) LIKE '%{t}%' OR LOWER(a.firstname) LIKE '%{t}%')))",
                t = term
            ));
        }

        if let Some(archive) = query.archive {
            if archive {
                conditions.push("archived_at IS NOT NULL".to_string());
            } else {
                conditions.push("archived_at IS NULL".to_string());
            }
        } else {
            conditions.push("archived_at IS NULL".to_string());
        }

        let where_clause = conditions.join(" AND ");

        let count_query = format!("SELECT COUNT(*) FROM items i WHERE {}", where_clause);
        let total: i64 = sqlx::query_scalar(&count_query)
            .fetch_one(&self.pool)
            .await?;

        let select_query = format!(
            r#"
            SELECT i.id, i.media_type, i.isbn, i.title,
                   i.publication_date as date, 0::smallint as status,
                   1::smallint as is_local, i.is_valid, i.archived_at,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s
                       WHERE s.item_id = i.id
                         AND s.archived_at IS NULL
                   ), 0::smallint)::smallint as nb_specimens,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT)
                       FROM specimens s
                       WHERE s.item_id = i.id
                         AND s.archived_at IS NULL
                         AND NOT EXISTS (
                             SELECT 1 FROM loans l
                             WHERE l.specimen_id = s.id
                               AND l.returned_date IS NULL
                         )
                   ), 0::smallint)::smallint as nb_available
            FROM items i
            WHERE {}
            ORDER BY i.title
            LIMIT {} OFFSET {}
            "#,
            where_clause, per_page, offset
        );

        let items = sqlx::query_as::<_, ItemShort>(&select_query)
            .fetch_all(&self.pool)
            .await?;

        Ok((items, total))
    }

    /// List all items belonging to a series
    pub async fn get_items_by_series(&self, series_id: i32) -> AppResult<Vec<ItemShort>> {
        let items = sqlx::query_as::<_, ItemShort>(
            r#"
            SELECT i.id, i.media_type, i.isbn, i.title,
                   i.publication_date as date, 0::smallint as status,
                   1::smallint as is_local, i.is_valid, i.archived_at,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT) FROM specimens s
                       WHERE s.item_id = i.id AND s.archived_at IS NULL
                   ), 0::smallint)::smallint as nb_specimens,
                   COALESCE((
                       SELECT CAST(COUNT(*) AS SMALLINT) FROM specimens s
                       WHERE s.item_id = i.id AND s.archived_at IS NULL
                         AND NOT EXISTS (SELECT 1 FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL)
                   ), 0::smallint)::smallint as nb_available
            FROM items i
            WHERE i.series_id = $1 AND i.archived_at IS NULL
            ORDER BY i.series_volume_number, i.title
            "#,
        )
        .bind(series_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    // =========================================================================
    // CREATE
    // =========================================================================

    /// Create a new item
    pub async fn create(&self, item: &Item) -> AppResult<Item> {
        let now = Utc::now();

        let series_id = self.process_serie(&item.series).await?;
        let collection_id = self.process_collection(&item.collection).await?;
        let edition_id = self.process_edition(&item.edition).await?;

        let id = sqlx::query_scalar::<_, i32>(
            r#"
            INSERT INTO items (
                media_type, isbn, price, barcode, call_number, publication_date,
                lang, lang_orig, title, genre, subject,
                audience_type, page_extent, format, table_of_contents, accompanying_material,
                abstract, notes, keywords, is_valid,
                series_id, series_volume_number,
                collection_id, collection_sequence_number, collection_volume_number,
                edition_id, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21,
                $22, $23, $24, $25, $26, $27, $28, $29
            ) RETURNING id
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.isbn.as_ref().map(|s| sanitize_isbn(s)))
        .bind(&item.price)
        .bind(&item.barcode)
        .bind(&item.call_number)
        .bind(&item.publication_date)
        .bind(&item.lang)
        .bind(&item.lang_orig)
        .bind(&item.title)
        .bind(&item.genre)
        .bind(&item.subject)
        .bind(&item.audience_type)
        .bind(&item.page_extent)
        .bind(&item.format)
        .bind(&item.table_of_contents)
        .bind(&item.accompanying_material)
        .bind(&item.abstract_)
        .bind(&item.notes)
        .bind(&item.keywords)
        .bind(&item.is_valid)
        .bind(series_id)
        .bind(&item.series_volume_number)
        .bind(collection_id)
        .bind(&item.collection_sequence_number)
        .bind(&item.collection_volume_number)
        .bind(edition_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        self.sync_item_authors(id, &item.authors).await?;

        if let Some(ref marc_record) = item.marc_record {
            sqlx::query("UPDATE items SET marc_record = $1 WHERE id = $2")
                .bind(marc_record)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }

        self.get_by_id_or_isbn(&id.to_string(), false).await
    }

    // =========================================================================
    // UPDATE
    // =========================================================================

    /// Update an existing item
    pub async fn update(&self, id: i32, item: &Item) -> AppResult<Item> {
        let now = Utc::now();

        let series_id = self.process_serie(&item.series).await?;
        let collection_id = self.process_collection(&item.collection).await?;
        let edition_id = self.process_edition(&item.edition).await?;

        sqlx::query(
            r#"
            UPDATE items SET
                media_type = COALESCE($1::text, media_type),
                isbn = COALESCE($2::text, isbn),
                title = COALESCE($3::text, title),
                series_id = $4,
                series_volume_number = $5,
                collection_id = $6,
                collection_sequence_number = $7,
                collection_volume_number = $8,
                edition_id = $9,
                updated_at = $10
            WHERE id = $11
            "#,
        )
        .bind(item.media_type.as_deref())
        .bind(item.isbn.as_ref().map(|s| sanitize_isbn(s)))
        .bind(item.title.as_deref())
        .bind(series_id)
        .bind(item.series_volume_number)
        .bind(collection_id)
        .bind(item.collection_sequence_number)
        .bind(item.collection_volume_number)
        .bind(edition_id)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if !item.authors.is_empty() {
            self.sync_item_authors(id, &item.authors).await?;
        }

        if let Some(ref marc_record) = item.marc_record {
            sqlx::query("UPDATE items SET marc_record = $1 WHERE id = $2")
                .bind(marc_record)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }

        self.get_by_id_or_isbn(&id.to_string(), false).await
    }

    /// Save marc_record JSONB for an item
    pub async fn save_marc_record(&self, item_id: i32, marc_record: &serde_json::Value) -> AppResult<()> {
        sqlx::query("UPDATE items SET marc_record = $1 WHERE id = $2")
            .bind(marc_record)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // =========================================================================
    // DELETE (archive)
    // =========================================================================

    /// Delete an item (soft delete — sets archived_at)
    pub async fn delete(&self, id: i32, force: bool) -> AppResult<()> {
        let now = Utc::now();

        let borrowed: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            WHERE s.item_id = $1 AND l.returned_date IS NULL
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

        sqlx::query(
            "UPDATE specimens SET archived_at = $1, updated_at = $1 WHERE item_id = $2 AND archived_at IS NULL"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "UPDATE items SET archived_at = $1, updated_at = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // AUTHORS (item_authors junction)
    // =========================================================================

    /// Replace all authors for an item: delete existing rows then insert new ones.
    async fn sync_item_authors(
        &self,
        item_id: i32,
        authors: &[AuthorWithFunction],
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM item_authors WHERE item_id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await?;

        for (idx, author) in authors.iter().enumerate() {
            let author_id = self.ensure_author(author).await?;
            let Some(author_id) = author_id else { continue };

            sqlx::query(
                r#"
                INSERT INTO item_authors (item_id, author_id, role, author_type, position)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (item_id, author_id, role) DO UPDATE SET position = $5
                "#,
            )
            .bind(item_id)
            .bind(author_id)
            .bind(&author.function)
            .bind(0i16) // personal by default
            .bind((idx + 1) as i16)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Insert author if new, or return existing id.
    async fn ensure_author(&self, author: &AuthorWithFunction) -> AppResult<Option<i32>> {
        if author.id != 0 {
            return Ok(Some(author.id));
        }

        let Some(ref lastname) = author.lastname else {
            return Ok(None);
        };

        let existing: Option<i32> = sqlx::query_scalar(
            "SELECT id FROM authors WHERE lastname = $1 AND firstname IS NOT DISTINCT FROM $2",
        )
        .bind(lastname)
        .bind(&author.firstname)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO authors (lastname, firstname) VALUES ($1, $2) RETURNING id",
            )
            .bind(lastname)
            .bind(&author.firstname)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    // =========================================================================
    // SERIES / COLLECTIONS / EDITIONS
    // =========================================================================

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

    async fn process_collection(&self, collection: &Option<Collection>) -> AppResult<Option<i32>> {
        let Some(collection) = collection else {
            return Ok(None);
        };

        if let Some(id) = collection.id {
            return Ok(Some(id));
        }

        let Some(ref primary_title) = collection.primary_title else {
            return Ok(None);
        };

        let key = normalize_key(primary_title);

        let existing: Option<i32> = sqlx::query_scalar(
            "SELECT id FROM collections WHERE key = $1 OR primary_title = $2",
        )
        .bind(&key)
        .bind(primary_title)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO collections (key, primary_title, secondary_title, tertiary_title, issn) VALUES ($1, $2, $3, $4, $5) RETURNING id",
            )
            .bind(&key)
            .bind(primary_title)
            .bind(&collection.secondary_title)
            .bind(&collection.tertiary_title)
            .bind(&collection.issn)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    async fn process_edition(&self, edition: &Option<Edition>) -> AppResult<Option<i32>> {
        let Some(edition) = edition else {
            return Ok(None);
        };

        if let Some(id) = edition.id {
            if id != 0 {
                return Ok(Some(id));
            }
            return Ok(None);
        }

        let Some(ref publisher_name) = edition.publisher_name else {
            return Ok(None);
        };

        let existing: Option<i32> = sqlx::query_scalar(
            "SELECT id FROM editions WHERE publisher_name = $1",
        )
        .bind(publisher_name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO editions (publisher_name, place_of_publication, date) VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(publisher_name)
            .bind(&edition.place_of_publication)
            .bind(&edition.date)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    // =========================================================================
    // SPECIMENS
    // =========================================================================

    /// Get specimens for an item (excludes archived specimens)
    pub async fn get_specimens(&self, item_id: i32) -> AppResult<Vec<Specimen>> {
        let specimens = sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.id, s.item_id, s.source_id, s.barcode, s.call_number, s.volume_designation,
                   s.place, s.borrow_status, s.circulation_status, s.notes, s.price,
                   s.created_at, s.updated_at, s.archived_at,
                   so.name as source_name,
                   (SELECT COUNT(*) FROM loans l WHERE l.specimen_id = s.id AND l.returned_date IS NULL) as availability
            FROM specimens s
            LEFT JOIN sources so ON s.source_id = so.id
            WHERE s.item_id = $1 AND s.archived_at IS NULL
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
                item_id, barcode, call_number, volume_designation, place, borrow_status, notes, price, source_id, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.volume_designation)
        .bind(&specimen.place)
        .bind(&specimen.borrow_status.unwrap_or(98))
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.id, s.item_id, s.source_id, s.barcode, s.call_number, s.volume_designation,
                   s.place, s.borrow_status, s.circulation_status, s.notes, s.price,
                   s.created_at, s.updated_at, s.archived_at,
                   so.name as source_name,
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

        sqlx::query(
            r#"
            UPDATE specimens SET
                barcode = COALESCE($1, barcode),
                call_number = COALESCE($2, call_number),
                volume_designation = COALESCE($3, volume_designation),
                place = COALESCE($4, place),
                borrow_status = COALESCE($5, borrow_status),
                notes = COALESCE($6, notes),
                price = COALESCE($7, price),
                source_id = COALESCE($8, source_id),
                updated_at = $9
            WHERE id = $10
            "#
        )
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.volume_designation)
        .bind(&specimen.place)
        .bind(&specimen.borrow_status)
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(&specimen.source_id)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.id, s.item_id, s.source_id, s.barcode, s.call_number, s.volume_designation,
                   s.place, s.borrow_status, s.circulation_status, s.notes, s.price,
                   s.created_at, s.updated_at, s.archived_at,
                   so.name as source_name,
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

    /// Delete a specimen (soft delete — sets archived_at)
    pub async fn delete_specimen(&self, id: i32, force: bool) -> AppResult<()> {
        let now = Utc::now();

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

        sqlx::query(
            "UPDATE specimens SET archived_at = $1, updated_at = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if specimen barcode already exists
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

    /// Get specimen id and archived_at by barcode
    pub async fn get_specimen_by_barcode(&self, barcode: &str) -> AppResult<Option<(i32, bool)>> {
        let row: Option<(i32, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
            "SELECT id, archived_at FROM specimens WHERE barcode = $1",
        )
        .bind(barcode)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id, archived_at)| (id, archived_at.is_some())))
    }

    /// Reactivate an archived specimen and update its fields.
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
                item_id = $1, barcode = $2, call_number = $3, volume_designation = $4,
                place = $5, borrow_status = $6,
                notes = $7, price = $8, source_id = $9,
                archived_at = NULL,
                updated_at = $10
            WHERE id = $11
            "#,
        )
        .bind(item_id)
        .bind(&specimen.barcode)
        .bind(&specimen.call_number)
        .bind(&specimen.volume_designation)
        .bind(&specimen.place)
        .bind(specimen.borrow_status.unwrap_or(98))
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(now)
        .bind(specimen_id)
        .execute(&self.pool)
        .await?;

        sqlx::query_as::<_, Specimen>(
            r#"
            SELECT s.id, s.item_id, s.source_id, s.barcode, s.call_number, s.volume_designation,
                   s.place, s.borrow_status, s.circulation_status, s.notes, s.price,
                   s.created_at, s.updated_at, s.archived_at,
                   so.name as source_name,
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
