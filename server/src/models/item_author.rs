//! Item-Author junction model (N:M relationship)

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Author type in MARC context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[repr(i16)]
pub enum AuthorType {
    Personal = 0,
    Corporate = 1,
    Meeting = 2,
}

impl From<i16> for AuthorType {
    fn from(v: i16) -> Self {
        match v {
            1 => AuthorType::Corporate,
            2 => AuthorType::Meeting,
            _ => AuthorType::Personal,
        }
    }
}

/// Junction row linking an item to an author with role and position
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ItemAuthor {
    pub id: i32,
    pub item_id: i32,
    pub author_id: i32,
    pub role: Option<String>,
    pub author_type: i16,
    pub position: i16,
}
