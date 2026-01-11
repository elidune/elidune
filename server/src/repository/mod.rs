//! Repository layer for database operations

pub mod items;
pub mod loans;
pub mod users;

use sqlx::{Pool, Postgres};

/// Main repository struct holding database connection pool
#[derive(Clone)]
pub struct Repository {
    pub pool: Pool<Postgres>,
    pub items: items::ItemsRepository,
    pub users: users::UsersRepository,
    pub loans: loans::LoansRepository,
}

impl Repository {
    /// Create a new repository with the given database pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            items: items::ItemsRepository::new(pool.clone()),
            users: users::UsersRepository::new(pool.clone()),
            loans: loans::LoansRepository::new(pool.clone()),
            pool,
        }
    }
}


