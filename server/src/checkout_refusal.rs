//! Checkout (`POST /loans`) refusals.
//!
//! [`checkout_refusal_forceable`] is the only rule for whether this caller's
//! `force` may skip a gate and for the `forceable` field on the 422 body. Gates are
//! recorded in order; the first non-forceable refusal wins over any forceable
//! one recorded earlier.

use serde::Serialize;

use crate::{
    error::{error_code, AppError, ItemToPickDetails},
    models::{
        circulation::CirculationStatus,
        user::{Rights, UserRights},
    },
};

/// Whether this caller may skip this checkout refusal.
///
/// `fines_over_threshold` is forceable only when `caller_rights.loans_rights` is
/// write — the same bar as [`crate::models::user::UserClaims::require_write_loans`]
/// and as renewal ([`crate::renewal::renewal_refusal_forceable`]). Mode `none`
/// never produces this code, so the override is not offered then.
/// `subscription_expired` and `item_held_for_other` are forceable only when
/// `caller_rights.circulation_override_rights` is write. The account-type name
/// and `settings_rights` are not consulted.
/// `max_loans`, `max_media_loans`, and `item_not_loanable` (not borrowable, or
/// damaged) are forceable for any caller who can send `force`.
/// Every other checkout code is false, including `card_lost`,
/// `item_to_pick_for_other`, and `business_rule_violation`.
#[must_use]
pub fn checkout_refusal_forceable(code: &str, caller_rights: &UserRights) -> bool {
    match code {
        error_code::SUBSCRIPTION_EXPIRED | error_code::ITEM_HELD_FOR_OTHER => {
            caller_rights.circulation_override_rights.rank() >= Rights::Write.rank()
        }
        error_code::FINES_OVER_THRESHOLD => {
            caller_rights.loans_rights.rank() >= Rights::Write.rank()
        }
        error_code::MAX_LOANS | error_code::MAX_MEDIA_LOANS | error_code::ITEM_NOT_LOANABLE => true,
        _ => false,
    }
}

/// Gates whose `force` skip requires a non-blank `overrideReason`.
#[must_use]
pub fn checkout_override_reason_required(code: &str) -> bool {
    matches!(
        code,
        error_code::SUBSCRIPTION_EXPIRED | error_code::ITEM_HELD_FOR_OTHER
    )
}

