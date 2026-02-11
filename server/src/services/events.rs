//! Events service

use crate::{
    error::AppResult,
    models::event::{CreateEvent, Event, EventQuery, UpdateEvent},
    repository::{events::EventAnnualStats, Repository},
};

#[derive(Clone)]
pub struct EventsService {
    repository: Repository,
}

impl EventsService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    pub async fn list(&self, query: &EventQuery) -> AppResult<(Vec<Event>, i64)> {
        self.repository.events.list(query).await
    }

    pub async fn get_by_id(&self, id: i32) -> AppResult<Event> {
        self.repository.events.get_by_id(id).await
    }

    pub async fn create(&self, data: &CreateEvent) -> AppResult<Event> {
        self.repository.events.create(data).await
    }

    pub async fn update(&self, id: i32, data: &UpdateEvent) -> AppResult<Event> {
        self.repository.events.update(id, data).await
    }

    pub async fn delete(&self, id: i32) -> AppResult<()> {
        self.repository.events.delete(id).await
    }

    /// Get annual event statistics (for annual report)
    pub async fn annual_stats(&self, year: i32) -> AppResult<EventAnnualStats> {
        self.repository.events.annual_stats(year).await
    }
}
