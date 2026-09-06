//! API-facing DTOs shared across HTTP handlers and services (not HTTP-specific).

pub mod circulation;
pub mod fines;
pub mod holds;
pub mod library_info;
pub mod loans;
pub mod sse;
pub mod stats;
pub mod transits;
pub mod z3950;
