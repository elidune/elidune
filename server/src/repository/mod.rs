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

/// Main repository struct holding database connection pool.
/// Methods are split across domain modules (items, loans, users, etc.) via separate `impl Repository` blocks.
#[derive(Clone)]
pub struct Repository {
    pub(crate) pool: Pool<Postgres>,
}

impl Repository {
    /// Create a new repository with the given database pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}
