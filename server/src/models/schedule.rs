//! Schedule models (periods, slots, closures)

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

// ---------------------------------------------------------------------------
// SchedulePeriod
// ---------------------------------------------------------------------------

/// A named schedule period (e.g. "Winter hours 2025")
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct SchedulePeriod {
    pub id: i32,
    /// Period name
    pub name: String,
    /// Period start date
    pub start_date: NaiveDate,
    /// Period end date
    pub end_date: NaiveDate,
    pub notes: Option<String>,
    pub crea_date: Option<DateTime<Utc>>,
    pub modif_date: Option<DateTime<Utc>>,
}

/// Create schedule period request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSchedulePeriod {
    pub name: String,
    /// Start date (YYYY-MM-DD)
    pub start_date: String,
    /// End date (YYYY-MM-DD)
    pub end_date: String,
    pub notes: Option<String>,
}

/// Update schedule period request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSchedulePeriod {
    pub name: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// ScheduleSlot
// ---------------------------------------------------------------------------

/// A time slot within a schedule period
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ScheduleSlot {
    pub id: i32,
    /// Parent period ID
    pub period_id: i32,
    /// Day of week (0=Monday, 6=Sunday)
    pub day_of_week: i16,
    /// Opening time
    pub open_time: NaiveTime,
    /// Closing time
    pub close_time: NaiveTime,
    pub crea_date: Option<DateTime<Utc>>,
}

/// Create schedule slot request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScheduleSlot {
    /// Day of week (0=Monday, 6=Sunday)
    pub day_of_week: i16,
    /// Opening time (HH:MM)
    pub open_time: String,
    /// Closing time (HH:MM)
    pub close_time: String,
}

// ---------------------------------------------------------------------------
// ScheduleClosure
// ---------------------------------------------------------------------------

/// An exceptional closure day
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ScheduleClosure {
    pub id: i32,
    /// Closure date
    pub closure_date: NaiveDate,
    /// Reason for closure
    pub reason: Option<String>,
    pub crea_date: Option<DateTime<Utc>>,
}

/// Create closure request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScheduleClosure {
    /// Closure date (YYYY-MM-DD)
    pub closure_date: String,
    pub reason: Option<String>,
}

/// Query parameters for schedule closures
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ScheduleClosureQuery {
    /// Filter closures from this date (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// Filter closures until this date (YYYY-MM-DD)
    pub end_date: Option<String>,
}
