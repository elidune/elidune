//! Hold (physical item / title queue) model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::biblio::BiblioShort;
use crate::models::user::UserShort;

/// Hold lifecycle status (stored as lowercase strings in DB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HoldStatus {
    Pending,
    /// Copy assigned and still on the shelf. Staff pull it; the patron is not notified yet.
    #[serde(rename = "to_pick")]
    ToPick,
    Ready,
    Fulfilled,
    Cancelled,
    Expired,
}

impl HoldStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::ToPick => "to_pick",
            Self::Ready => "ready",
            Self::Fulfilled => "fulfilled",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    /// Counts toward the active-hold cap and the one-hold-per-title uniqueness rule.
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::ToPick | Self::Ready)
    }
}

impl From<String> for HoldStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "to_pick" => Self::ToPick,
            "ready" => Self::Ready,
            "fulfilled" => Self::Fulfilled,
            "cancelled" => Self::Cancelled,
            "expired" => Self::Expired,
            _ => Self::Pending,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for HoldStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for HoldStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for HoldStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Hold row from database (`holds` table).
///
/// Copy-level holds have `item_id` set. An unassigned title hold keeps
/// `item_id` NULL (`pending`) until a copy is allocated: `to_pick` when a shelf
/// copy is captured at placement, or `ready` / transit when a returned copy is
/// already in staff hands.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hold {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    /// Note visible to the patron and to staff. Blank stores SQL NULL.
    pub patron_note: Option<String>,
    /// Staff-only remark. Patrons and public hold views omit this field.
    pub internal_note: Option<String>,
    /// When the current `to_pick` assignment started. Null for every other status.
    pub pick_started_at: Option<DateTime<Utc>>,
}

impl Hold {
    /// Title-level until a concrete copy is assigned.
    #[must_use]
    pub fn is_title_level(&self) -> bool {
        self.item_id.is_none()
    }
}

/// Hold with bibliographic context and user details.
///
/// `biblio.items` is empty for an unassigned title-level hold, otherwise the
/// single physical copy this hold is queued on or trapped for.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldDetails {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// Patron who placed the hold. Same value as `user.id` when `user` is loaded.
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub biblio: BiblioShort,
    pub user: Option<UserShort>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    /// Note visible to the patron and to staff.
    pub patron_note: Option<String>,
    /// Staff-only remark. Omitted from `GET /holds`, public lists, and `GET /me/holds`.
    pub internal_note: Option<String>,
}

/// Default global cap when `circulation_settings` has no row yet.
pub const DEFAULT_MAX_ACTIVE_HOLDS: i16 = 20;

/// Days before a `to_pick` row is flagged overdue on the pick list. No auto-expiry.
pub const DEFAULT_PICK_DELAY_DAYS: i16 = 2;
/// Inclusive bounds for stored `max_active_holds` values.
pub const MIN_MAX_ACTIVE_HOLDS: i16 = 1;
pub const MAX_MAX_ACTIVE_HOLDS: i16 = 1000;

/// Create hold request — same reservation type, queued on the biblio.
///
/// `biblio_id` alone waits for any copy. `item_id` pins a specimen on that
/// title’s queue (staff). When both are sent, the item must belong to the biblio.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateHold {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    /// Pickup site (`sources.id`). When different from the copy's current site, fulfillment uses transit.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    /// Note visible to the patron and to staff. Blank stores SQL NULL.
    pub patron_note: Option<String>,
    /// Staff-only remark. Callers without `holds_rights` write cannot set it.
    pub internal_note: Option<String>,
    /// Staff-only: place the hold even when the patron is at/over the active-hold cap.
    #[serde(default)]
    pub force: bool,
}

impl CreateHold {
    #[must_use]
    pub fn for_item(user_id: i64, item_id: i64) -> Self {
        Self {
            user_id,
            item_id: Some(item_id),
            biblio_id: None,
            pickup_site_id: None,
            patron_note: None,
            internal_note: None,
            force: false,
        }
    }

    #[must_use]
    pub fn for_biblio(user_id: i64, biblio_id: i64) -> Self {
        Self {
            user_id,
            item_id: None,
            biblio_id: Some(biblio_id),
            pickup_site_id: None,
            patron_note: None,
            internal_note: None,
            force: false,
        }
    }

    #[must_use]
    pub fn is_title_level(&self) -> bool {
        self.item_id.is_none()
    }
}

/// Maximum length of a hold note, in Unicode scalar values (`chars().count()`), not bytes.
pub const HOLD_NOTE_MAX_CHARS: usize = 4000;

/// Returned as 400 `validation_error` when a hold write still sends `notes`.
pub const LEGACY_HOLD_NOTES_MESSAGE: &str =
    "notes is no longer accepted on hold writes. Use patronNote and internalNote.";

/// Returned as 403 `authorization_failed` when the caller lacks `holds_rights` write.
pub const INTERNAL_HOLD_NOTE_FORBIDDEN: &str = "Insufficient rights to set an internal hold note";

