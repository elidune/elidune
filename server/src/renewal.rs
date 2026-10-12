//! One renewal decision for `POST /loans/{id}/renew` and `GET /me/loans`.
//!
//! Account facts in [`RenewalContext`] are computed once per request. Loan facts
//! are per copy. Staff `force` sets [`RenewalContext::bypass_account_gates`].

use chrono::{DateTime, Duration, Utc};

use crate::{
    error::AppError, models::item::WeedingStatus, services::calendar::format_patron_iso_date,
};

/// Facts loaded once for `renew_loan` before the locked write.
#[derive(Debug, Clone)]
pub struct LoanRenewalSnapshot {
    pub user_id: i64,
    pub returned: bool,
    pub loan: RenewalLoan,
    pub max_renewals: i16,
    pub renew_window_days: i16,
}

/// Loan-level facts the eligibility function needs. Not a catalog view.
#[derive(Debug, Clone, Copy)]
pub struct RenewalLoan {
    pub renewal_count: i16,
    pub due_at: DateTime<Utc>,
    pub withdrawn: bool,
    pub claimed_returned: bool,
    pub hold_waiting: bool,
}

/// Borrower facts. `can_borrow` is [`crate::models::user::User::can_borrow`].
#[derive(Debug, Clone, Copy)]
pub struct RenewalUser {
    pub can_borrow: bool,
    /// `users.card_lost`. Renewal always refuses. Staff `force` does not skip it.
    pub card_lost: bool,
}

/// Precomputed account-level facts and the settings for this loan's medium.
#[derive(Debug, Clone)]
pub struct RenewalContext {
    /// Staff `force`: skip `account_blocked` and `fines_over_threshold`.
    /// Does not skip `card_lost`.
    pub bypass_account_gates: bool,
    pub fines_over_threshold: bool,
    /// Desk message used when the refusal is `fines_over_threshold`.
    pub fines_message: String,
    pub max_renewals: i16,
    pub renew_window_days: i16,
    pub now: DateTime<Utc>,
    pub zone: chrono_tz::Tz,
}

/// Why renewal is refused. Serialized as a snake_case code on `MyLoanView`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenewalRefusal {
    /// `users.card_lost`. Not skipped by staff `force`.
    CardLost,
    AccountBlocked,
    FinesOverThreshold,
    Withdrawn,
    ClaimedReturned,
    HoldWaiting,
    MaxRenewals,
    RenewalTooEarly,
}

impl RenewalRefusal {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::CardLost => crate::error::error_code::CARD_LOST,
            Self::AccountBlocked => "account_blocked",
            Self::FinesOverThreshold => "fines_over_threshold",
            Self::Withdrawn => "withdrawn",
            Self::ClaimedReturned => "claimed_returned",
            Self::HoldWaiting => "hold_waiting",
            Self::MaxRenewals => "max_renewals",
            Self::RenewalTooEarly => "renewal_too_early",
        }
    }

    fn static_message(self) -> &'static str {
        match self {
            Self::CardLost => crate::error::error_code::CARD_LOST_MESSAGE,
            Self::AccountBlocked => {
                "User account is not active or cannot borrow — use force=true to override"
            }
            Self::Withdrawn => WeedingStatus::RENEW_BLOCKED,
            Self::ClaimedReturned => {
                "Cannot renew a loan that is in the claims-returned queue — resolve the claim first"
            }
            Self::HoldWaiting => {
                "Cannot renew: another patron is waiting in the hold queue for this copy"
            }
            Self::FinesOverThreshold | Self::MaxRenewals | Self::RenewalTooEarly => "",
        }
    }

    /// Map a refusal onto the existing renew HTTP errors (422).
    #[must_use]
    pub fn into_app_error(self, loan: &RenewalLoan, ctx: &RenewalContext) -> AppError {
        match self {
            Self::CardLost => AppError::coded(
                422,
                crate::error::error_code::CARD_LOST,
                crate::error::error_code::CARD_LOST_MESSAGE,
            ),
            Self::RenewalTooEarly => {
                let opens = renewal_opens_at(loan.due_at, ctx.renew_window_days);
                AppError::RenewalTooEarly(renewal_too_early_message(opens, ctx.zone))
            }
            Self::FinesOverThreshold => AppError::RenewalRefused {
                code: self.code(),
                message: ctx.fines_message.clone(),
            },
            Self::MaxRenewals => AppError::RenewalRefused {
                code: self.code(),
                message: format!(
                    "Maximum renewals reached ({}/{})",
                    loan.renewal_count, ctx.max_renewals
                ),
            },
            other => AppError::RenewalRefused {
                code: other.code(),
                message: other.static_message().to_string(),
            },
        }
    }

    /// Library-local `YYYY-MM-DD` when this refusal is `renewal_too_early`.
    #[must_use]
    pub fn opens_at_iso(self, loan: &RenewalLoan, ctx: &RenewalContext) -> Option<String> {
        if self != Self::RenewalTooEarly {
            return None;
        }
        Some(format_patron_iso_date(
            ctx.zone,
            renewal_opens_at(loan.due_at, ctx.renew_window_days),
        ))
    }
}

