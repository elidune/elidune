//! One renewal decision for `POST /loans/{id}/renew` and `GET /me/loans`.
//!
//! Account facts in [`RenewalContext`] are computed once per request. Loan facts
//! are per copy. Staff `force` sets the bypass flags that
//! [`renewal_refusal_forceable`] allows for this caller. The account-type name
//! is not consulted. `subscription_expired` and `fines_over_threshold` use the
//! same rights rules as checkout
//! ([`crate::checkout_refusal::checkout_refusal_forceable`]): circulation override
//! write and a non-blank `overrideReason` for an expired subscription, loans write
//! for the unpaid-fine gate.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::{
    error::AppError,
    models::{
        item::WeedingStatus,
        user::{Rights, UserRights},
    },
    services::calendar::format_patron_iso_date,
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
    /// `users.expiry_at`. `None` is a membership with no end date.
    /// Strictly before [`RenewalContext::now`] is `subscription_expired`.
    /// Skipped only when this caller may force that code (circulation override write)
    /// and sent a non-blank `overrideReason`.
    pub subscription_expiry: Option<DateTime<Utc>>,
}

/// Precomputed account-level facts and the settings for this loan's medium.
#[derive(Debug, Clone)]
pub struct RenewalContext {
    /// Staff `force` with loans write: skip `account_blocked` and
    /// `fines_over_threshold`. Does not skip `card_lost` or `subscription_expired`.
    pub bypass_account_gates: bool,
    /// Staff `force` with circulation override write: skip `subscription_expired`.
    /// Same bar as checkout. Loans write alone does not set this. A reason is
    /// still required before the write.
    pub bypass_subscription_expired: bool,
    /// Staff `force` with loans write: skip `max_renewals` and `renewal_too_early`.
    /// Does not skip `withdrawn`, `claimed_returned`, or `hold_waiting`.
    pub bypass_renewal_limits: bool,
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
    /// `users.expiry_at` is before now. Skipped only with circulation override write.
    SubscriptionExpired,
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
            Self::SubscriptionExpired => crate::error::error_code::SUBSCRIPTION_EXPIRED,
            Self::FinesOverThreshold => crate::error::error_code::FINES_OVER_THRESHOLD,
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
            Self::SubscriptionExpired
            | Self::FinesOverThreshold
            | Self::MaxRenewals
            | Self::RenewalTooEarly => "",
        }
    }

    /// Map a refusal onto the existing renew HTTP errors (422).
    #[must_use]
    pub fn into_app_error(
        self,
        loan: &RenewalLoan,
        user: &RenewalUser,
        ctx: &RenewalContext,
    ) -> AppError {
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
            Self::SubscriptionExpired => AppError::RenewalRefused {
                code: self.code(),
                message: subscription_expired_message(user.subscription_expiry, ctx.zone),
            },
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
    if !ctx.bypass_subscription_expired
        && user
            .subscription_expiry
            .is_some_and(|expiry| expiry < ctx.now)
    {
        return Err(RenewalRefusal::SubscriptionExpired);
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
    if !ctx.bypass_renewal_limits && loan.renewal_count >= ctx.max_renewals {
        return Err(RenewalRefusal::MaxRenewals);
    }
    if !ctx.bypass_renewal_limits && ctx.now < renewal_opens_at(loan.due_at, ctx.renew_window_days)
    {
        return Err(RenewalRefusal::RenewalTooEarly);
    }
    Ok(())
}

/// Limit gates staff `force` may lift. Ignores [`RenewalContext::bypass_renewal_limits`].
#[must_use]
pub fn renewal_limits_hit(loan: &RenewalLoan, ctx: &RenewalContext) -> Vec<RenewalRefusal> {
    let mut hit = Vec::new();
    if loan.renewal_count >= ctx.max_renewals {
        hit.push(RenewalRefusal::MaxRenewals);
    }
    if ctx.now < renewal_opens_at(loan.due_at, ctx.renew_window_days) {
        hit.push(RenewalRefusal::RenewalTooEarly);
    }
    hit
}

/// Whether this caller may skip this renewal refusal.
///
/// `subscription_expired` and `fines_over_threshold` use
/// [`crate::checkout_refusal::checkout_refusal_forceable`]: circulation override
/// write for an expired subscription, loans write for the unpaid-fine gate.
/// `account_blocked`,
/// `max_renewals`, and `renewal_too_early` are forceable when
/// `caller_rights.loans_rights` is write — the same bar as
/// [`crate::models::user::UserClaims::require_write_loans`]. The account-type
/// name is not consulted. Every other renewal code is false, including
/// `card_lost`, `withdrawn`, `claimed_returned`, `hold_waiting`, and
/// `business_rule_violation`.
#[must_use]
pub fn renewal_refusal_forceable(code: &str, caller_rights: &UserRights) -> bool {
    if code == crate::error::error_code::SUBSCRIPTION_EXPIRED
        || code == crate::error::error_code::FINES_OVER_THRESHOLD
    {
        return crate::checkout_refusal::checkout_refusal_forceable(code, caller_rights);
    }
    if caller_rights.loans_rights.rank() < Rights::Write.rank() {
        return false;
    }
    matches!(
        code,
        "account_blocked" | "max_renewals" | crate::error::error_code::RENEWAL_TOO_EARLY
    )
}

/// Gates `force` may skip on this call. Built from [`renewal_refusal_forceable`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenewalOverride {
    /// Skip `account_blocked` and `fines_over_threshold`.
    pub account_gates: bool,
    /// Skip `subscription_expired`.
    pub subscription_expired: bool,
    /// Skip `max_renewals` and `renewal_too_early`.
    pub renewal_limits: bool,
}

