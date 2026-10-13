//! Checkout (`POST /loans`) refusals.
//!
//! [`checkout_refusal_forceable`] is the only rule for whether staff `force`
//! may skip a gate and for the `forceable` field on the 422 body. Gates are
//! recorded in order; the first non-forceable refusal wins over any forceable
//! one recorded earlier.

use serde::Serialize;

use crate::{
    error::{error_code, AppError, ItemToPickDetails},
    models::circulation::CirculationStatus,
};

/// Whether staff `force` may skip this checkout refusal.
///
/// True only for `subscription_expired`, `fines_over_threshold`, `max_loans`,
/// `max_media_loans`, and `item_not_loanable` (not borrowable, or damaged).
/// Every other checkout code is false, including `business_rule_violation`.
#[must_use]
pub fn checkout_refusal_forceable(code: &str) -> bool {
    matches!(
        code,
        error_code::SUBSCRIPTION_EXPIRED
            | error_code::FINES_OVER_THRESHOLD
            | error_code::MAX_LOANS
            | error_code::MAX_MEDIA_LOANS
            | error_code::ITEM_NOT_LOANABLE
    )
}

/// `force` skips the gate only when [`checkout_refusal_forceable`] is true.
#[must_use]
pub fn force_skips_checkout_refusal(force: bool, code: &str) -> bool {
    force && checkout_refusal_forceable(code)
}

/// Patron named on a `to_pick` copy.
#[derive(Debug, Clone)]
pub struct ToPickPatron {
    pub hold_id: i64,
    pub user_id: i64,
    pub card_number: Option<String>,
    pub initials: String,
}

/// One checkout gate that failed.
#[derive(Debug, Clone)]
pub struct CheckoutRefusal {
    pub code: &'static str,
    pub message: String,
    pub item_status: Option<CirculationStatus>,
    pub to_pick: Option<ToPickPatron>,
}

impl CheckoutRefusal {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            item_status: None,
            to_pick: None,
        }
    }

    #[must_use]
    pub fn business(message: impl Into<String>) -> Self {
        Self::new(error_code::BUSINESS_RULE, message)
    }

    /// Lost, claimed returned, to search, or missing. Not forceable.
    #[must_use]
    pub fn status_action(status: CirculationStatus) -> Self {
        Self {
            code: error_code::ITEM_STATUS_ACTION_REQUIRED,
            message: format!(
                "Item circulation status is {}. Resolve it before checkout.",
                status.as_api_str()
            ),
            item_status: Some(status),
            to_pick: None,
        }
    }

    #[must_use]
    pub fn to_pick(patron: ToPickPatron) -> Self {
        Self {
            code: error_code::ITEM_TO_PICK_FOR_OTHER,
            message: "This copy is on the pick list for another patron".to_string(),
            item_status: None,
            to_pick: Some(patron),
        }
    }

    #[must_use]
    pub fn forceable(&self) -> bool {
        checkout_refusal_forceable(self.code)
    }

    #[must_use]
    pub fn to_body(&self) -> CheckoutRefusalBody {
        CheckoutRefusalBody {
            code: self.code.to_string(),
            error: "Business Rule Violation".to_string(),
            message: self.message.clone(),
            forceable: self.forceable(),
            item_status: self.item_status,
            details: self.to_pick.as_ref().map(|patron| ItemToPickDetails {
                hold_id: patron.hold_id.to_string(),
                user_id: patron.user_id.to_string(),
                card_number: patron.card_number.clone(),
                initials: patron.initials.clone(),
            }),
        }
    }
}

impl std::fmt::Display for CheckoutRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// 422 body for every checkout refusal.
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutRefusalBody {
    /// Stable machine-readable code.
    pub code: String,
    /// Always `Business Rule Violation` for checkout refusals.
    pub error: String,
    /// Desk-facing explanation.
    pub message: String,
    /// Server-computed. True only when staff `force` may skip this refusal.
    pub forceable: bool,
    /// Circulation status for `item_status_action_required`
    /// (`lost`, `claimedReturned`, `toSearch`, `missing`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_status: Option<CirculationStatus>,
    /// Patron waiting on a `to_pick` copy. Present for `item_to_pick_for_other`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<ItemToPickDetails>,
}