/// Trimmed staff reason, or `None` when missing or blank.
#[must_use]
pub fn normalized_override_reason(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 422 `{ code, error, message }` when `force` would skip a reason-required gate
/// and `overrideReason` is missing or blank.
#[must_use]
pub fn override_reason_required() -> AppError {
    AppError::coded(
        422,
        error_code::OVERRIDE_REASON_REQUIRED,
        error_code::OVERRIDE_REASON_REQUIRED_MESSAGE,
    )
}

/// Checkout gates collected for one `POST /loans`.
#[derive(Debug, Default)]
pub struct CheckoutProgress {
    pub refusals: Vec<CheckoutRefusal>,
    /// Trimmed `overrideReason`, when the caller sent one.
    pub override_reason: Option<String>,
    /// `force` would skip a reason-required gate and no reason was sent.
    pub missing_override_reason: bool,
    /// Hold left in place because `force` skipped `item_held_for_other`.
    pub preserved_hold_id: Option<i64>,
}

impl CheckoutProgress {
    #[must_use]
    pub fn new(override_reason: Option<String>) -> Self {
        Self {
            override_reason,
            ..Self::default()
        }
    }
}

/// `force` skips the gate only when [`checkout_refusal_forceable`] is true for this caller.
#[must_use]
pub fn force_skips_checkout_refusal(force: bool, code: &str, caller_rights: &UserRights) -> bool {
    force && checkout_refusal_forceable(code, caller_rights)
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
    /// Caller rights used by [`checkout_refusal_forceable`] for this refusal.
    pub caller_rights: UserRights,
}

impl CheckoutRefusal {
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>, caller_rights: &UserRights) -> Self {
        Self {
            code,
            message: message.into(),
            item_status: None,
            to_pick: None,
            caller_rights: caller_rights.clone(),
        }
    }

    #[must_use]
    pub fn business(message: impl Into<String>, caller_rights: &UserRights) -> Self {
        Self::new(error_code::BUSINESS_RULE, message, caller_rights)
    }

    /// Lost, claimed returned, to search, or missing. Not forceable.
    #[must_use]
    pub fn status_action(status: CirculationStatus, caller_rights: &UserRights) -> Self {
        Self {
            code: error_code::ITEM_STATUS_ACTION_REQUIRED,
            message: format!(
                "Item circulation status is {}. Resolve it before checkout.",
                status.as_api_str()
            ),
            item_status: Some(status),
            to_pick: None,
            caller_rights: caller_rights.clone(),
        }
    }

    #[must_use]
    pub fn to_pick(patron: ToPickPatron, caller_rights: &UserRights) -> Self {
        Self {
            code: error_code::ITEM_TO_PICK_FOR_OTHER,
            message: "This copy is on the pick list for another patron".to_string(),
            item_status: None,
            to_pick: Some(patron),
            caller_rights: caller_rights.clone(),
        }
    }

    #[must_use]
    pub fn forceable(&self) -> bool {
        checkout_refusal_forceable(self.code, &self.caller_rights)
    }

    #[must_use]
    pub fn to_body(&self) -> CheckoutRefusalBody {
        CheckoutRefusalBody {
            code: self.code.to_string(),
            error: "Business Rule Violation".to_string(),
            message: self.message.clone(),
            forceable: self.forceable(),
            reason_required: (self.forceable() && checkout_override_reason_required(self.code))
                .then_some(true),
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
    /// Server-computed for this caller. `fines_over_threshold` is true only when
    /// the caller has loans write. `subscription_expired` and
    /// `item_held_for_other` are true only when the caller has circulation
    /// override write.
    pub forceable: bool,
    /// Present and true when this refusal is forceable and `force` requires
    /// `overrideReason`. Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_required: Option<bool>,
    /// Circulation status for `item_status_action_required`
    /// (`lost`, `claimedReturned`, `toSearch`, `missing`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_status: Option<CirculationStatus>,
    /// Patron on the hold. Present for `item_to_pick_for_other` and
    /// `item_held_for_other`: `holdId`, `userId`, `cardNumber`, and `initials`.
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
        .position(|refusal| !refusal.forceable())
        .unwrap_or(0);
    refusals.into_iter().nth(index)
}

/// Record a gate. `force` drops the refusal when [`checkout_refusal_forceable`] is true.
/// A skipped `subscription_expired` or `item_held_for_other` without `overrideReason`
/// is remembered and returned from [`finish_checkout_refusals`] unless a later
/// non-forceable refusal wins. A non-forceable refusal is returned immediately.
pub fn push_checkout_refusal(
    progress: &mut CheckoutProgress,
    force: bool,
    refusal: CheckoutRefusal,
) -> Result<(), AppError> {
    if force_skips_checkout_refusal(force, refusal.code, &refusal.caller_rights) {
        if checkout_override_reason_required(refusal.code) && progress.override_reason.is_none() {
            progress.missing_override_reason = true;
        } else if refusal.code == error_code::ITEM_HELD_FOR_OTHER {
            if let Some(patron) = refusal.to_pick.as_ref() {
                progress.preserved_hold_id = Some(patron.hold_id);
            }
        }
        return Ok(());
    }
    let stop = !refusal.forceable();
    progress.refusals.push(refusal);
    if stop {
        return Err(refusal_error(std::mem::take(&mut progress.refusals)));
    }
    Ok(())
}

/// Return the winning refusal, or `override_reason_required` when `force` skipped
/// a reason-required gate without a reason and every other gate passed.
pub fn finish_checkout_refusals(progress: &CheckoutProgress) -> Result<(), AppError> {
    if let Some(refusal) = select_checkout_refusal(progress.refusals.clone()) {
        return Err(AppError::CheckoutRefused(Box::new(refusal)));
    }
    if progress.missing_override_reason {
        return Err(override_reason_required());
    }
    Ok(())
}

fn refusal_error(refusals: Vec<CheckoutRefusal>) -> AppError {
    let selected = select_checkout_refusal(refusals).expect("at least one checkout refusal");
    AppError::CheckoutRefused(Box::new(selected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rights(override_write: bool) -> UserRights {
        UserRights {
            circulation_override_rights: if override_write {
                Rights::Write
            } else {
                Rights::None
            },
            settings_rights: Rights::Write,
            ..UserRights::default()
        }
    }

    fn refusal(code: &'static str, message: &str, override_write: bool) -> CheckoutRefusal {
        CheckoutRefusal::new(code, message, &rights(override_write))
    }

    #[test]
    fn forceable_set_depends_on_the_right_for_that_gate() {
        let settings = rights(false);
        let admin = rights(true);
        let loans = UserRights {
            loans_rights: Rights::Write,
            ..UserRights::default()
        };
        let desk = UserRights::default();
        assert!(settings.settings_rights.rank() >= Rights::Write.rank());
        assert!(admin.circulation_override_rights.rank() >= Rights::Write.rank());
        for code in [
            error_code::SUBSCRIPTION_EXPIRED,
            error_code::ITEM_HELD_FOR_OTHER,
        ] {
            assert!(checkout_refusal_forceable(code, &admin), "{code}");
            assert!(!checkout_refusal_forceable(code, &settings), "{code}");
            assert!(!checkout_refusal_forceable(code, &loans), "{code}");
            assert!(!checkout_refusal_forceable(code, &desk), "{code}");
            assert!(force_skips_checkout_refusal(true, code, &admin), "{code}");
            assert!(
                !force_skips_checkout_refusal(true, code, &settings),
                "{code}"
            );
            assert!(checkout_override_reason_required(code), "{code}");
        }
        assert!(checkout_refusal_forceable(
            error_code::FINES_OVER_THRESHOLD,
            &loans
        ));
        assert!(!checkout_refusal_forceable(
            error_code::FINES_OVER_THRESHOLD,
            &settings
        ));
        assert!(!checkout_refusal_forceable(
            error_code::FINES_OVER_THRESHOLD,
            &admin
        ));
        assert!(!checkout_refusal_forceable(
            error_code::FINES_OVER_THRESHOLD,
            &desk
        ));
        assert!(force_skips_checkout_refusal(
            true,
            error_code::FINES_OVER_THRESHOLD,
            &loans
        ));
        assert!(!force_skips_checkout_refusal(
            true,
            error_code::FINES_OVER_THRESHOLD,
            &settings
        ));
        assert!(!force_skips_checkout_refusal(
            false,
            error_code::FINES_OVER_THRESHOLD,
            &loans
        ));
        assert!(!checkout_override_reason_required(
            error_code::FINES_OVER_THRESHOLD
        ));
        for code in [
            error_code::MAX_LOANS,
            error_code::MAX_MEDIA_LOANS,
            error_code::ITEM_NOT_LOANABLE,
        ] {
            assert!(checkout_refusal_forceable(code, &settings), "{code}");
            assert!(checkout_refusal_forceable(code, &desk), "{code}");
            assert!(force_skips_checkout_refusal(true, code, &desk), "{code}");
            assert!(!force_skips_checkout_refusal(false, code, &admin), "{code}");
            assert!(!checkout_override_reason_required(code), "{code}");
        }
        for code in [
            error_code::ITEM_STATUS_ACTION_REQUIRED,
            error_code::CARD_LOST,
            error_code::ITEM_TO_PICK_FOR_OTHER,
            error_code::BUSINESS_RULE,
        ] {
            assert!(!checkout_refusal_forceable(code, &admin), "{code}");
            assert!(!checkout_refusal_forceable(code, &loans), "{code}");
            assert!(!force_skips_checkout_refusal(true, code, &admin), "{code}");
        }
    }

    #[test]
    fn item_to_pick_for_other_stays_non_forceable_with_override_and_loans_write() {
        let caller = UserRights {
            circulation_override_rights: Rights::Write,
            loans_rights: Rights::Write,
            ..UserRights::default()
        };
        assert!(!checkout_refusal_forceable(
            error_code::ITEM_TO_PICK_FOR_OTHER,
            &caller
        ));
        assert!(!force_skips_checkout_refusal(
            true,
            error_code::ITEM_TO_PICK_FOR_OTHER,
            &caller
        ));
        let body = CheckoutRefusal::to_pick(
            ToPickPatron {
                hold_id: 9,
                user_id: 4,
                card_number: Some("C4".into()),
                initials: "AB".into(),
            },
            &caller,
        )
        .to_body();
        assert!(!body.forceable);
        assert_eq!(body.code, error_code::ITEM_TO_PICK_FOR_OTHER);
        assert_eq!(body.reason_required, None);
    }

    #[test]
    fn non_forceable_refusal_wins_over_an_earlier_forceable_one() {
        let selected = select_checkout_refusal(vec![
            refusal(error_code::SUBSCRIPTION_EXPIRED, "expired", true),
            refusal(error_code::CARD_LOST, "lost", true),
        ])
        .expect("refusal");
        assert_eq!(selected.code, error_code::CARD_LOST);
        assert!(!selected.forceable());
    }

    #[test]
    fn subscription_expired_without_override_right_is_itself_non_forceable() {
        let selected = select_checkout_refusal(vec![refusal(
            error_code::SUBSCRIPTION_EXPIRED,
            "expired",
            false,
        )])
        .expect("refusal");
        assert_eq!(selected.code, error_code::SUBSCRIPTION_EXPIRED);
        assert!(!selected.forceable());
    }

    #[test]
    fn missing_reason_is_returned_when_force_skips_a_reason_required_gate() {
        let mut progress = CheckoutProgress::new(None);
        push_checkout_refusal(
            &mut progress,
            true,
            refusal(error_code::SUBSCRIPTION_EXPIRED, "expired", true),
        )
        .expect("skipped");
        assert!(progress.missing_override_reason);
        match finish_checkout_refusals(&progress) {
            Err(AppError::Coded { status, code, .. }) => {
                assert_eq!(status, 422);
                assert_eq!(code, error_code::OVERRIDE_REASON_REQUIRED);
            }
            other => panic!("expected override_reason_required, got {other:?}"),
        }
    }

    #[test]
    fn non_forceable_gate_wins_over_a_missing_override_reason() {
        let mut progress = CheckoutProgress::new(None);
        push_checkout_refusal(
            &mut progress,
            true,
            refusal(error_code::ITEM_HELD_FOR_OTHER, "held", true),
        )
        .expect("skipped");
        let err = push_checkout_refusal(
            &mut progress,
            true,
            refusal(error_code::CARD_LOST, "lost", true),
        )
        .expect_err("lost card");
        match err {
            AppError::CheckoutRefused(refusal) => {
                assert_eq!(refusal.code, error_code::CARD_LOST);
                assert!(!refusal.forceable());
            }
            other => panic!("expected card_lost, got {other:?}"),
        }
    }

    #[test]
    fn reason_required_is_true_only_when_the_refusal_is_forceable() {
        let body = refusal(error_code::SUBSCRIPTION_EXPIRED, "expired", true).to_body();
        assert!(body.forceable);
        assert_eq!(body.reason_required, Some(true));
        let denied = refusal(error_code::SUBSCRIPTION_EXPIRED, "expired", false).to_body();
        assert!(!denied.forceable);
        assert_eq!(denied.reason_required, None);
        let loans_write = UserRights {
            loans_rights: Rights::Write,
            ..UserRights::default()
        };
        let fines =
            CheckoutRefusal::new(error_code::FINES_OVER_THRESHOLD, "fines", &loans_write).to_body();
        assert!(fines.forceable);
        assert_eq!(fines.reason_required, None);
    }

    #[test]
    fn first_forceable_wins_when_every_refusal_is_forceable() {
        let selected = select_checkout_refusal(vec![
            refusal(error_code::SUBSCRIPTION_EXPIRED, "expired", true),
            refusal(error_code::MAX_LOANS, "quota", true),
        ])
        .expect("refusal");
        assert_eq!(selected.code, error_code::SUBSCRIPTION_EXPIRED);
    }

    #[test]
    fn status_action_body_carries_the_api_status() {
        let body =
            CheckoutRefusal::status_action(CirculationStatus::ToSearch, &rights(false)).to_body();
        assert_eq!(body.code, error_code::ITEM_STATUS_ACTION_REQUIRED);
        assert!(!body.forceable);
        assert_eq!(body.item_status, Some(CirculationStatus::ToSearch));
        let json = serde_json::to_value(&body).expect("json");
        assert_eq!(json["itemStatus"], "toSearch");
        assert_eq!(json["forceable"], false);
    }
}
