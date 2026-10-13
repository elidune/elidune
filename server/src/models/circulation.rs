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
    /// Lost copy accepted back onto the shelf.
    AcceptFound,
    /// Lost copy accepted back, but not lendable.
    AcceptFoundDamaged,
}

/// Item circulation exception status (stored as `items.circulation_status` SMALLINT).
///
/// `NULL` or `0` in the database is [`CirculationStatus::Available`]. Known codes
/// are 1 lost, 2 damaged, 3 claimed-returned, 4 to-search, 5 declared missing.
/// Any other number is treated as available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum CirculationStatus {
    #[default]
    Available,
    Lost,
    Damaged,
    ClaimedReturned,
    /// Not on the shelf during a pick. Still counted in the public catalog as
    /// temporarily unavailable until staff declare it [`Self::Missing`] or [`Self::Lost`].
    /// A barcode return scan does not clear it. `POST /loans/{id}/return` still does.
    #[serde(rename = "toSearch")]
    ToSearch,
    /// Declared missing. Omitted from the public and patron catalogs.
    /// Distinct from [`Self::ToSearch`], which stays listed until this declaration.
    Missing,
}

impl CirculationStatus {
    pub const LOST: i16 = 1;
    pub const DAMAGED: i16 = 2;
    pub const CLAIMED_RETURNED: i16 = 3;
    pub const TO_SEARCH: i16 = 4;
    pub const MISSING: i16 = 5;

    #[must_use]
    pub fn from_db(value: Option<i16>) -> Self {
        match value {
            Some(Self::LOST) => Self::Lost,
            Some(Self::DAMAGED) => Self::Damaged,
            Some(Self::CLAIMED_RETURNED) => Self::ClaimedReturned,
            Some(Self::TO_SEARCH) => Self::ToSearch,
            Some(Self::MISSING) => Self::Missing,
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
            Self::ToSearch => Self::TO_SEARCH,
            Self::Missing => Self::MISSING,
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

    /// Barcode return must not change these. Staff use the dedicated action
    /// afterwards (for example lost-found). Claimed-returned is not included:
    /// a return of that open loan still clears it.
    #[must_use]
    pub fn blocks_return_scan(self) -> bool {
        matches!(
            self,
            Self::Lost | Self::Damaged | Self::Missing | Self::ToSearch
        )
    }

    /// Next item status, or a desk-facing refusal.
    pub fn transition(self, action: CirculationAction) -> Result<Self, &'static str> {
        match (self, action) {
            (
                Self::Available
                | Self::Damaged
                | Self::ClaimedReturned
                | Self::ToSearch
                | Self::Missing,
                CirculationAction::MarkLost,
            ) => Ok(Self::Lost),
            (Self::Lost, CirculationAction::MarkLost) => Err("Item is already marked lost"),
            (Self::Available | Self::Damaged, CirculationAction::MarkDamaged) => Ok(Self::Damaged),
            (Self::Lost, CirculationAction::MarkDamaged) => {
                Err("Cannot mark a lost item as damaged")
            }
            (Self::Missing, CirculationAction::MarkDamaged) => {
                Err("Cannot mark a missing item as damaged")
            }
            (Self::ClaimedReturned, CirculationAction::MarkDamaged) => {
                Err("Resolve the claims-returned case before marking the item damaged")
            }
            (Self::ToSearch, CirculationAction::MarkDamaged) => {
                Err("Clear the missing-shelf search before marking the item damaged")
            }
            (Self::Available, CirculationAction::ClaimReturned) => Ok(Self::ClaimedReturned),
            (Self::ClaimedReturned, CirculationAction::ClaimReturned) => {
                Err("Item is already in the claims-returned queue")
            }
            (
                Self::Lost | Self::Damaged | Self::ToSearch | Self::Missing,
                CirculationAction::ClaimReturned,
            ) => Err("Cannot flag claims-returned on a lost, damaged, or missing-shelf item"),
            (Self::ClaimedReturned, CirculationAction::ResolveFound) => Ok(Self::Available),
            (Self::ClaimedReturned, CirculationAction::ResolveNotFound) => Ok(Self::Lost),
            (_, CirculationAction::ResolveFound | CirculationAction::ResolveNotFound) => {
                Err("Item is not in the claims-returned queue")
            }
            (Self::Lost, CirculationAction::AcceptFound) => Ok(Self::Available),
            (Self::Lost, CirculationAction::AcceptFoundDamaged) => Ok(Self::Damaged),
            (_, CirculationAction::AcceptFound | CirculationAction::AcceptFoundDamaged) => {
                Err("Item is not marked lost")
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
            Self::ClaimReturned | Self::AcceptFound | Self::AcceptFoundDamaged => false,
            Self::MarkDamaged => matches!(disposition, Some(DamageDisposition::Return) | None),
        }
    }

    #[must_use]
    pub fn notify_holds(self) -> bool {
        matches!(self, Self::ResolveFound | Self::AcceptFound)
    }

    /// Billing is a lost/damaged *transition effect* only.
    /// Claimed-returned (flag or resolve) never bills — investigation queue.
    #[must_use]
    pub fn allows_billing(self) -> bool {
        matches!(self, Self::MarkLost | Self::MarkDamaged)
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
            CirculationStatus::from_db(Some(4)),
            CirculationStatus::ToSearch
        );
        assert_eq!(
            CirculationStatus::from_db(Some(5)),
            CirculationStatus::Missing
        );
        assert_eq!(
            CirculationStatus::from_db(Some(99)),
            CirculationStatus::Available
        );
    }

    #[test]
    fn return_scan_blocks_lost_damaged_missing_and_to_search() {
        assert!(!CirculationStatus::Available.blocks_return_scan());
        assert!(!CirculationStatus::ClaimedReturned.blocks_return_scan());
        assert!(CirculationStatus::Lost.blocks_return_scan());
        assert!(CirculationStatus::Damaged.blocks_return_scan());
        assert!(CirculationStatus::Missing.blocks_return_scan());
        assert!(CirculationStatus::ToSearch.blocks_return_scan());
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

    #[test]
    fn billing_only_for_lost_and_damaged_transitions() {
        assert!(CirculationAction::MarkLost.allows_billing());
        assert!(CirculationAction::MarkDamaged.allows_billing());
        assert!(!CirculationAction::ResolveNotFound.allows_billing());
        assert!(!CirculationAction::ClaimReturned.allows_billing());
        assert!(!CirculationAction::ResolveFound.allows_billing());
    }

    #[test]
    fn lost_found_returns_to_shelf_or_damaged_without_billing() {
        assert_eq!(
            CirculationStatus::Lost
                .transition(CirculationAction::AcceptFound)
                .expect("circulate"),
            CirculationStatus::Available
        );
        assert!(CirculationStatus::Available.borrowable_after());
        assert_eq!(
            CirculationStatus::Lost
                .transition(CirculationAction::AcceptFoundDamaged)
                .expect("damaged"),
            CirculationStatus::Damaged
        );
        assert!(!CirculationStatus::Damaged.borrowable_after());
        assert!(CirculationStatus::Available
            .transition(CirculationAction::AcceptFound)
            .is_err());
        assert!(!CirculationAction::AcceptFound.closes_loan(None));
        assert!(CirculationAction::AcceptFound.notify_holds());
        assert!(!CirculationAction::AcceptFoundDamaged.notify_holds());
        assert!(!CirculationAction::AcceptFound.allows_billing());
        assert!(!CirculationAction::AcceptFoundDamaged.allows_billing());
    }
}
