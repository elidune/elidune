//! Data models for Elidune

pub mod author;
pub mod enums;
pub mod equipment;
pub mod event;
pub mod item;
pub mod loan;
pub mod remote_item;
pub mod schedule;
pub mod source;
pub mod specimen;
pub mod user;
pub mod visitor_count;

// Re-export commonly used types
pub use author::Author;
pub use enums::{Genre, Lang, Occupation, Sex, StaffType, EquipmentType, EquipmentStatus, EventType};
pub use equipment::Equipment;
pub use event::Event;
pub use item::{Item, ItemShort, MediaType};
pub use loan::{Loan, LoanDetails};
pub use remote_item::{ItemRemote, ItemRemoteShort};
pub use schedule::{SchedulePeriod, ScheduleSlot, ScheduleClosure};
pub use source::Source;
pub use specimen::Specimen;
pub use user::{User, UserShort};
pub use visitor_count::VisitorCount;


