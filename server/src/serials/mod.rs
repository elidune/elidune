//! Serials domain helpers that do not touch the database.
//!
//! Persistence lives in `repository::serials` and `services::serials`.

pub mod grace;
pub mod prediction;
