//! Catalog management service

use crate::{
    error::AppResult,
    marc::MarcRecord,
    models::{
        item::{Item, ItemQuery, ItemShort},
        specimen::{CreateSpecimen, Specimen, UpdateSpecimen},
    },
    repository::Repository,
};

#[derive(Clone)]
pub struct CatalogService {
    repository: Repository,
}

impl CatalogService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Search items with filters
    pub async fn search_items(&self, query: &ItemQuery) -> AppResult<(Vec<ItemShort>, i64)> {
        self.repository.items.search(query).await
    }

    /// Get item by ID with full details
    pub async fn get_item(&self, id: i32) -> AppResult<Item> {
        self.repository.items.get_by_id_or_isbn(&id.to_string(), false).await
    }

    /// Get item by ID with full MARC record (marc_data)
    pub async fn get_item_with_full_record(&self, id: i32) -> AppResult<Item> {
        self.repository.items.get_by_id_or_isbn(&id.to_string(), true).await
    }

    /// Create a new item.
    /// If `allow_duplicate_isbn` is true, creation is allowed even when another item has the same ISBN.
    pub async fn create_item(&self, mut item: Item, allow_duplicate_isbn: bool) -> AppResult<Item> {
        if !allow_duplicate_isbn {
            if let Some(ref isbn) = item.isbn {
                if self
                    .repository
                    .items
                    .isbn_exists(isbn, None)
                    .await?
                {
                    return Err(crate::error::AppError::Conflict(
                        "Item with this ISBN already exists".to_string(),
                    ));
                }
            }
        }

        let record = MarcRecord::from(&item);
        item.marc_record = serde_json::to_value(&record).ok();
        self.repository.items.create(&item).await
    }

    /// Update an existing item
    pub async fn update_item(&self, id: i32, mut item: Item) -> AppResult<Item> {
        // Check if item exists
        self.repository.items.get_by_id_or_isbn(&id.to_string(), false).await?;

        // Check for duplicate ISBN
        if let Some(ref isbn) = item.isbn {
            if self
                .repository
                .items
                .isbn_exists(isbn, Some(id))
                .await?
            {
                return Err(crate::error::AppError::Conflict(
                    "Item with this ISBN already exists".to_string(),
                ));
            }
        }

        let record = MarcRecord::from(&item);
        item.marc_record = serde_json::to_value(&record).ok();
        self.repository.items.update(id, &item).await
    }

    /// Delete an item
    pub async fn delete_item(&self, id: i32, force: bool) -> AppResult<()> {
        self.repository.items.delete(id, force).await
    }

    /// Get specimens for an item
    pub async fn get_specimens(&self, item_id: i32) -> AppResult<Vec<Specimen>> {
        // Verify item exists
        self.repository.items.get_by_id_or_isbn(&item_id.to_string(), false).await?;
        self.repository.items.get_specimens(item_id).await
    }

    /// Create a specimen for an item.
    /// Barcode must be unique among active specimens.
    /// If barcode exists on an archived specimen, it is reactivated and updated.
    pub async fn create_specimen(&self, item_id: i32, specimen: CreateSpecimen) -> AppResult<Specimen> {
        self.repository.items.get_by_id_or_isbn(&item_id.to_string(), false).await?;
        if let Some(ref barcode) = specimen.barcode {
            if let Some((existing_id, is_archived)) = self
                .repository
                .items
                .get_specimen_by_barcode(barcode)
                .await?
            {
                if is_archived {
                    return self
                        .repository
                        .items
                        .reactivate_specimen(existing_id, item_id, &specimen)
                        .await;
                }
                return Err(crate::error::AppError::Conflict(
                    "A specimen with this barcode already exists".to_string(),
                ));
            }
        }
        self.repository.items.create_specimen(item_id, &specimen).await
    }

    /// Update a specimen
    pub async fn update_specimen(&self, item_id: i32, specimen_id: i32, specimen: UpdateSpecimen) -> AppResult<Specimen> {
        // Verify item exists
        self.repository.items.get_by_id_or_isbn(&item_id.to_string(), false).await?;
        // Verify specimen belongs to item
        let specimens = self.repository.items.get_specimens(item_id).await?;
        if !specimens.iter().any(|s| s.id == specimen_id) {
            return Err(crate::error::AppError::NotFound(
                format!("Specimen {} not found for item {}", specimen_id, item_id)
            ));
        }
        // Enforce barcode uniqueness when changing barcode
        if let Some(ref barcode) = specimen.barcode {
            if self
                .repository
                .items
                .specimen_barcode_exists(barcode, Some(specimen_id))
                .await?
            {
                return Err(crate::error::AppError::Conflict(
                    "A specimen with this barcode already exists".to_string(),
                ));
            }
        }
        self.repository.items.update_specimen(specimen_id, &specimen).await
    }

    /// Delete a specimen
    pub async fn delete_specimen(&self, _item_id: i32, specimen_id: i32, force: bool) -> AppResult<()> {
        self.repository.items.delete_specimen(specimen_id, force).await
    }

    /// List all items in a series (ordered by volume number)
    pub async fn get_items_by_series(&self, series_id: i32) -> AppResult<Vec<ItemShort>> {
        self.repository.items.get_items_by_series(series_id).await
    }
}


