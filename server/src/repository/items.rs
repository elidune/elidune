//! Items repository for database operations

use chrono::Utc;
use sqlx::{Pool, Postgres, Row};

use crate::{
    error::{AppError, AppResult},
    models::{
        author::AuthorWithFunction,
        item::{Collection, CreateItem, Edition, Item, ItemQuery, ItemShort, Serie, UpdateItem},
        specimen::{CreateSpecimen, Specimen},
    },
};

#[derive(Clone)]
pub struct ItemsRepository {
    pool: Pool<Postgres>,
}

impl ItemsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    /// Get item by ID with all related data
    pub async fn get_by_id(&self, id: i32) -> AppResult<Item> {
        let mut item = sqlx::query_as::<_, Item>(
            r#"
            SELECT id, media_type, identification, price, barcode, dewey,
                   publication_date, lang, lang_orig, title1, title2, title3, title4,
                   genre, subject, public_type, nb_pages, format, content, addon,
                   abstract as abstract_, notes, keywords, nb_specimens, state,
                   is_archive, is_valid, lifecycle_status, crea_date, modif_date, archived_date
            FROM items
            WHERE id = $1 AND lifecycle_status != 2
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Item with id {} not found", id)))?;

        // Load authors
        item.authors1 = self.get_item_authors(id, "author1_ids", "author1_functions").await?;
        item.authors2 = self.get_item_authors(id, "author2_ids", "author2_functions").await?;
        item.authors3 = self.get_item_authors(id, "author3_ids", "author3_functions").await?;

        // Load serie
        let serie_id: Option<i32> = sqlx::query_scalar("SELECT serie_id FROM items WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        if let Some(sid) = serie_id {
            item.serie = sqlx::query_as::<_, Serie>("SELECT * FROM series WHERE id = $1")
                .bind(sid)
                .fetch_optional(&self.pool)
                .await?;
        }

        // Load collection
        let collection_id: Option<i32> = sqlx::query_scalar("SELECT collection_id FROM items WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        if let Some(cid) = collection_id {
            item.collection = sqlx::query_as::<_, Collection>("SELECT * FROM collections WHERE id = $1")
                .bind(cid)
                .fetch_optional(&self.pool)
                .await?;
        }

        // Load edition
        let edition_id: Option<i32> = sqlx::query_scalar("SELECT edition_id FROM items WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        if let Some(eid) = edition_id {
            let edition_row = sqlx::query(
                "SELECT e.*, i.edition_date FROM editions e 
                 JOIN items i ON i.edition_id = e.id 
                 WHERE e.id = $1"
            )
            .bind(eid)
            .fetch_optional(&self.pool)
            .await?;
            
            if let Some(row) = edition_row {
                item.edition = Some(Edition {
                    id: row.get("id"),
                    name: row.get("name"),
                    place: row.get("place"),
                    date: row.get("edition_date"),
                });
            }
        }

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

        if let Some(ref identification) = query.identification {
            conditions.push(format!("identification = '{}'", identification));
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
            SELECT id, media_type, identification, title1 as title, 
                   publication_date as date, 0::smallint as status,
                   1::smallint as is_local, is_archive, is_valid
            FROM items
            WHERE {}
            ORDER BY title1
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
    pub async fn create(&self, item: &CreateItem) -> AppResult<Item> {
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
                media_type, identification, price, barcode, dewey, publication_date,
                lang, lang_orig, title1, title2, title3, title4, genre, subject,
                public_type, nb_pages, format, content, addon, abstract, notes,
                keywords, is_valid, author1_ids, author1_functions, author2_ids,
                author2_functions, author3_ids, author3_functions, serie_id,
                collection_id, edition_id, edition_date, crea_date, modif_date
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26,
                $27, $28, $29, $30, $31, $32, $33, $34, $34
            ) RETURNING id
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.identification)
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
        .bind(collection_id)
        .bind(edition_id)
        .bind(item.edition.as_ref().and_then(|e| e.date.clone()))
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        // Create specimens if provided
        if let Some(ref specimens) = item.specimens {
            for specimen in specimens {
                self.create_specimen(id, specimen).await?;
            }
        }

        self.get_by_id(id).await
    }

    /// Process authors and return IDs and functions
    async fn process_authors(
        &self,
        authors: &Option<Vec<crate::models::item::CreateItemAuthor>>,
    ) -> AppResult<(Option<Vec<i32>>, Option<String>)> {
        let Some(authors) = authors else {
            return Ok((None, None));
        };

        let mut ids = Vec::new();
        let mut functions = Vec::new();

        for author in authors {
            let id = if let Some(id) = author.id {
                id
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
    async fn process_serie(&self, serie: &Option<crate::models::item::CreateSerie>) -> AppResult<Option<i32>> {
        let Some(serie) = serie else {
            return Ok(None);
        };

        if let Some(id) = serie.id {
            return Ok(Some(id));
        }

        let Some(ref name) = serie.name else {
            return Ok(None);
        };

        // Insert or get existing
        let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM series WHERE name = $1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO series (name) VALUES ($1) RETURNING id"
            )
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
            Ok(Some(id))
        }
    }

    /// Process collection and return ID
    async fn process_collection(&self, collection: &Option<crate::models::item::CreateCollection>) -> AppResult<Option<i32>> {
        let Some(collection) = collection else {
            return Ok(None);
        };

        if let Some(id) = collection.id {
            return Ok(Some(id));
        }

        let Some(ref title1) = collection.title1 else {
            return Ok(None);
        };

        // Insert or get existing
        let existing: Option<i32> = sqlx::query_scalar("SELECT id FROM collections WHERE title1 = $1")
            .bind(title1)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(id) = existing {
            Ok(Some(id))
        } else {
            let id = sqlx::query_scalar::<_, i32>(
                "INSERT INTO collections (title1, title2, title3, issn) VALUES ($1, $2, $3, $4) RETURNING id"
            )
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
    async fn process_edition(&self, edition: &Option<crate::models::item::CreateEdition>) -> AppResult<Option<i32>> {
        let Some(edition) = edition else {
            return Ok(None);
        };

        if let Some(id) = edition.id {
            return Ok(Some(id));
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
    pub async fn update(&self, id: i32, item: &UpdateItem) -> AppResult<Item> {
        let now = Utc::now();

        // Simple update for now - can be expanded with dynamic query building
        sqlx::query(
            r#"
            UPDATE items SET
                media_type = COALESCE($1, media_type),
                identification = COALESCE($2, identification),
                title1 = COALESCE($3, title1),
                title2 = COALESCE($4, title2),
                modif_date = $5
            WHERE id = $6
            "#,
        )
        .bind(&item.media_type)
        .bind(&item.identification)
        .bind(&item.title1)
        .bind(&item.title2)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await
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
            ORDER BY s.identification
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
                id_item, identification, cote, place, status, notes, price, source_id, crea_date, modif_date
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            RETURNING id
            "#,
        )
        .bind(item_id)
        .bind(&specimen.identification)
        .bind(&specimen.cote)
        .bind(&specimen.place)
        .bind(&specimen.status.unwrap_or(98)) // Default: borrowable
        .bind(&specimen.notes)
        .bind(&specimen.price)
        .bind(source_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        // Update specimen count
        sqlx::query("UPDATE items SET nb_specimens = (SELECT COUNT(*) FROM specimens WHERE id_item = $1) WHERE id = $1")
            .bind(item_id)
            .execute(&self.pool)
            .await?;

        sqlx::query_as::<_, Specimen>("SELECT * FROM specimens WHERE id = $1")
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

        // Get item_id before soft deletion
        let item_id: Option<i32> = sqlx::query_scalar("SELECT id_item FROM specimens WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        // Soft delete specimen
        sqlx::query(
            "UPDATE specimens SET lifecycle_status = 2, archive_date = $1, modif_date = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Update specimen count (only count active specimens)
        if let Some(item_id) = item_id {
            sqlx::query(
                "UPDATE items SET nb_specimens = (SELECT COUNT(*) FROM specimens WHERE id_item = $1 AND lifecycle_status != 2) WHERE id = $1"
            )
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// Check if identification already exists
    pub async fn identification_exists(&self, identification: &str, exclude_id: Option<i32>) -> AppResult<bool> {
        let exists: bool = if let Some(id) = exclude_id {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE identification = $1 AND id != $2)")
                .bind(identification)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM items WHERE identification = $1)")
                .bind(identification)
                .fetch_one(&self.pool)
                .await?
        };

        Ok(exists)
    }
}


