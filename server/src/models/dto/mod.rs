//! API-facing DTOs shared across HTTP handlers and services (not HTTP-specific).

pub mod card;
pub mod circulation;
pub mod deposit_import;
pub mod fines;
pub mod holds;
pub mod library_info;
pub mod loans;
pub mod patron_card;
pub mod sse;
pub mod stats;
pub mod transits;
pub mod weeding;
pub mod z3950;
