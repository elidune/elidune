//! Item-level circulation exception state machine.
//!
//! Lost / damaged / claimed-returned live on the **item**, not the loan.
//! The loan is archived or left open according to the staff action; the item
//! status is the source of truth for whether the copy may circulate.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Staff choice when marking an item damaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum DamageDisposition {
    /// Patron returned the copy; close the loan. Item stays damaged / not borrowable.
    Return,
    /// Item stays charged to the patron; loan remains open.
    Keep,
}

/// How a claims-returned case is closed after an inventory check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ClaimsResolveOutcome {
    /// Copy found; treat as a real return.
    Found,
    /// Copy not found; escalate to lost.
    NotFound,
}

/// Desk action that may change [`CirculationStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CirculationAction {
    MarkLost,
    MarkDamaged,
    ClaimReturned,
    ResolveFound,
    ResolveNotFound,
}

/// Item circulation exception status (stored as `items.circulation_status` SMALLINT).
///
/// `NULL` or `0` in the database is [`CirculationStatus::Available`]. Legacy
/// numeric codes that are not 1–3 are treated as available (they do not enter
/// this state machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum CirculationStatus {
    #[default]
    Available,
    Lost,
    Damaged,
    ClaimedReturned,
}

impl CirculationStatus {
    pub const LOST: i16 = 1;
    pub const DAMAGED: i16 = 2;
    pub const CLAIMED_RETURNED: i16 = 3;

    #[must_use]
    pub fn from_db(value: Option<i16>) -> Self {
        match value {
            Some(Self::LOST) => Self::Lost,
            Some(Self::DAMAGED) => Self::Damaged,
            Some(Self::CLAIMED_RETURNED) => Self::ClaimedReturned,
            _ => Self::Available,
        }
    }

    #[must_use]
    pub fn as_db(self) -> i16 {
        match self {
            Self::Available => 0,
            Self::Lost => Self::LOST,
            Self::Damaged => Self::DAMAGED,
            Self::ClaimedReturned => Self::CLAIMED_RETURNED,
        }
    }

    #[must_use]
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub fn blocks_circulation(self) -> bool {
        !self.is_available()
    }

    /// Next item status, or a desk-facing refusal.
    pub fn transition(self, action: CirculationAction) -> Result<Self, &'static str> {
        match (self, action) {
            (
                Self::Available | Self::Damaged | Self::ClaimedReturned,
                CirculationAction::MarkLost,
            ) => Ok(Self::Lost),
            (Self::Lost, CirculationAction::MarkLost) => Err("Item is already marked lost"),
            (Self::Available | Self::Damaged, CirculationAction::MarkDamaged) => Ok(Self::Damaged),
            (Self::Lost, CirculationAction::MarkDamaged) => {
                Err("Cannot mark a lost item as damaged")
            }
            (Self::ClaimedReturned, CirculationAction::MarkDamaged) => {
                Err("Resolve the claims-returned case before marking the item damaged")
            }
            (Self::Available, CirculationAction::ClaimReturned) => Ok(Self::ClaimedReturned),
            (Self::ClaimedReturned, CirculationAction::ClaimReturned) => {
                Err("Item is already in the claims-returned queue")
            }
            (Self::Lost | Self::Damaged, CirculationAction::ClaimReturned) => {
                Err("Cannot flag claims-returned on a lost or damaged item")
            }
            (Self::ClaimedReturned, CirculationAction::ResolveFound) => Ok(Self::Available),
            (Self::ClaimedReturned, CirculationAction::ResolveNotFound) => Ok(Self::Lost),
            (_, CirculationAction::ResolveFound | CirculationAction::ResolveNotFound) => {
                Err("Item is not in the claims-returned queue")
            }
        }
    }

    /// Whether the copy should be borrowable after this status is applied.
    #[must_use]
    pub fn borrowable_after(self) -> bool {
        self.is_available()
    }
}

impl CirculationAction {
    #[must_use]
    pub fn closes_loan(self, disposition: Option<DamageDisposition>) -> bool {
        match self {
            Self::MarkLost | Self::ResolveFound | Self::ResolveNotFound => true,
            Self::ClaimReturned => false,
            Self::MarkDamaged => matches!(disposition, Some(DamageDisposition::Return) | None),
        }
    }

