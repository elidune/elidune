//! Data models for Elidune

pub mod author;
pub mod item;
pub mod loan;
pub mod specimen;
pub mod user;

// Re-export commonly used types
pub use author::Author;
pub use item::{Item, ItemShort};
pub use loan::{Loan, LoanDetails};
pub use specimen::Specimen;
pub use user::{User, UserShort};