/// Account reasons first, then loan reasons.
///
/// `Ok(())` means every gate in this list passed. The due-date calendar slide
/// ("renewal would not move the due date") stays on the write path only.
pub fn renewal_eligibility(
    loan: &RenewalLoan,
    user: &RenewalUser,
    ctx: &RenewalContext,
) -> Result<(), RenewalRefusal> {
    if user.card_lost {
        return Err(RenewalRefusal::CardLost);
    }
    if !ctx.bypass_account_gates && !user.can_borrow {
        return Err(RenewalRefusal::AccountBlocked);
    }
    if !ctx.bypass_account_gates && ctx.fines_over_threshold {
        return Err(RenewalRefusal::FinesOverThreshold);
    }
    if loan.withdrawn {
        return Err(RenewalRefusal::Withdrawn);
    }
    if loan.claimed_returned {
        return Err(RenewalRefusal::ClaimedReturned);
    }
    if loan.hold_waiting {
        return Err(RenewalRefusal::HoldWaiting);
    }
    if loan.renewal_count >= ctx.max_renewals {
        return Err(RenewalRefusal::MaxRenewals);
    }
    if ctx.now < renewal_opens_at(loan.due_at, ctx.renew_window_days) {
        return Err(RenewalRefusal::RenewalTooEarly);
    }
    Ok(())
}

/// Default days before the due date when a renewal becomes allowed.
pub const DEFAULT_RENEW_WINDOW_DAYS: i16 = 7;

/// First instant at which a renewal of a loan due at `due` is allowed.
#[must_use]
pub fn renewal_opens_at(due: DateTime<Utc>, window_days: i16) -> DateTime<Utc> {
    let days = i64::from(window_days.max(0));
    due - Duration::days(days)
}

/// Client message for [`AppError::RenewalTooEarly`].
///
/// The date is the library-local civil date, still written `YYYY-MM-DD` so the UI
/// can parse it (`renewalTooEarlyOpenDate`).
#[must_use]
pub fn renewal_too_early_message(opens_at: DateTime<Utc>, zone: chrono_tz::Tz) -> String {
    format!(
        "Renewal is too early. Renewal is possible from {}.",
        format_patron_iso_date(zone, opens_at)
    )
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};

    use super::{
        renewal_eligibility, renewal_opens_at, renewal_too_early_message, RenewalContext,
        RenewalLoan, RenewalRefusal, RenewalUser,
    };

    fn loan() -> RenewalLoan {
        RenewalLoan {
            renewal_count: 0,
            due_at: Utc.with_ymd_and_hms(2026, 11, 1, 15, 0, 0).unwrap(),
            withdrawn: false,
            claimed_returned: false,
            hold_waiting: false,
        }
    }

    fn user(can_borrow: bool) -> RenewalUser {
        RenewalUser {
            can_borrow,
            card_lost: false,
        }
    }

    fn ctx(now: DateTime<Utc>) -> RenewalContext {
        RenewalContext {
            bypass_account_gates: false,
            fines_over_threshold: false,
            fines_message: "fines".into(),
            max_renewals: 2,
            renew_window_days: 7,
            now,
            zone: chrono_tz::Europe::Paris,
        }
    }

    #[test]
    fn account_reasons_win_over_loan_reasons() {
        let mut row = loan();
        row.hold_waiting = true;
        let early = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let mut blocked = ctx(early);
        let refusal = renewal_eligibility(&row, &user(false), &blocked).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::AccountBlocked);

        blocked.fines_over_threshold = true;
        let refusal = renewal_eligibility(&row, &user(true), &blocked).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::FinesOverThreshold);

        blocked.bypass_account_gates = true;
        let refusal = renewal_eligibility(&row, &user(false), &blocked).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::HoldWaiting);
    }

    #[test]
    fn too_early_names_the_library_local_opening_date() {
        let row = loan();
        let now = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let context = ctx(now);
        let refusal = renewal_eligibility(&row, &user(true), &context).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::RenewalTooEarly);
        assert_eq!(
            refusal.opens_at_iso(&row, &context).as_deref(),
            Some("2026-10-25")
        );
        let opens = renewal_opens_at(row.due_at, 7);
        assert!(renewal_too_early_message(opens, chrono_tz::Europe::Paris).contains("2026-10-25"));
        assert_eq!(renewal_opens_at(row.due_at, 0), row.due_at);
    }

    #[test]
    fn loan_reasons_follow_withdrawn_then_claim_hold_cap_and_window() {
        let mut row = loan();
        row.withdrawn = true;
        row.claimed_returned = true;
        row.hold_waiting = true;
        row.renewal_count = 9;
        let early = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let context = ctx(early);
        let borrower = user(true);
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::Withdrawn
        );
        row.withdrawn = false;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::ClaimedReturned
        );
        row.claimed_returned = false;
        let refusal = renewal_eligibility(&row, &borrower, &context).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::HoldWaiting);
        assert!(refusal.opens_at_iso(&row, &context).is_none());
        row.hold_waiting = false;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::MaxRenewals
        );
        row.renewal_count = 0;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::RenewalTooEarly
        );
    }

    #[test]
    fn lost_card_blocks_renewal_even_when_force_skips_account_gates() {
        let row = loan();
        let now = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
        let mut context = ctx(now);
        context.bypass_account_gates = true;
        let borrower = RenewalUser {
            can_borrow: false,
            card_lost: true,
        };
        let refusal = renewal_eligibility(&row, &borrower, &context).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::CardLost);
        assert_eq!(refusal.code(), "card_lost");
        let err = refusal.into_app_error(&row, &context);
        match err {
            crate::error::AppError::Coded {
                status,
                code,
                message,
            } => {
                assert_eq!(status, 422);
                assert_eq!(code, "card_lost");
                assert_eq!(message, "Library card is reported lost or stolen");
            }
            other => panic!("expected coded card_lost, got {other:?}"),
        }
    }

    #[test]
    fn inside_the_window_with_no_blocker_can_renew() {
        let row = loan();
        let now = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
        assert!(renewal_eligibility(&row, &user(true), &ctx(now)).is_ok());
    }
}
