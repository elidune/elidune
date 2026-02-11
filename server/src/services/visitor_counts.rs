//! Visitor counts service

use chrono::NaiveDate;

use crate::{
    error::AppResult,
    models::visitor_count::{CreateVisitorCount, VisitorCount},
    repository::Repository,
};

#[derive(Clone)]
pub struct VisitorCountsService {
    repository: Repository,
}

impl VisitorCountsService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// List visitor counts for a date range
    pub async fn list(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> AppResult<Vec<VisitorCount>> {
        self.repository.visitor_counts.list(start_date, end_date).await
    }

    /// Get total visitor count for a date range
    pub async fn total(&self, start_date: NaiveDate, end_date: NaiveDate) -> AppResult<i64> {
        self.repository.visitor_counts.total(start_date, end_date).await
    }

    /// Create a visitor count record
    pub async fn create(&self, data: &CreateVisitorCount) -> AppResult<VisitorCount> {
        self.repository.visitor_counts.create(data).await
    }

    /// Delete a visitor count record
    pub async fn delete(&self, id: i32) -> AppResult<()> {
        self.repository.visitor_counts.delete(id).await
    }
}
