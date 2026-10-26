//! Exact library-card lookup for the circulation desk.

use chrono::Utc;

use crate::{
    error::{AppError, AppResult},
    models::{dto::patron_card::PatronLoanStatus, user::User},
    patron_eligibility::{patron_block_reasons, patron_checkout_blocked, PatronGateInput},
    repository::Repository,
    services::{calendar, fines::FinesService},
};

/// Resolved card scan. The HTTP layer picks the staff or desk body.
pub struct PatronCardLookup {
    pub user: User,
    pub loan_status: PatronLoanStatus,
}

/// Exact `users.barcode` match.
///
/// `agent_site_id` limits `readyHolds` to that pickup site. `None` counts every
/// site: the user row and the JWT have no site, so callers pass `None`.
pub async fn lookup_patron_by_card(
    repo: &Repository,
    fines: &FinesService,
    card_number: &str,
    agent_site_id: Option<i64>,
) -> AppResult<PatronCardLookup> {
    let card_number = card_number.trim();
    if card_number.is_empty() {
        return Err(AppError::Validation("Card number is required".to_string()));
    }

    let user_id = repo
        .users_id_by_exact_barcode(card_number)
        .await?
        .ok_or_else(|| AppError::NotFound("No patron with this card number".to_string()))?;
    let user = repo.users_get_by_id(user_id).await?;
    let counters = repo.patron_card_counters(user_id, agent_site_id).await?;
    let (_, _, max_total, _, _) = repo.resolve_loan_settings(user.public_type, None).await?;
    let fines_check = fines
        .check_unpaid_threshold(user_id, user.public_type)
        .await?;
    let unpaid_fines_cents = fines_check.unpaid.get();

    let zone = match repo.library_timezone_name().await {
        Ok(name) => calendar::parse_timezone(&name).unwrap_or_else(|_| calendar::default_zone()),
        Err(err) => {
            tracing::warn!(error = %err, "library timezone unavailable; using Europe/Paris");
            calendar::default_zone()
        }
    };
    let subscription_expires_at = user
        .expiry_at
        .map(|instant| calendar::local_date(zone, instant));
    let subscription_expired = user.expiry_at.is_some_and(|expiry| expiry < Utc::now());

    let reasons = patron_block_reasons(&PatronGateInput {
        can_borrow: user.can_borrow(),
        card_lost: counters.card_lost,
        subscription_expired,
        fines_block: fines_check.blocks_circulation(),
        active_loans: counters.active_loans,
        max_loans: Some(i64::from(max_total)),
        overdue_loans: counters.overdue_loans,
    });

    Ok(PatronCardLookup {
        user,
        loan_status: PatronLoanStatus {
            subscription_valid: !subscription_expired,
            subscription_expires_at,
            blocked: patron_checkout_blocked(&reasons),
            block_reasons: reasons,
            active_loans: counters.active_loans,
            overdue_loans: counters.overdue_loans,
            max_loans: Some(i32::from(max_total)),
            unpaid_fines_cents,
            ready_holds: counters.ready_holds,
        },
    })
}