    #[must_use]
    pub fn notify_holds(self) -> bool {
        matches!(self, Self::ResolveFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_db_maps_known_and_legacy_values() {
        assert_eq!(
            CirculationStatus::from_db(None),
            CirculationStatus::Available
        );
        assert_eq!(
            CirculationStatus::from_db(Some(0)),
            CirculationStatus::Available
        );
        assert_eq!(CirculationStatus::from_db(Some(1)), CirculationStatus::Lost);
        assert_eq!(
            CirculationStatus::from_db(Some(2)),
            CirculationStatus::Damaged
        );
        assert_eq!(
            CirculationStatus::from_db(Some(3)),
            CirculationStatus::ClaimedReturned
        );
        assert_eq!(
            CirculationStatus::from_db(Some(99)),
            CirculationStatus::Available
        );
    }

    #[test]
    fn lost_closes_loan_and_blocks_borrow() {
        let next = CirculationStatus::Available
            .transition(CirculationAction::MarkLost)
            .expect("lost");
        assert_eq!(next, CirculationStatus::Lost);
        assert!(next.blocks_circulation());
        assert!(!next.borrowable_after());
        assert!(CirculationAction::MarkLost.closes_loan(None));
        assert!(!CirculationAction::MarkLost.notify_holds());
    }

    #[test]
    fn damaged_return_closes_loan_keep_does_not() {
        let next = CirculationStatus::Available
            .transition(CirculationAction::MarkDamaged)
            .expect("damaged");
        assert_eq!(next, CirculationStatus::Damaged);
        assert!(!next.borrowable_after());
        assert!(CirculationAction::MarkDamaged.closes_loan(Some(DamageDisposition::Return)));
        assert!(!CirculationAction::MarkDamaged.closes_loan(Some(DamageDisposition::Keep)));
        assert!(!CirculationAction::MarkDamaged.notify_holds());
    }

    #[test]
    fn claimed_returned_keeps_loan_open() {
        let next = CirculationStatus::Available
            .transition(CirculationAction::ClaimReturned)
            .expect("claimed");
        assert_eq!(next, CirculationStatus::ClaimedReturned);
        assert!(!next.borrowable_after());
        assert!(!CirculationAction::ClaimReturned.closes_loan(None));
        assert!(!CirculationAction::ClaimReturned.notify_holds());
    }

    #[test]
    fn claimed_returned_does_not_silently_clear() {
        let status = CirculationStatus::ClaimedReturned;
        assert!(status.transition(CirculationAction::ClaimReturned).is_err());
        assert_eq!(
            status
                .transition(CirculationAction::ResolveFound)
                .expect("found"),
            CirculationStatus::Available
        );
        assert_eq!(
            status
                .transition(CirculationAction::ResolveNotFound)
                .expect("not found"),
            CirculationStatus::Lost
        );
        assert!(CirculationAction::ResolveFound.closes_loan(None));
        assert!(CirculationAction::ResolveFound.notify_holds());
        assert!(CirculationAction::ResolveNotFound.closes_loan(None));
        assert!(!CirculationAction::ResolveNotFound.notify_holds());
    }

    #[test]
    fn cannot_resolve_available_or_damage_lost_item() {
        assert!(CirculationStatus::Available
            .transition(CirculationAction::ResolveFound)
            .is_err());
        assert!(CirculationStatus::Lost
            .transition(CirculationAction::MarkDamaged)
            .is_err());
        assert!(CirculationStatus::Lost
            .transition(CirculationAction::ClaimReturned)
            .is_err());
        assert!(CirculationStatus::Damaged
            .transition(CirculationAction::ClaimReturned)
            .is_err());
        assert!(CirculationStatus::ClaimedReturned
            .transition(CirculationAction::MarkDamaged)
            .is_err());
    }

    #[test]
    fn claimed_returned_may_escalate_to_lost() {
        assert_eq!(
            CirculationStatus::ClaimedReturned
                .transition(CirculationAction::MarkLost)
                .expect("escalate"),
            CirculationStatus::Lost
        );
    }
}