impl RenewalOverride {
    /// `force` is false for every gate this caller may not skip.
    #[must_use]
    pub fn from_force(force: bool, caller_rights: &UserRights) -> Self {
        if !force {
            return Self::default();
        }
        Self {
            account_gates: renewal_refusal_forceable("account_blocked", caller_rights),
            subscription_expired: renewal_refusal_forceable(
                crate::error::error_code::SUBSCRIPTION_EXPIRED,
                caller_rights,
            ),
            renewal_limits: renewal_refusal_forceable("max_renewals", caller_rights),
        }
    }
}

/// Refusals this renewal skipped. Empty when no bypass flag is set.
/// Does not include `card_lost`, copy blocks, or a waiting hold.
#[must_use]
pub fn lifted_renewal_refusals(
    loan: &RenewalLoan,
    user: &RenewalUser,
    ctx: &RenewalContext,
) -> Vec<RenewalRefusal> {
    let mut lifted = Vec::new();
    if ctx.bypass_account_gates && !user.can_borrow {
        lifted.push(RenewalRefusal::AccountBlocked);
    }
    if ctx.bypass_subscription_expired
        && user
            .subscription_expiry
            .is_some_and(|expiry| expiry < ctx.now)
    {
        lifted.push(RenewalRefusal::SubscriptionExpired);
    }
    if ctx.bypass_account_gates && ctx.fines_over_threshold {
        lifted.push(RenewalRefusal::FinesOverThreshold);
    }
    if ctx.bypass_renewal_limits {
        lifted.extend(renewal_limits_hit(loan, ctx));
    }
    lifted
}

/// Locked renew outcome. `lifted` is empty unless staff `force` skipped a gate.
#[derive(Debug, Clone)]
pub struct LoanRenewalResult {
    pub expiry_at: DateTime<Utc>,
    pub renew_count: i16,
    /// Refusals this renewal skipped. See [`lifted_renewal_refusals`].
    pub lifted: Vec<RenewalRefusal>,
}

/// 422 body for every renewal refusal (`POST /loans/{id}/renew` and the barcode route).
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenewalRefusalBody {
    /// Stable machine-readable code.
    pub code: String,
    /// Always `Business Rule Violation` for renewal refusals.
    pub error: String,
    /// Desk-facing explanation. `renewal_too_early` includes the library-local opening date.
    pub message: String,
    /// Server-computed for this caller by [`renewal_refusal_forceable`].
    pub forceable: bool,
    /// Present and true when this refusal is forceable and `force` requires
    /// `overrideReason`. Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_required: Option<bool>,
}

/// Attach `forceable` for this caller. Errors that are not renewal refusals pass through.
#[must_use]
pub fn with_renewal_forceable(err: AppError, caller_rights: &UserRights) -> AppError {
    let Some((code, message)) = renewal_error_parts(&err) else {
        return err;
    };
    AppError::RenewalDenied {
        code,
        message,
        forceable: renewal_refusal_forceable(code, caller_rights),
    }
}

