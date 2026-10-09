//! Event model (cultural actions, school visits, animations)

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

/// Optional attachment supplied when creating an event (Base64-encoded payload).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventAttachmentInput {
    /// Display file name (path segments are stripped server-side).
    pub file_name: String,
    /// MIME type (e.g. `application/pdf`, `image/png`).
    pub mime_type: String,
    /// File content encoded as standard Base64.
    pub data_base64: String,
}

/// Event record
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// Event name
    pub name: String,
    /// Type (0=animation, 1=school_visit, 2=exhibition, 3=conference, 4=workshop, 5=show, 6=other)
    pub event_type: i16,
    /// Event date
    pub event_date: NaiveDate,
    /// Start time
    pub start_time: Option<NaiveTime>,
    /// End time
    pub end_time: Option<NaiveTime>,
    /// Number of attendees
    pub attendees_count: Option<i32>,
    /// When true, the event targets every audience and `public_types` is empty.
    /// This is explicit: an empty `public_types` list without this flag is invalid.
    pub all_audiences: bool,
    /// Selected audiences (`public_types.name`). Empty when `all_audiences` is true.
    /// Not a column on `events`; loaded from `event_audiences`.
    #[sqlx(default)]
    pub public_types: Vec<String>,
    /// School name (for school visits)
    pub school_name: Option<String>,
    /// Class name (for school visits)
    pub class_name: Option<String>,
    /// Number of students (for school visits)
    pub students_count: Option<i32>,
    /// Partner organization name
    pub partner_name: Option<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub update_at: Option<DateTime<Utc>>,
    /// Date the announcement email was last sent
    pub announcement_sent_at: Option<DateTime<Utc>>,
    /// Original attachment file name when present
    pub attachment_filename: Option<String>,
    /// Attachment MIME type when present
    pub attachment_mime_type: Option<String>,
    /// Attachment size in bytes when present.
    pub attachment_size: Option<i32>,
    /// Full attachment as standard Base64. Included only in single-event responses (`GET` / `POST` / `PUT`), not in list responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[sqlx(skip)]
    pub attachment_data_base64: Option<String>,
}

/// Create event request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEvent {
    pub name: String,
    /// Type (0=animation, 1=school_visit, 2=exhibition, 3=conference, 4=workshop, 5=show, 6=other)
    #[serde(default)]
    pub event_type: Option<i16>,
    /// Event date (YYYY-MM-DD)
    pub event_date: String,
    /// Start time (HH:MM)
    #[serde(default)]
    pub start_time: Option<String>,
    /// End time (HH:MM)
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub attendees_count: Option<i32>,
    /// When true, notify every audience. Mutually exclusive with a non-empty `publicTypes` list.
    /// Omitted or false with no audiences is rejected: an empty selection does not mean everyone.
    #[serde(default)]
    pub all_audiences: bool,
    /// Target audiences: `public_types.name` values from `GET /public-types`.
    /// Required unless `allAudiences` is true. Duplicates and blank entries are ignored.
    /// When this field is omitted, a legacy `publicType` string is accepted and mapped to a one-element list.
    #[serde(default)]
    pub public_types: Option<Vec<String>>,
    /// Deprecated single audience (`public_types.name`).
    /// Used only when `publicTypes` is omitted. Null or blank does not mean every audience; send `allAudiences: true` for that.
    #[serde(default)]
    pub public_type: Option<String>,
    #[serde(default)]
    pub school_name: Option<String>,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub students_count: Option<i32>,
    #[serde(default)]
    pub partner_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Optional attachment (stored in-database; max size enforced server-side).
    #[serde(default)]
    pub attachment: Option<EventAttachmentInput>,
}

/// Update event request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEvent {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub event_type: Option<i16>,
    #[serde(default)]
    pub event_date: Option<String>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub attendees_count: Option<i32>,
    /// When set, replaces the audience mode. `true` clears `event_audiences`.
    /// Omitted leaves the stored audiences unchanged unless `publicTypes` or legacy `publicType` is sent.
    #[serde(default)]
    pub all_audiences: Option<bool>,
    /// When set, replaces the audience list. An empty list is rejected unless `allAudiences` is true.
    /// When omitted, a legacy `publicType` string is accepted and mapped to a one-element list.
    #[serde(default)]
    pub public_types: Option<Vec<String>>,
    /// Deprecated single audience. Ignored when `publicTypes` is present.
    /// Null or blank does not mean every audience.
    #[serde(default)]
    pub public_type: Option<String>,
    #[serde(default)]
    pub school_name: Option<String>,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub students_count: Option<i32>,
    #[serde(default)]
    pub partner_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// When `true`, removes the attachment. Takes precedence over `attachment`.
    #[serde(default)]
    pub remove_attachment: Option<bool>,
    /// Replaces the attachment (same shape as in [`CreateEvent`]).
    #[serde(default)]
    pub attachment: Option<EventAttachmentInput>,
}

/// Query parameters for events
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventQuery {
    /// Filter by start date (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// Filter by end date (YYYY-MM-DD)
    pub end_date: Option<String>,
    /// Filter by event type
    pub event_type: Option<i16>,
    /// Page number (1-based)
    pub page: Option<i64>,
    /// Items per page
    pub per_page: Option<i64>,
}

/// Resolved audience targeting stored on an event.
/// `all_audiences` and a non-empty `public_types` list are mutually exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudienceSelection {
    pub all_audiences: bool,
    pub public_types: Vec<String>,
}
