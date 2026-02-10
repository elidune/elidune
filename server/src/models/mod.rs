//! Data models for Elidune

pub mod author;
pub mod enums;
pub mod item;
pub mod loan;
pub mod remote_item;
pub mod specimen;
pub mod user;

// Re-export commonly used types
pub use author::Author;
pub use enums::{Genre, Lang, Occupation, Sex};
pub use item::{Item, ItemShort, MediaType};
pub use loan::{Loan, LoanDetails};
pub use remote_item::{ItemRemote, ItemRemoteShort};
pub use specimen::Specimen;
pub use user::{User, UserShort};