/// First non-forceable refusal, otherwise the first refusal.
#[must_use]
pub fn select_checkout_refusal(refusals: Vec<CheckoutRefusal>) -> Option<CheckoutRefusal> {
    if refusals.is_empty() {
        return None;
    }
    let index = refusals
        .iter()
        .position(|refusal| !checkout_refusal_forceable(refusal.code))
        .unwrap_or(0);
    refusals.into_iter().nth(index)
}

/// Record a gate. `force` drops the refusal when [`checkout_refusal_forceable`] is true.
/// A non-forceable refusal is returned immediately so it wins over earlier forceable ones.
pub fn push_checkout_refusal(
    refusals: &mut Vec<CheckoutRefusal>,
    force: bool,
    refusal: CheckoutRefusal,
) -> Result<(), AppError> {
    if force_skips_checkout_refusal(force, refusal.code) {
        return Ok(());
    }
    let stop = !refusal.forceable();
    refusals.push(refusal);
    if stop {
        return Err(refusal_error(std::mem::take(refusals)));
    }
    Ok(())
}

/// Return the winning refusal when any gate failed.
pub fn finish_checkout_refusals(refusals: Vec<CheckoutRefusal>) -> Result<(), AppError> {
    match select_checkout_refusal(refusals) {
        Some(refusal) => Err(AppError::CheckoutRefused(Box::new(refusal))),
        None => Ok(()),
    }
}

fn refusal_error(refusals: Vec<CheckoutRefusal>) -> AppError {
    let selected = select_checkout_refusal(refusals).expect("at least one checkout refusal");
    AppError::CheckoutRefused(Box::new(selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forceable_set_is_exactly_the_five_skippable_codes() {
        for code in [
            error_code::SUBSCRIPTION_EXPIRED,
            error_code::FINES_OVER_THRESHOLD,
            error_code::MAX_LOANS,
            error_code::MAX_MEDIA_LOANS,
            error_code::ITEM_NOT_LOANABLE,
        ] {
            assert!(checkout_refusal_forceable(code), "{code}");
            assert!(force_skips_checkout_refusal(true, code), "{code}");
            assert!(!force_skips_checkout_refusal(false, code), "{code}");
        }
        for code in [
            error_code::ITEM_HELD_FOR_OTHER,
            error_code::ITEM_STATUS_ACTION_REQUIRED,
            error_code::CARD_LOST,
            error_code::ITEM_TO_PICK_FOR_OTHER,
            error_code::BUSINESS_RULE,
        ] {
            assert!(!checkout_refusal_forceable(code), "{code}");
            assert!(!force_skips_checkout_refusal(true, code), "{code}");
        }
    }

    #[test]
    fn non_forceable_refusal_wins_over_an_earlier_forceable_one() {
        let selected = select_checkout_refusal(vec![
            CheckoutRefusal::new(error_code::SUBSCRIPTION_EXPIRED, "expired"),
            CheckoutRefusal::new(error_code::ITEM_HELD_FOR_OTHER, "held"),
        ])
        .expect("refusal");
        assert_eq!(selected.code, error_code::ITEM_HELD_FOR_OTHER);
        assert!(!selected.forceable());
    }

    #[test]
    fn first_forceable_wins_when_every_refusal_is_forceable() {
        let selected = select_checkout_refusal(vec![
            CheckoutRefusal::new(error_code::SUBSCRIPTION_EXPIRED, "expired"),
            CheckoutRefusal::new(error_code::MAX_LOANS, "quota"),
        ])
        .expect("refusal");
        assert_eq!(selected.code, error_code::SUBSCRIPTION_EXPIRED);
    }

    #[test]
    fn status_action_body_carries_the_api_status() {
        let body = CheckoutRefusal::status_action(CirculationStatus::ToSearch).to_body();
        assert_eq!(body.code, error_code::ITEM_STATUS_ACTION_REQUIRED);
        assert!(!body.forceable);
        assert_eq!(body.item_status, Some(CirculationStatus::ToSearch));
        let json = serde_json::to_value(&body).expect("json");
        assert_eq!(json["itemStatus"], "toSearch");
        assert_eq!(json["forceable"], false);
    }
}