fn renewal_error_parts(err: &AppError) -> Option<(&'static str, String)> {
    match err {
        AppError::RenewalTooEarly(message) => {
            Some((crate::error::error_code::RENEWAL_TOO_EARLY, message.clone()))
        }
        AppError::RenewalRefused { code, message } => Some((*code, message.clone())),
        AppError::BusinessRule(message) => {
            Some((crate::error::error_code::BUSINESS_RULE, message.clone()))
        }
        AppError::Coded {
            status: 422,
            code,
            message,
        } if *code == crate::error::error_code::CARD_LOST => Some((*code, message.clone())),
        _ => None,
    }
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

/// Client message for an expired membership. The date is the library-local civil date.
pub(crate) fn subscription_expired_message(
    expiry: Option<DateTime<Utc>>,
    zone: chrono_tz::Tz,
) -> String {
    match expiry {
        Some(expiry) => format!(
            "User subscription expired on {} — use force=true to override",
            format_patron_iso_date(zone, expiry)
        ),
        None => "User subscription expired — use force=true to override".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeZone, Utc};

    use super::{
        lifted_renewal_refusals, renewal_eligibility, renewal_limits_hit, renewal_opens_at,
        renewal_refusal_forceable, renewal_too_early_message, with_renewal_forceable,
        RenewalContext, RenewalLoan, RenewalRefusal, RenewalUser,
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
            subscription_expiry: None,
        }
    }

    fn ctx(now: DateTime<Utc>) -> RenewalContext {
        RenewalContext {
            bypass_account_gates: false,
            bypass_subscription_expired: false,
            bypass_renewal_limits: false,
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
            subscription_expiry: Some(Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap()),
        };
        let refusal = renewal_eligibility(&row, &borrower, &context).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::CardLost);
        assert_eq!(refusal.code(), "card_lost");
        let err = refusal.into_app_error(&row, &borrower, &context);
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
    fn expired_subscription_blocks_before_loan_reasons_unless_force() {
        let mut row = loan();
        row.hold_waiting = true;
        let now = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
        let mut context = ctx(now);
        context.fines_over_threshold = true;
        let expired = Utc.with_ymd_and_hms(2026, 1, 15, 11, 0, 0).unwrap();
        let borrower = RenewalUser {
            can_borrow: true,
            card_lost: false,
            subscription_expiry: Some(expired),
        };
        let refusal = renewal_eligibility(&row, &borrower, &context).unwrap_err();
        assert_eq!(refusal, RenewalRefusal::SubscriptionExpired);
        assert_eq!(refusal.code(), "subscription_expired");
        let err = refusal.into_app_error(&row, &borrower, &context);
        match err {
            crate::error::AppError::RenewalRefused { code, message } => {
                assert_eq!(code, "subscription_expired");
                assert!(message.contains("2026-01-15"), "{message}");
                assert!(message.contains("force=true"), "{message}");
            }
            other => panic!("expected subscription_expired, got {other:?}"),
        }

        let blocked = RenewalUser {
            can_borrow: false,
            ..borrower
        };
        assert_eq!(
            renewal_eligibility(&row, &blocked, &context).unwrap_err(),
            RenewalRefusal::AccountBlocked
        );

        context.bypass_account_gates = true;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::SubscriptionExpired,
            "loans-write account bypass does not skip an expired subscription"
        );
        context.bypass_subscription_expired = true;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::HoldWaiting
        );

        row.hold_waiting = false;
        assert!(renewal_eligibility(&row, &borrower, &context).is_ok());
        context.bypass_account_gates = false;
        context.fines_over_threshold = false;
        let open_ended = RenewalUser {
            subscription_expiry: None,
            ..borrower
        };
        assert!(renewal_eligibility(&row, &open_ended, &context).is_ok());
        let still_current = RenewalUser {
            subscription_expiry: Some(now),
            ..borrower
        };
        assert!(renewal_eligibility(&row, &still_current, &context).is_ok());
    }

    #[test]
    fn force_lifts_the_cap_and_the_early_window_but_not_a_copy_or_hold_block() {
        let mut row = loan();
        row.renewal_count = 2;
        let early = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let mut context = ctx(early);
        let borrower = user(true);
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::MaxRenewals
        );
        context.bypass_renewal_limits = true;
        assert!(renewal_eligibility(&row, &borrower, &context).is_ok());
        assert_eq!(
            renewal_limits_hit(&row, &context),
            vec![RenewalRefusal::MaxRenewals, RenewalRefusal::RenewalTooEarly]
        );
        assert_eq!(
            lifted_renewal_refusals(&row, &borrower, &context),
            vec![RenewalRefusal::MaxRenewals, RenewalRefusal::RenewalTooEarly]
        );
        let expired = RenewalUser {
            subscription_expiry: Some(Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap()),
            ..borrower
        };
        let blocked = RenewalUser {
            can_borrow: false,
            ..expired
        };
        context.fines_over_threshold = true;
        context.bypass_account_gates = true;
        assert_eq!(
            lifted_renewal_refusals(&row, &blocked, &context),
            vec![
                RenewalRefusal::AccountBlocked,
                RenewalRefusal::FinesOverThreshold,
                RenewalRefusal::MaxRenewals,
                RenewalRefusal::RenewalTooEarly,
            ]
        );
        context.bypass_subscription_expired = true;
        assert_eq!(
            lifted_renewal_refusals(&row, &blocked, &context)[1],
            RenewalRefusal::SubscriptionExpired
        );
        row.hold_waiting = true;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::HoldWaiting
        );
        row.hold_waiting = false;
        row.withdrawn = true;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::Withdrawn
        );
        row.withdrawn = false;
        row.claimed_returned = true;
        assert_eq!(
            renewal_eligibility(&row, &borrower, &context).unwrap_err(),
            RenewalRefusal::ClaimedReturned
        );
    }

    #[test]
    fn forceable_follows_loans_write_and_ignores_every_other_code() {
        use crate::checkout_refusal::checkout_refusal_forceable;
        use crate::models::user::UserRights;

        let desk = UserRights {
            loans_rights: crate::models::user::Rights::Write,
            ..UserRights::default()
        };
        let settings = UserRights {
            settings_rights: crate::models::user::Rights::Write,
            circulation_override_rights: crate::models::user::Rights::None,
            ..UserRights::default()
        };
        let override_right = UserRights {
            circulation_override_rights: crate::models::user::Rights::Write,
            settings_rights: crate::models::user::Rights::None,
            ..UserRights::default()
        };
        let patron = UserRights::default();
        for code in [
            "account_blocked",
            "fines_over_threshold",
            "max_renewals",
            "renewal_too_early",
        ] {
            assert!(renewal_refusal_forceable(code, &desk), "{code}");
            assert!(!renewal_refusal_forceable(code, &patron), "{code}");
            assert!(!renewal_refusal_forceable(code, &settings), "{code}");
        }
        let both = UserRights {
            loans_rights: crate::models::user::Rights::Write,
            circulation_override_rights: crate::models::user::Rights::Write,
            ..UserRights::default()
        };
        assert!(
            !renewal_refusal_forceable("subscription_expired", &desk),
            "loans write alone cannot force an expired subscription"
        );
        assert!(!renewal_refusal_forceable(
            "subscription_expired",
            &settings
        ));
        assert!(renewal_refusal_forceable(
            "subscription_expired",
            &override_right
        ));
        assert!(renewal_refusal_forceable("subscription_expired", &both));
        assert!(!renewal_refusal_forceable("subscription_expired", &patron));
        for rights in [&desk, &settings, &override_right, &both, &patron] {
            assert_eq!(
                renewal_refusal_forceable("subscription_expired", rights),
                checkout_refusal_forceable("subscription_expired", rights)
            );
            assert_eq!(
                renewal_refusal_forceable("fines_over_threshold", rights),
                checkout_refusal_forceable("fines_over_threshold", rights)
            );
        }
        for code in [
            "card_lost",
            "withdrawn",
            "claimed_returned",
            "hold_waiting",
            "business_rule_violation",
        ] {
            assert!(!renewal_refusal_forceable(code, &desk), "{code}");
            assert!(!renewal_refusal_forceable(code, &patron), "{code}");
        }

        let read_only = UserRights {
            loans_rights: crate::models::user::Rights::Read,
            ..UserRights::default()
        };
        assert!(!renewal_refusal_forceable("max_renewals", &read_only));

        let row = loan();
        let early = Utc.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap();
        let context = ctx(early);
        let borrower = user(true);
        let refusal = renewal_eligibility(&row, &borrower, &context).unwrap_err();
        let denied =
            with_renewal_forceable(refusal.into_app_error(&row, &borrower, &context), &desk);
        match denied {
            crate::error::AppError::RenewalDenied {
                code,
                message,
                forceable,
            } => {
                assert_eq!(code, "renewal_too_early");
                assert!(forceable);
                assert!(message.contains("2026-10-25"), "{message}");
            }
            other => panic!("expected renewal denial, got {other:?}"),
        }
        let patron_denied = with_renewal_forceable(
            crate::error::AppError::coded(
                422,
                crate::error::error_code::CARD_LOST,
                crate::error::error_code::CARD_LOST_MESSAGE,
            ),
            &desk,
        );
        match patron_denied {
            crate::error::AppError::RenewalDenied {
                code, forceable, ..
            } => {
                assert_eq!(code, "card_lost");
                assert!(!forceable);
            }
            other => panic!("expected card_lost denial, got {other:?}"),
        }
    }

    #[test]
    fn inside_the_window_with_no_blocker_can_renew() {
        let row = loan();
        let now = Utc.with_ymd_and_hms(2026, 10, 26, 12, 0, 0).unwrap();
        assert!(renewal_eligibility(&row, &user(true), &ctx(now)).is_ok());
    }
}
