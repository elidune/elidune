//! Catalog management service

use crate::{
    error::AppResult,
    models::{
        item::{Item, ItemQuery, ItemShort},
        specimen::{CreateSpecimen, Specimen},
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
        self.repository.items.get_by_id(id).await
    }

    /// Create a new item
    pub async fn create_item(&self, item: Item) -> AppResult<Item> {
        // Check for duplicate identification
        if let Some(ref identification) = item.identification {
            if self
                .repository
                .items
                .identification_exists(identification, None)
                .await?
            {
                return Err(crate::error::AppError::Conflict(
                    "Item with this identification already exists".to_string(),
                ));
            }
        }

        self.repository.items.create(&item).await
    }

    /// Update an existing item
    pub async fn update_item(&self, id: i32, item: Item) -> AppResult<Item> {
        // Check if item exists
        self.repository.items.get_by_id(id).await?;

        // Check for duplicate identification
        if let Some(ref identification) = item.identification {
            if self
                .repository
                .items
                .identification_exists(identification, Some(id))
                .await?
            {
                return Err(crate::error::AppError::Conflict(
                    "Item with this identification already exists".to_string(),
                ));
            }
        }

        self.repository.items.update(id, &item).await
    }

    /// Delete an item
    pub async fn delete_item(&self, id: i32, force: bool) -> AppResult<()> {
        self.repository.items.delete(id, force).await
    }

    /// Get specimens for an item
    pub async fn get_specimens(&self, item_id: i32) -> AppResult<Vec<Specimen>> {
        // Verify item exists
        self.repository.items.get_by_id(item_id).await?;
        self.repository.items.get_specimens(item_id).await
    }

    /// Create a specimen for an item
    pub async fn create_specimen(&self, item_id: i32, specimen: CreateSpecimen) -> AppResult<Specimen> {
        // Verify item exists
        self.repository.items.get_by_id(item_id).await?;
        self.repository.items.create_specimen(item_id, &specimen).await
    }

    /// Delete a specimen
    pub async fn delete_specimen(&self, id: i32, force: bool) -> AppResult<()> {
        self.repository.items.delete_specimen(id, force).await
    }
}