/// `internalNote` on `POST /holds`, including a JSON `null`.
///
/// A missing key stays [`Default`] (`present` is false). A present key, including
/// `null`, sets `present` so a patron cannot send the staff field.
#[derive(Debug, Clone, Default)]
pub struct MaybeHoldNote {
    pub present: bool,
    pub value: Option<String>,
}

impl<'de> Deserialize<'de> for MaybeHoldNote {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Option::<String>::deserialize(deserializer)?;
        Ok(Self {
            present: true,
            value,
        })
    }
}

/// Presence of the retired hold JSON key `notes`, including `null`.
pub fn deserialize_legacy_hold_notes<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(true)
}

/// Reject a hold note longer than [`HOLD_NOTE_MAX_CHARS`] after trimming.
pub fn validate_hold_note(label: &str, value: Option<&str>) -> Result<(), String> {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(());
    };
    if text.chars().count() > HOLD_NOTE_MAX_CHARS {
        return Err(format!(
            "{label} must be at most {HOLD_NOTE_MAX_CHARS} characters"
        ));
    }
    Ok(())
}

/// Trim a hold note. Blank and whitespace become SQL NULL.
#[must_use]
pub fn normalize_hold_note(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Hold JSON for a caller who does not see the internal catalog.
///
/// Same fields as [`Hold`] except `internalNote`, which is absent (not null).
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HoldPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pickup_site_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: HoldStatus,
    pub position: i32,
    pub patron_note: Option<String>,
    pub pick_started_at: Option<DateTime<Utc>>,
}

impl From<Hold> for HoldPublic {
    fn from(hold: Hold) -> Self {
        Self {
            id: hold.id,
            user_id: hold.user_id,
            biblio_id: hold.biblio_id,
            item_id: hold.item_id,
            pickup_site_id: hold.pickup_site_id,
            created_at: hold.created_at,
            notified_at: hold.notified_at,
            expires_at: hold.expires_at,
            status: hold.status,
            position: hold.position,
            patron_note: hold.patron_note,
            pick_started_at: hold.pick_started_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_hold_note, validate_hold_note, Hold, HoldPublic, HoldStatus, MaybeHoldNote,
        HOLD_NOTE_MAX_CHARS,
    };
    use chrono::Utc;
    use serde_json::json;

    fn sample(patron: &str, internal: &str) -> Hold {
        Hold {
            id: 1,
            user_id: 2,
            biblio_id: 3,
            item_id: None,
            pickup_site_id: None,
            created_at: Utc::now(),
            notified_at: None,
            expires_at: None,
            status: HoldStatus::Pending,
            position: 1,
            patron_note: Some(patron.into()),
            internal_note: Some(internal.into()),
            pick_started_at: None,
        }
    }

    #[test]
    fn staff_hold_json_drops_legacy_notes_and_keeps_both_fields() {
        let body = serde_json::to_value(sample("visible", "staff secret")).expect("json");
        assert_eq!(body["patronNote"], "visible");
        assert_eq!(body["internalNote"], "staff secret");
        assert!(body.get("notes").is_none());
    }

    #[test]
    fn public_hold_json_omits_the_internal_note() {
        let body = serde_json::to_value(HoldPublic::from(sample("visible", "staff secret")))
            .expect("json");
        assert_eq!(body["patronNote"], "visible");
        assert!(body.get("internalNote").is_none());
        assert!(body.get("notes").is_none());
        assert!(!body.to_string().contains("staff secret"));
    }

    #[test]
    fn note_limit_counts_unicode_scalars() {
        let accepted = "é".repeat(HOLD_NOTE_MAX_CHARS);
        assert!(accepted.len() > HOLD_NOTE_MAX_CHARS);
        assert!(validate_hold_note("Patron note", Some(&accepted)).is_ok());
        let rejected = "é".repeat(HOLD_NOTE_MAX_CHARS + 1);
        let err = validate_hold_note("Internal note", Some(&rejected)).expect_err("too long");
        assert!(err.contains("4000"));
        assert!(validate_hold_note("Patron note", Some("   ")).is_ok());
    }

    #[test]
    fn blank_notes_normalize_to_null() {
        assert_eq!(
            normalize_hold_note(Some("  hello  ".into())).as_deref(),
            Some("hello")
        );
        assert!(normalize_hold_note(Some("   ".into())).is_none());
        assert!(normalize_hold_note(None).is_none());
    }

    #[test]
    fn internal_note_null_is_present_and_omitted_key_is_not() {
        let with_null: MaybeHoldNote = serde_json::from_value(json!(null)).expect("null");
        assert!(with_null.present);
        assert!(with_null.value.is_none());
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            #[serde(default)]
            internal_note: MaybeHoldNote,
        }
        let absent: Body = serde_json::from_value(json!({})).expect("absent");
        assert!(!absent.internal_note.present);
    }
}
