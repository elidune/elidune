//! Statistics persistence (saved queries, executor, dashboard aggregates).

pub mod dashboard;
pub mod executor;
pub mod saved_queries;

pub use dashboard::StatsFilter;
