//! Patron gates shared by checkout and the desk card scan.
//!
//! Checkout refuses a patron for every reason except [`PatronBlockReason::OverdueLoans`].
//! The card scan reports that extra code so the desk can see late loans, without
//! marking the patron blocked when it is the only reason.

use serde::Serialize;
use utoipa::ToSchema;

/// Why a patron cannot borrow, in checkout order.
///
/// `overdue_loans` is reported by the card scan only. Checkout does not refuse
/// a patron solely because a loan is past its due time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PatronBlockReason {
    /// [`crate::models::user::User::can_borrow`] is false.
    AccountBlocked,
    /// `users.card_lost`. Checkout always returns `422` `card_lost`. `force` does not override.
    CardLost,
    /// `users.expiry_at` is before now. Checkout refuses unless `force`.
    SubscriptionExpired,
    /// Unpaid balance is strictly above the effective threshold
    /// (public-type value when set, else the global circulation threshold).
    /// No amount-based block when that threshold is null.
    UnpaidFines,
    /// Active loans are at or above the audience total cap.
    /// Checkout refuses unless `force`. A per-media cap is applied only when
    /// an item is checked out, so it is not a card-scan reason.
    MaxLoans,
    /// At least one unreturned loan is past `expiry_at`. Checkout allows this.
    OverdueLoans,
}

impl PatronBlockReason {
    /// Whether checkout without `force` refuses the patron for this reason.
    #[must_use]
    pub fn blocks_checkout(self) -> bool {
        !matches!(self, Self::OverdueLoans)
    }
}

/// Facts already resolved the same way checkout resolves them.
#[derive(Debug, Clone, Copy)]
pub struct PatronGateInput {
    pub can_borrow: bool,
    pub card_lost: bool,
    pub subscription_expired: bool,
    pub fines_block: bool,
    pub active_loans: i64,
    /// `None` means no total cap. The settings resolver currently always supplies a number.
    pub max_loans: Option<i64>,
    pub overdue_loans: i64,
}

/// Block reasons in checkout order, then `overdue_loans`.
#[must_use]
pub fn patron_block_reasons(input: &PatronGateInput) -> Vec<PatronBlockReason> {
    let mut reasons = Vec::new();
    if !input.can_borrow {
        reasons.push(PatronBlockReason::AccountBlocked);
    }
    if input.card_lost {
        reasons.push(PatronBlockReason::CardLost);
    }
    if input.subscription_expired {
        reasons.push(PatronBlockReason::SubscriptionExpired);
    }
    if input.fines_block {
        reasons.push(PatronBlockReason::UnpaidFines);
    }
    if input.max_loans.is_some_and(|max| input.active_loans >= max) {
        reasons.push(PatronBlockReason::MaxLoans);
    }
    if input.overdue_loans > 0 {
        reasons.push(PatronBlockReason::OverdueLoans);
    }
    reasons
}

/// True when checkout without `force` would refuse this patron.
#[must_use]
pub fn patron_checkout_blocked(reasons: &[PatronBlockReason]) -> bool {
    reasons
        .iter()
        .copied()
        .any(PatronBlockReason::blocks_checkout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear() -> PatronGateInput {
        PatronGateInput {
            can_borrow: true,
            card_lost: false,
            subscription_expired: false,
            fines_block: false,
            active_loans: 0,
            max_loans: Some(5),
            overdue_loans: 0,
        }
    }

    #[test]
    fn clear_patron_has_no_reasons() {
        let reasons = patron_block_reasons(&clear());
        assert!(reasons.is_empty());
        assert!(!patron_checkout_blocked(&reasons));
    }

    #[test]
    fn each_checkout_reason_blocks_and_overdue_does_not() {
        let cases = [
            (
                PatronGateInput {
                    can_borrow: false,
                    ..clear()
                },
                PatronBlockReason::AccountBlocked,
                true,
            ),
            (
                PatronGateInput {
                    card_lost: true,
                    ..clear()
                },
                PatronBlockReason::CardLost,
                true,
            ),
            (
                PatronGateInput {
                    subscription_expired: true,
                    ..clear()
                },
                PatronBlockReason::SubscriptionExpired,
                true,
            ),
            (
                PatronGateInput {
                    fines_block: true,
                    ..clear()
                },
                PatronBlockReason::UnpaidFines,
                true,
            ),
            (
                PatronGateInput {
                    active_loans: 5,
                    max_loans: Some(5),
                    ..clear()
                },
                PatronBlockReason::MaxLoans,
                true,
            ),
            (
                PatronGateInput {
                    overdue_loans: 1,
                    active_loans: 1,
                    ..clear()
                },
                PatronBlockReason::OverdueLoans,
                false,
            ),
        ];
        for (input, reason, blocked) in cases {
            let reasons = patron_block_reasons(&input);
            assert_eq!(reasons, vec![reason]);
            assert_eq!(patron_checkout_blocked(&reasons), blocked);
        }
    }

    #[test]
    fn under_the_cap_and_unlimited_cap_do_not_add_max_loans() {
        let under = PatronGateInput {
            active_loans: 4,
            max_loans: Some(5),
            ..clear()
        };
        assert!(patron_block_reasons(&under).is_empty());
        let unlimited = PatronGateInput {
            active_loans: 99,
            max_loans: None,
            ..clear()
        };
        assert!(patron_block_reasons(&unlimited).is_empty());
    }
}
