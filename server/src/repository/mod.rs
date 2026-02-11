//! Repository layer for database operations

pub mod equipment;
pub mod events;
pub mod items;
pub mod loans;
pub mod schedules;
pub mod sources;
pub mod users;
pub mod visitor_counts;

use sqlx::{Pool, Postgres};

/// Main repository struct holding database connection pool
#[derive(Clone)]
pub struct Repository {
    pub pool: Pool<Postgres>,
    pub items: items::ItemsRepository,
    pub users: users::UsersRepository,
    pub loans: loans::LoansRepository,
    pub visitor_counts: visitor_counts::VisitorCountsRepository,
    pub schedules: schedules::SchedulesRepository,
    pub sources: sources::SourcesRepository,
    pub equipment: equipment::EquipmentRepository,
    pub events: events::EventsRepository,
}

impl Repository {
    /// Create a new repository with the given database pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            items: items::ItemsRepository::new(pool.clone()),
            users: users::UsersRepository::new(pool.clone()),
            loans: loans::LoansRepository::new(pool.clone()),
            visitor_counts: visitor_counts::VisitorCountsRepository::new(pool.clone()),
            schedules: schedules::SchedulesRepository::new(pool.clone()),
            sources: sources::SourcesRepository::new(pool.clone()),
            equipment: equipment::EquipmentRepository::new(pool.clone()),
            events: events::EventsRepository::new(pool.clone()),
            pool,
        }
    }
}


