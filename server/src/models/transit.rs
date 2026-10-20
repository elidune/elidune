//! Inter-site item transit (hold fulfillment movement).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::catalog_view::{hold_for_identity, LoanBorrowerMinimized, LoanBorrowerResponse};
use crate::models::hold::Hold;
use crate::models::user::{Rights, UserRights, UserShort};

/// Transit lifecycle (stored as lowercase strings in DB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TransitStatus {
    Requested,
    InTransit,
    Received,
    Cancelled,
}

impl TransitStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::InTransit => "in_transit",
            Self::Received => "received",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::Requested | Self::InTransit)
    }

    #[must_use]
    pub fn blocks_checkout(self, force: bool) -> bool {
        match self {
            Self::InTransit => true,
            Self::Requested => !force,
            Self::Received | Self::Cancelled => false,
        }
    }
}

impl From<String> for TransitStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "in_transit" => Self::InTransit,
            "received" => Self::Received,
            "cancelled" => Self::Cancelled,
            _ => Self::Requested,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for TransitStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TransitStatus {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for TransitStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Patron on a transit row: library card and initials only.
///
/// Never the name, login, or the staff profile, including when the caller has
/// users read. Null when the transit has no hold, the hold is cancelled or
/// expired (or otherwise closed), or the caller has no holds read/write.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitPatron {
    /// `users.barcode`. Null when the patron has no card on file.
    pub card_number: Option<String>,
    /// Uppercase first letter of `firstname` plus first letter of `lastname`.
    pub initials: String,
}

impl From<LoanBorrowerMinimized> for TransitPatron {
    fn from(minimized: LoanBorrowerMinimized) -> Self {
        Self {
            card_number: minimized.card_number,
            initials: minimized.initials,
        }
    }
}

/// Card and initials for a transit, or `None` when the caller lacks holds read.
///
/// Calls the minimized arm of [`hold_for_identity`] (the same `{ cardNumber, initials }`
/// shape as a hold slip or shelf label). Users read is cleared before that call so
/// the staff profile is never selected. The helper still returns `None` when
/// `holds_rights` is below read. The hold-status gate (pending, set aside, or
/// ready) is applied by the caller.
#[must_use]
pub fn transit_patron(rights: &UserRights, patron: UserShort) -> Option<TransitPatron> {
    let mut rights = rights.clone();
    rights.users_rights = Rights::None;
    let LoanBorrowerResponse::Minimized(minimized) = hold_for_identity(&rights, patron)? else {
        return None;
    };
    Some(TransitPatron::from(minimized))
}

/// Item movement row (`item_transits`) plus the copy and patron for the queue.
///
/// `barcode`, `title`, `sourceId`, `sourceName`, and `patron` are filled when a
/// transit is returned. They are not columns of `item_transits`.
///
/// `sourceId` is the copy's current site (`items.site_id`), next to `sourceName`.
/// `patron` is `{ cardNumber, initials }` while the linked hold is pending, set
/// aside (`to_pick`), or ready for pickup, and the caller may see that patron.
/// It is null for a return to the home site, a cancelled or expired hold, and a
/// caller without holds read or write.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemTransit {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    /// Item barcode. Null when the copy has none.
    #[serde(default)]
    #[sqlx(skip)]
    pub barcode: Option<String>,
    /// Bibliographic title.
    #[serde(default)]
    #[sqlx(skip)]
    pub title: Option<String>,
    /// Current site of the copy (`items.site_id`), beside [`Self::site_name`].
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    #[serde(default)]
    #[sqlx(skip)]
    pub site_id: Option<i64>,
    /// Current site name (`sources.name` for [`Self::source_id`]).
    #[serde(default)]
    #[sqlx(skip)]
    pub site_name: Option<String>,
    /// `{ cardNumber, initials }`, or null. See [`TransitPatron`].
    #[serde(default)]
    #[sqlx(skip)]
    pub patron: Option<TransitPatron>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub hold_id: Option<i64>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub from_site_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub to_site_id: i64,
    pub status: TransitStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub received_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub shipped_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub received_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub cancelled_by: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub reversed_from_id: Option<i64>,
}

/// Staff request to start (and optionally ship) a hold transit.
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransit {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub from_site_id: Option<i64>,
    /// Destination. Must match `hold.pickupSiteId` when that field is already set.
    /// If the hold has no pickup site yet, this value is stored on the hold.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub to_site_id: Option<i64>,
    pub notes: Option<String>,
    /// When true, skip `requested` and mark the copy in transit immediately.
    #[serde(default)]
    pub ship: bool,
}

/// Ship / receive / cancel notes.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitActionRequest {
    pub notes: Option<String>,
    /// On cancel: create a reverse transit back to the origin (default true when in transit).
    pub reverse: Option<bool>,
}

/// Transit plus the hold it fulfills (when linked).
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitWithHold {
    pub transit: ItemTransit,
    pub hold: Option<Hold>,
}

#[cfg(test)]
mod tests {
    use super::{transit_patron, TransitStatus};
    use crate::models::catalog_view::partial_user_short;
    use crate::models::user::{Rights, UserRights};

    #[test]
    fn users_read_still_yields_card_and_initials_only() {
        let mut patron = partial_user_short(7, Some("CARD".into()), Some("Helene".into()), Some("Holder".into()));
        patron.login = Some("helene".into());
        let rights = UserRights {
            holds_rights: Rights::Write,
            users_rights: Rights::Write,
            ..UserRights::default()
        };
        let shown = transit_patron(&rights, patron).expect("holds rights");
        assert_eq!(shown.card_number.as_deref(), Some("CARD"));
        assert_eq!(shown.initials, "HH");
        let value = serde_json::to_value(&shown).expect("json");
        let mut keys: Vec<&str> = value.as_object().expect("object").keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["cardNumber", "initials"]);
        assert!(!value.to_string().contains("Helene"));
        assert!(!value.to_string().contains("helene"));
    }

    #[test]
    fn own_or_no_holds_rights_hides_the_patron_even_with_users_read() {
        let patron = partial_user_short(7, Some("CARD".into()), Some("Helene".into()), Some("Holder".into()));
        for holds_rights in [Rights::None, Rights::Own] {
            let rights = UserRights {
                holds_rights,
                users_rights: Rights::Write,
                ..UserRights::default()
            };
            assert!(transit_patron(&rights, patron.clone()).is_none(), "{holds_rights:?}");
        }
    }

    #[test]
    fn in_transit_always_blocks_checkout() {
        assert!(TransitStatus::InTransit.blocks_checkout(false));
        assert!(TransitStatus::InTransit.blocks_checkout(true));
    }

    #[test]
    fn requested_blocks_normal_checkout_but_allows_force() {
        assert!(TransitStatus::Requested.blocks_checkout(false));
        assert!(!TransitStatus::Requested.blocks_checkout(true));
    }

    #[test]
    fn terminal_states_do_not_block() {
        assert!(!TransitStatus::Received.blocks_checkout(false));
        assert!(!TransitStatus::Cancelled.blocks_checkout(false));
    }
}
