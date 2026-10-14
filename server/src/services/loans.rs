//! Loan management service

use chrono::{DateTime, Utc};

use std::sync::Arc;

use crate::{
    checkout_refusal::{
        force_skips_checkout_refusal, normalized_override_reason, push_checkout_refusal,
        CheckoutProgress, CheckoutRefusal,
    },
    error::{error_code, AppError, AppResult},
    marc::{marc_record_for_loan_export, MarcRecord},
    models::{
        dto::loans::{LoanSettingsDto, UpdateLoanSettingsRequest},
        loan::{
            CreateLoan, HoldReadyEmailOutcome, LoanCreateOutcome, LoanDetails,
            LoanMarcExportEncoding, LoanMarcExportFormat, LoanReturnOutcome, LoanSettingsRenewAt,
            LOANS_MARC_EXPORT_MAX,
        },
        user::{UserRights, UserStatus},
        Loan,
    },
    renewal::{
        renewal_eligibility, subscription_expired_message, RenewalContext, RenewalOverride,
        RenewalRefusal, RenewalUser,
    },
    repository::LoansServiceRepository,
    services::{
        audit::{self, AuditLogMeta, AuditService},
        email::EmailService,
        event_bus::EventBus,
        fines::FinesService,
    },
};
use z3950_rs::marc_rs::{BinaryWriter, Encoding as MarcEncoding, MarcFormat, XmlWriter};

/// Desk scan of a barcode: a loan return, a `to_pick` check-in, a known available
/// shelf copy with no active loan, or a warning when the copy is lost, damaged,
/// missing, to-search, or withdrawn.
#[derive(Debug, Clone)]
pub struct ItemCheckIn {
    pub loan: Option<LoanDetails>,
    pub hold: Option<crate::models::hold::Hold>,
    pub transit_started: bool,
    pub destination_site_id: Option<i64>,
    pub destination_site_name: Option<String>,
    /// Set when the copy is lost. Status is unchanged.
    pub lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
    /// Informational note for an available copy that was not on loan, or a
    /// warning when the copy's status blocks the scan.
    pub notice: Option<crate::models::dto::circulation::DeskNotice>,
    /// Set with the blocked-status warning. Current circulation exception.
    pub circulation_status: Option<crate::models::circulation::CirculationStatus>,
    /// Set with the blocked-status warning. Current weeding status.
    pub weeding_status: Option<crate::models::item::WeedingStatus>,
    /// Present when this scan set a title hold aside, or repeated that assignment.
    pub set_aside: Option<SetAsideOutcome>,
}

/// Outcome of a title-hold set-aside on the barcode return.
#[derive(Debug, Clone, Copy)]
pub struct SetAsideOutcome {
    pub hold_status: crate::models::hold::HoldStatus,
    pub transit_id: Option<i64>,
    pub pickup_site_id: Option<i64>,
}

/// `POST /loans/items/{barcode}/return` body that assigns the oldest pending
/// unassigned title hold to the scanned copy. `hold_id` must be that hold.
#[derive(Debug, Clone, Copy)]
pub struct TitleHoldSetAside {
    pub hold_id: i64,
    pub item_id: i64,
}

/// Result of `POST /loans/{id}/return`. `loan` is absent when the loan was already
/// closed by a lost declaration and the copy is still lost.
#[derive(Debug, Clone)]
pub struct DeskReturn {
    pub loan: Option<LoanDetails>,
    pub lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
}

/// Result of the unpaid-fine gate for one checkout.
enum CheckoutFineGate {
    /// Balance is at or under the threshold, or amount blocking is off (`none`).
    Clear,
    /// This loan may skip `fines_over_threshold`. Audited only after it commits.
    Overridden,
    /// Caller may not skip the gate, or `force` was not set.
    Refused(CheckoutRefusal),
}

#[derive(Clone)]
pub struct LoansService {
    repository: Arc<dyn LoansServiceRepository>,
    fines: FinesService,
    audit: AuditService,
    email: EmailService,
    events: EventBus,
}

impl LoansService {
    async fn library_zone(&self) -> chrono_tz::Tz {
        match self.repository.library_timezone_name().await {
            Ok(name) => crate::services::calendar::parse_timezone(&name)
                .unwrap_or_else(|_| crate::services::calendar::default_zone()),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "library timezone unavailable; using Europe/Paris"
                );
                crate::services::calendar::default_zone()
            }
        }
    }

    pub fn new(
        repository: Arc<dyn LoansServiceRepository>,
        fines: FinesService,
        audit: AuditService,
        email: EmailService,
        events: EventBus,
    ) -> Self {
        Self {
            repository,
            fines,
            audit,
            email,
            events,
        }
    }

    /// Get active loans for a user (paginated). `page` and `per_page` must be valid (≥1, capped by caller).
    pub async fn get_user_loans(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<LoanDetails>, i64)> {
        self.repository.users_get_by_id(user_id).await?;
        self.repository
            .loans_get_for_user(user_id, page, per_page)
            .await
    }

    /// Get archived (returned) loans for a user (paginated).
    pub async fn get_user_archived_loans(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<LoanDetails>, i64)> {
        self.repository.users_get_by_id(user_id).await?;
        self.repository
            .loans_archives_get_for_user(user_id, page, per_page)
            .await
    }

    /// Unpaid fines strictly above the effective threshold.
    /// A null threshold (`none`, or inherit of a global `none`) does not block.
    /// `Overridden` means this call may skip the gate. The audit is written only
    /// after the loan commits, and only for that loan.
    async fn checkout_fine_refusal(
        &self,
        user_id: i64,
        public_type_id: Option<i64>,
        force: bool,
        caller_rights: &UserRights,
    ) -> AppResult<CheckoutFineGate> {
        let check = self
            .fines
            .check_unpaid_threshold(user_id, public_type_id)
            .await?;
        if !check.blocks_circulation() {
            return Ok(CheckoutFineGate::Clear);
        }
        if force_skips_checkout_refusal(force, error_code::FINES_OVER_THRESHOLD, caller_rights) {
            return Ok(CheckoutFineGate::Overridden);
        }
        Ok(CheckoutFineGate::Refused(CheckoutRefusal::new(
            error_code::FINES_OVER_THRESHOLD,
            check.desk_block_message(),
            caller_rights,
        )))
    }

    /// Create a new loan (borrow an item).
    ///
    /// Enforces user-level rules before delegating to the repository:
    /// - a deleted or inactive account is `422` `business_rule_violation` (`force` does not skip it)
    /// - a lost or stolen library card is always rejected (`422` `card_lost`), including `force`
    /// - expired subscriptions are `422` `subscription_expired` unless `force` skips that code
    ///   for a caller with circulation override write and a non-blank `overrideReason`
    /// - unpaid fines strictly above the effective threshold are `422` `fines_over_threshold`
    ///   unless `force` skips that code for a caller with loans write (a null
    ///   threshold, including mode `none`, does not block on amount).
    ///   `reminders.accrue_fines` false ignores every stored threshold, so the
    ///   refusal is not offered. The skip
    ///   applies to this loan only. It is audited after the write commits as
    ///   `loan.fine_threshold_overridden` with `{ reason, loanId, patron_id }`.
    ///
    /// Forceable patron refusals are passed into the repository. A later non-forceable item
    /// gate (another patron's hold, a circulation status that needs a desk action, a withdrawn
    /// copy, transit) is returned first. `force` never closes or replaces an open loan.
    pub async fn create_loan(
        &self,
        loan: CreateLoan,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        caller_rights: &UserRights,
    ) -> AppResult<LoanCreateOutcome> {
        let user = self.repository.users_get_by_id(loan.user_id).await?;

        let mut progress =
            CheckoutProgress::new(normalized_override_reason(loan.override_reason.as_deref()));
        let mut subscription_overridden = false;
        let mut fine_overridden = false;
        let status = user.status.unwrap_or(UserStatus::Active);
        if status == UserStatus::Deleted {
            push_checkout_refusal(
                &mut progress,
                loan.force,
                CheckoutRefusal::business(
                    "Cannot create a loan for a deleted user account",
                    caller_rights,
                ),
            )?;
        }

        if !user.can_borrow() {
            push_checkout_refusal(
                &mut progress,
                loan.force,
                CheckoutRefusal::business(
                    "User account is not active or cannot borrow",
                    caller_rights,
                ),
            )?;
        }

        if self.repository.users_card_lost(user.id).await? {
            push_checkout_refusal(
                &mut progress,
                loan.force,
                CheckoutRefusal::new(
                    error_code::CARD_LOST,
                    error_code::CARD_LOST_MESSAGE,
                    caller_rights,
                ),
            )?;
        }

        if let Some(expiry_at) = user.expiry_at {
            if expiry_at < Utc::now() {
                let skipped = force_skips_checkout_refusal(
                    loan.force,
                    error_code::SUBSCRIPTION_EXPIRED,
                    caller_rights,
                );
                if skipped && progress.override_reason.is_some() {
                    subscription_overridden = true;
                } else {
                    let zone = crate::services::calendar::parse_timezone(
                        &self.repository.library_timezone_name().await?,
                    )
                    .unwrap_or_else(|_| crate::services::calendar::default_zone());
                    push_checkout_refusal(
                        &mut progress,
                        loan.force,
                        CheckoutRefusal::new(
                            error_code::SUBSCRIPTION_EXPIRED,
                            subscription_expired_message(Some(expiry_at), zone),
                            caller_rights,
                        ),
                    )?;
                }
            }
        }

        match self
            .checkout_fine_refusal(loan.user_id, user.public_type, loan.force, caller_rights)
            .await?
        {
            CheckoutFineGate::Clear => {}
            CheckoutFineGate::Overridden => fine_overridden = true,
            CheckoutFineGate::Refused(fine_refusal) => {
                push_checkout_refusal(&mut progress, loan.force, fine_refusal)?;
            }
        }

        let outcome = self
            .repository
            .loans_create(&loan, &mut progress, caller_rights)
            .await?;
        if fine_overridden {
            self.audit.log(
                audit::event::LOAN_FINE_THRESHOLD_OVERRIDDEN,
                audit_actor,
                Some("loan"),
                Some(outcome.loan_id),
                client_ip.clone(),
                Some(serde_json::json!({
                    "reason": error_code::FINES_OVER_THRESHOLD,
                    "loanId": outcome.loan_id,
                    "patron_id": loan.user_id,
                })),
                AuditLogMeta::success(),
            );
        }
        if subscription_overridden {
            self.audit.log(
                audit::event::LOAN_SUBSCRIPTION_EXPIRED_OVERRIDDEN,
                audit_actor,
                Some("loan"),
                Some(outcome.loan_id),
                client_ip.clone(),
                Some(serde_json::json!({
                    "patron_id": loan.user_id,
                    "item_id": loan.item_id,
                    "loan_id": outcome.loan_id,
                    "code": error_code::SUBSCRIPTION_EXPIRED,
                    "overrideReason": progress.override_reason,
                })),
                AuditLogMeta::success(),
            );
        }
        if let Some(hold_id) = outcome.preserved_hold_id {
            self.audit.log(
                audit::event::LOAN_HELD_FOR_OTHER_OVERRIDDEN,
                audit_actor,
                Some("loan"),
                Some(outcome.loan_id),
                client_ip.clone(),
                Some(serde_json::json!({
                    "reason": progress.override_reason,
                    "overrideReason": progress.override_reason,
                    "holdId": hold_id,
                    "patron_id": loan.user_id,
                    "loanId": outcome.loan_id,
                    "code": error_code::ITEM_HELD_FOR_OTHER,
                })),
                AuditLogMeta::success(),
            );
        }
        if let Some(item_id) = loan.item_id {
            self.events
                .loan_created(outcome.loan_id, loan.user_id, item_id);
        }
        if let Some(hold_id) = outcome.fulfilled_hold_id {
            self.audit.log(
                audit::event::HOLD_FULFILLED,
                audit_actor,
                Some("hold"),
                Some(hold_id),
                client_ip.clone(),
                Some(serde_json::json!({
                    "loan_id": outcome.loan_id,
                    "user_id": loan.user_id,
                    "item_id": loan.item_id,
                    "trigger": "checkout",
                })),
                AuditLogMeta::success(),
            );
        }
        Ok(outcome)
    }

    /// Return a borrowed item. A lost copy is reported and left lost.
    /// When the loan was already closed by the lost declaration, the same alert
    /// is returned without writing the item.
    pub async fn return_loan(
        &self,
        loan_id: i64,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<DeskReturn> {
        match self.repository.loans_return(loan_id).await {
            Ok(mut outcome) => {
                outcome.hold_ready_email = self.send_hold_ready_email(&outcome).await;
                self.publish_return_events(&outcome);
                self.audit_return_side_effects(audit_actor, client_ip.clone(), &outcome);
                let item_id = outcome.details.item_id;
                self.audit_lost_seen(audit_actor, client_ip, item_id, outcome.lost.as_ref());
                Ok(DeskReturn {
                    loan: Some(outcome.details),
                    lost: outcome.lost,
                })
            }
            Err(AppError::NotFound(_)) => {
                if let Some(found) = self
                    .repository
                    .loans_lost_notice_for_closed_loan(loan_id)
                    .await?
                {
                    self.audit_lost_seen(
                        audit_actor,
                        client_ip,
                        found.item_id,
                        Some(&found.notice),
                    );
                    return Ok(DeskReturn {
                        loan: None,
                        lost: Some(found.notice),
                    });
                }
                Err(AppError::NotFound(format!("Loan {loan_id} not found")))
            }
            Err(e) => Err(e),
        }
    }

    /// Check in a barcode. An active loan is returned. A shelf copy assigned as
    /// `to_pick` becomes `ready` (same site, patron notified) or starts transit.
    /// An available copy that is not on loan succeeds with an informational notice
    /// and, when a pending hold can take it, sets that hold aside.
    /// `set_aside` assigns the oldest pending unassigned title hold when `holdId`
    /// is that queue head. A newer hold is refused. Same site becomes `ready`;
    /// a different pickup site starts transit and stays pending until arrival.
    /// A lost, damaged, missing, to-search, or withdrawn copy is left unchanged.
    pub async fn return_loan_by_item(
        &self,
        item_identification: &str,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        set_aside: Option<TitleHoldSetAside>,
    ) -> AppResult<ItemCheckIn> {
        if let Some(copy) = self
            .repository
            .loans_return_scan_copy(item_identification)
            .await?
        {
            if copy.blocks_return_scan() {
                self.audit.log(
                    audit::event::ITEM_RETURN_BLOCKED_STATUS,
                    audit_actor,
                    Some("item"),
                    Some(copy.item_id),
                    client_ip,
                    Some(serde_json::json!({
                        "barcode": item_identification,
                        "notice": crate::error::error_code::ITEM_NOT_AVAILABLE_STATUS,
                        "circulationStatus": copy.circulation_status,
                        "weedingStatus": copy.weeding_status,
                    })),
                    AuditLogMeta::success(),
                );
                return Ok(ItemCheckIn {
                    loan: None,
                    hold: None,
                    transit_started: false,
                    destination_site_id: None,
                    destination_site_name: None,
                    lost: None,
                    notice: Some(
                        crate::models::dto::circulation::DeskNotice::item_not_available_status(),
                    ),
                    circulation_status: Some(copy.circulation_status),
                    weeding_status: Some(copy.weeding_status),
                    set_aside: None,
                });
            }
        }
        match self
            .repository
            .loans_get_by_item_identification(item_identification)
            .await
        {
            Ok(loan) => {
                let mut outcome = self.repository.loans_return(loan.id).await?;
                outcome.hold_ready_email = self.send_hold_ready_email(&outcome).await;
                self.publish_return_events(&outcome);
                self.audit_return_side_effects(audit_actor, client_ip.clone(), &outcome);
                let item_id = outcome.details.item_id;
                self.audit_lost_seen(audit_actor, client_ip, item_id, outcome.lost.as_ref());
                Ok(ItemCheckIn {
                    loan: Some(outcome.details),
                    hold: None,
                    transit_started: false,
                    destination_site_id: None,
                    destination_site_name: None,
                    lost: outcome.lost,
                    notice: None,
                    circulation_status: None,
                    weeding_status: None,
                    set_aside: None,
                })
            }
            Err(AppError::NotFound(_)) => {
                if let Some(found) = self
                    .repository
                    .loans_lost_scan_by_barcode(item_identification)
                    .await?
                {
                    self.audit_lost_seen(
                        audit_actor,
                        client_ip,
                        found.item_id,
                        Some(&found.notice),
                    );
                    return Ok(ItemCheckIn {
                        loan: None,
                        hold: None,
                        transit_started: false,
                        destination_site_id: None,
                        destination_site_name: None,
                        lost: Some(found.notice),
                        notice: None,
                        circulation_status: None,
                        weeding_status: None,
                        set_aside: None,
                    });
                }
                if let Some(target) = set_aside {
                    let done = self
                        .repository
                        .loans_set_aside_unassigned_title_hold(
                            item_identification,
                            target.hold_id,
                            target.item_id,
                        )
                        .await?;
                    return Ok(self
                        .not_on_loan_check_in(done, item_identification, audit_actor, client_ip)
                        .await);
                }
                if let Some(done) = self
                    .repository
                    .loans_complete_to_pick_by_identification(item_identification)
                    .await?
                {
                    if !done.transit_started {
                        self.notify_picked_hold_ready(
                            &done.hold,
                            done.title.as_deref().unwrap_or("(unknown title)"),
                            done.barcode.as_deref(),
                            audit_actor,
                            client_ip,
                            "pick_check_in",
                        )
                        .await;
                    }
                    return Ok(ItemCheckIn {
                        loan: None,
                        hold: Some(done.hold),
                        transit_started: done.transit_started,
                        destination_site_id: done.destination_site_id,
                        destination_site_name: done.destination_site_name,
                        lost: None,
                        notice: None,
                        circulation_status: None,
                        weeding_status: None,
                        set_aside: None,
                    });
                }
                if let Some(done) = self
                    .repository
                    .loans_check_in_shelf_copy(item_identification)
                    .await?
                {
                    return Ok(self
                        .not_on_loan_check_in(done, item_identification, audit_actor, client_ip)
                        .await);
                }
                Err(AppError::NotFound(format!(
                    "No active loan found for item {item_identification}"
                )))
            }
            Err(e) => Err(e),
        }
    }

    async fn not_on_loan_check_in(
        &self,
        done: crate::models::dto::holds::ShelfCheckIn,
        item_identification: &str,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> ItemCheckIn {
        if done.newly_ready {
            if let Some(hold) = done.hold.as_ref() {
                if !done.transit_started && hold.status == crate::models::hold::HoldStatus::Ready {
                    self.notify_picked_hold_ready(
                        hold,
                        done.title.as_deref().unwrap_or("(unknown title)"),
                        done.barcode.as_deref(),
                        audit_actor,
                        client_ip.clone(),
                        "item_not_on_loan",
                    )
                    .await;
                }
            }
        }
        self.audit.log(
            audit::event::ITEM_NOT_ON_LOAN,
            audit_actor,
            Some("item"),
            Some(done.item_id),
            client_ip,
            Some(serde_json::json!({
                "barcode": item_identification,
                "notice": crate::error::error_code::ITEM_NOT_ON_LOAN,
                "hold_id": done.hold.as_ref().map(|hold| hold.id),
                "transit_started": done.transit_started,
            })),
            AuditLogMeta::success(),
        );
        let set_aside = done.report_set_aside.then(|| SetAsideOutcome {
            hold_status: done
                .hold
                .as_ref()
                .map(|hold| hold.status)
                .unwrap_or(crate::models::hold::HoldStatus::Pending),
            transit_id: done.transit_id,
            pickup_site_id: done.pickup_site_id,
        });
        ItemCheckIn {
            loan: None,
            hold: done.hold,
            transit_started: done.transit_started,
            destination_site_id: done.destination_site_id,
            destination_site_name: done.destination_site_name,
            lost: None,
            notice: Some(crate::models::dto::circulation::DeskNotice::item_not_on_loan()),
            circulation_status: None,
            weeding_status: None,
            set_aside,
        }
    }

    async fn notify_picked_hold_ready(
        &self,
        hold: &crate::models::hold::Hold,
        title: &str,
        barcode: Option<&str>,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        trigger: &'static str,
    ) {
        self.audit.log(
            audit::event::HOLD_READY,
            audit_actor,
            Some("hold"),
            Some(hold.id),
            client_ip.clone(),
            Some(serde_json::json!({
                "user_id": hold.user_id,
                "item_id": hold.item_id,
                "expires_at": hold.expires_at,
                "trigger": trigger,
            })),
            AuditLogMeta::success(),
        );
        if let Some(item_id) = hold.item_id {
            self.events.hold_ready(hold.id, hold.user_id, item_id);
        }
        let contact = self
            .repository
            .users_hold_ready_contact(hold.user_id)
            .await
            .ok()
            .flatten();
        let zone = self.library_zone().await;
        if let Err(e) = crate::hold_email::send_hold_ready_simple(
            &self.email,
            contact,
            hold,
            title,
            barcode,
            zone,
        )
        .await
        {
            tracing::warn!(
                target: "loans",
                error = %e,
                hold_id = hold.id,
                "Failed to queue hold ready email after pick check-in"
            );
        }
    }

    async fn send_hold_ready_email(
        &self,
        outcome: &LoanReturnOutcome,
    ) -> Option<HoldReadyEmailOutcome> {
        let hold = outcome.readied_hold.as_ref()?;
        let contact = self
            .repository
            .users_hold_ready_contact(hold.user_id)
            .await
            .ok()
            .flatten();
        let to = contact
            .as_ref()
            .and_then(|c| c.email.as_deref().map(str::trim))
            .filter(|s| !s.is_empty())?;
        let zone = self.library_zone().await;
        match crate::hold_email::send_hold_ready(
            &self.email,
            contact.clone(),
            hold,
            &outcome.details,
            zone,
        )
        .await
        {
            Ok(()) => Some(HoldReadyEmailOutcome {
                email: Some(to.to_string()),
                send_error: None,
            }),
            Err(e) => {
                tracing::warn!(
                    target: "loans",
                    error = %e,
                    hold_id = hold.id,
                    "Failed to queue hold ready email"
                );
                Some(HoldReadyEmailOutcome {
                    email: Some(to.to_string()),
                    send_error: Some(e.to_string()),
                })
            }
        }
    }

    fn publish_return_events(&self, outcome: &LoanReturnOutcome) {
        let user_id = outcome.details.user.as_ref().map(|u| u.id).unwrap_or(0);
        self.events
            .loan_returned(outcome.details.id, user_id, outcome.details.item_id);
        if let Some(ref hold) = outcome.readied_hold {
            if let Some(item_id) = hold.item_id {
                self.events.hold_ready(hold.id, hold.user_id, item_id);
            }
        }
    }

    fn audit_lost_seen(
        &self,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        item_id: i64,
        lost: Option<&crate::models::dto::circulation::LostItemOnReturn>,
    ) {
        let Some(lost) = lost else {
            return;
        };
        self.audit.log(
            audit::event::ITEM_LOST_FOUND_SEEN,
            audit_actor,
            Some("item"),
            Some(item_id),
            client_ip,
            Some(serde_json::json!({
                "wasLost": lost.was_lost,
                "fineId": lost.fine_id.map(|id| id.to_string()),
                "fineStatus": lost.fine_status,
                "fineAmount": lost.fine_amount,
                "paidAmount": lost.paid_amount,
            })),
            AuditLogMeta::success(),
        );
    }

    fn audit_return_side_effects(
        &self,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        outcome: &LoanReturnOutcome,
    ) {
        let Some(ref hold) = outcome.readied_hold else {
            return;
        };

        self.audit.log(
            audit::event::HOLD_READY,
            audit_actor,
            Some("hold"),
            Some(hold.id),
            client_ip.clone(),
            Some(serde_json::json!({
                "user_id": hold.user_id,
                "item_id": hold.item_id,
                "expires_at": hold.expires_at,
                "trigger": "loan_return",
            })),
            AuditLogMeta::success(),
        );

        let Some(ref email_outcome) = outcome.hold_ready_email else {
            return;
        };

        let biblio_id = outcome.details.biblio.id;
        let title = outcome
            .details
            .biblio
            .title
            .as_deref()
            .unwrap_or("(unknown title)");

        if let Some(ref err) = email_outcome.send_error {
            self.audit.log(
                audit::event::EMAIL_HOLD_READY_SENT,
                audit_actor,
                Some("hold"),
                Some(hold.id),
                client_ip.clone(),
                Some(serde_json::json!({
                    "user_id": hold.user_id,
                    "email": email_outcome.email,
                    "biblio_id": biblio_id,
                    "title": title,
                    "trigger": "loan_return",
                })),
                AuditLogMeta::failure_background("email_delivery_failed", err.clone()),
            );
        } else if email_outcome.email.is_some() {
            self.audit.log(
                audit::event::EMAIL_HOLD_READY_SENT,
                audit_actor,
                Some("user"),
                Some(hold.user_id),
                client_ip,
                Some(serde_json::json!({
                    "hold_id": hold.id,
                    "email": email_outcome.email,
                    "biblio_id": biblio_id,
                    "title": title,
                    "trigger": "loan_return",
                })),
                AuditLogMeta::success(),
            );
        }
    }

    /// Get a loan by id
    pub async fn get_loan(&self, loan_id: i64) -> AppResult<Loan> {
        self.repository.loans_get_by_id(loan_id).await
    }

    /// Patron linked to an active loan (for circulation desk lookups).
    pub async fn get_loan_borrower(
        &self,
        loan_id: i64,
    ) -> AppResult<crate::models::user::UserShort> {
        self.repository.loans_get_borrower_for_loan(loan_id).await
    }

    /// Borrowers of open loans, keyed by item id.
    pub async fn active_borrowers_by_item_ids(
        &self,
        item_ids: &[i64],
    ) -> AppResult<std::collections::HashMap<i64, crate::models::user::UserShort>> {
        self.repository
            .loans_active_borrowers_by_item_ids(item_ids)
            .await
    }

    /// Renew a loan. `force` skips a gate only when
    /// [`crate::renewal::renewal_refusal_forceable`] is true for this caller.
    /// Loans write lifts account status, the unpaid-fine gate, `max_renewals`,
    /// and `renewal_too_early`. `subscription_expired` needs circulation override
    /// write, the same rule as checkout, and a non-blank `overrideReason`.
    /// A lost card, a withdrawn copy, a claims-returned copy, and a waiting hold
    /// stay `422`. Each skipped gate is audited after the write commits.
    /// The payload is `{ reason, loanId, patron_id }`, plus `overrideReason`
    /// when the skipped gate is `subscription_expired`.
    ///
    /// [`renewal_eligibility`] runs before any write and again inside the locked update.
    pub async fn renew_loan(
        &self,
        loan_id: i64,
        force: bool,
        override_reason: Option<String>,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        caller_rights: &UserRights,
    ) -> AppResult<(DateTime<Utc>, i16)> {
        self.perform_renewal(
            loan_id,
            force,
            override_reason,
            audit_actor,
            client_ip,
            caller_rights,
        )
        .await
        .map_err(|err| crate::renewal::with_renewal_forceable(err, caller_rights))
    }

    async fn perform_renewal(
        &self,
        loan_id: i64,
        force: bool,
        override_reason: Option<String>,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        caller_rights: &UserRights,
    ) -> AppResult<(DateTime<Utc>, i16)> {
        let snapshot = self.repository.loans_renewal_snapshot(loan_id).await?;
        if snapshot.returned {
            return Err(AppError::BusinessRule(
                "Cannot renew a returned loan".to_string(),
            ));
        }
        let user = self.repository.users_get_by_id(snapshot.user_id).await?;
        let card_lost = self.repository.users_card_lost(user.id).await?;
        if card_lost {
            return Err(AppError::coded(
                422,
                crate::error::error_code::CARD_LOST,
                crate::error::error_code::CARD_LOST_MESSAGE,
            ));
        }
        let check = self
            .fines
            .check_unpaid_threshold(user.id, user.public_type)
            .await?;
        let renewal_override = RenewalOverride::from_force(force, caller_rights);
        let ctx = RenewalContext {
            bypass_account_gates: renewal_override.account_gates,
            bypass_subscription_expired: renewal_override.subscription_expired,
            bypass_renewal_limits: renewal_override.renewal_limits,
            fines_over_threshold: check.blocks_circulation(),
            fines_message: check.desk_block_message(),
            max_renewals: snapshot.max_renewals,
            renew_window_days: snapshot.renew_window_days,
            now: Utc::now(),
            zone: self.library_zone().await,
        };
        let renewal_user = RenewalUser {
            can_borrow: user.can_borrow(),
            card_lost,
            subscription_expiry: user.expiry_at,
        };
        if let Err(refusal) = renewal_eligibility(&snapshot.loan, &renewal_user, &ctx) {
            return Err(refusal.into_app_error(&snapshot.loan, &renewal_user, &ctx));
        }
        let override_reason = normalized_override_reason(override_reason.as_deref());
        let subscription_applies = renewal_user
            .subscription_expiry
            .is_some_and(|expiry| expiry < ctx.now);
        if ctx.bypass_subscription_expired && subscription_applies && override_reason.is_none() {
            return Err(crate::checkout_refusal::override_reason_required());
        }
        let renewed = self
            .repository
            .loans_renew(loan_id, renewal_override)
            .await?;
        for refusal in renewed.lifted {
            let mut payload = serde_json::json!({
                "reason": refusal.code(),
                "loanId": loan_id,
                "patron_id": snapshot.user_id,
            });
            if refusal == RenewalRefusal::SubscriptionExpired {
                payload["overrideReason"] = serde_json::json!(override_reason);
            }
            self.audit.log(
                Self::renewal_lift_event(refusal),
                audit_actor,
                Some("loan"),
                Some(loan_id),
                client_ip.clone(),
                Some(payload),
                AuditLogMeta::success(),
            );
        }
        Ok((renewed.expiry_at, renewed.renew_count))
    }

    /// Audit event for a renewal gate this call skipped.
    fn renewal_lift_event(refusal: RenewalRefusal) -> &'static str {
        match refusal {
            RenewalRefusal::SubscriptionExpired => {
                audit::event::LOAN_SUBSCRIPTION_EXPIRED_OVERRIDDEN
            }
            RenewalRefusal::FinesOverThreshold => audit::event::LOAN_FINE_THRESHOLD_OVERRIDDEN,
            RenewalRefusal::AccountBlocked
            | RenewalRefusal::MaxRenewals
            | RenewalRefusal::RenewalTooEarly
            | RenewalRefusal::CardLost
            | RenewalRefusal::Withdrawn
            | RenewalRefusal::ClaimedReturned
            | RenewalRefusal::HoldWaiting => audit::event::LOAN_RENEWAL_FORCED,
        }
    }

    /// Renew a loan by item identification (barcode or call number)
    pub async fn renew_loan_by_item(
        &self,
        item_identification: &str,
        force: bool,
        override_reason: Option<String>,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
        caller_rights: &UserRights,
    ) -> AppResult<(i64, DateTime<Utc>, i16)> {
        let loan = self
            .repository
            .loans_get_by_item_identification(item_identification)
            .await?;
        let (new_expiry_date, renew_count) = self
            .renew_loan(
                loan.id,
                force,
                override_reason,
                audit_actor,
                client_ip,
                caller_rights,
            )
            .await?;
        Ok((loan.id, new_expiry_date, renew_count))
    }

    /// Count active loans
    pub async fn count_active(&self) -> AppResult<i64> {
        self.repository.loans_count_active().await
    }

    /// Count overdue loans
    pub async fn count_overdue(&self) -> AppResult<i64> {
        self.repository.loans_count_overdue().await
    }

    /// Count active loans for a specific physical item
    pub async fn count_active_for_item(&self, item_id: i64) -> AppResult<i64> {
        self.repository.loans_count_active_for_item(item_id).await
    }

    /// Count active loans across all physical items of a biblio (used by OPAC availability)
    pub async fn count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64> {
        self.repository
            .loans_count_active_for_biblio(biblio_id)
            .await
    }

    /// Global loan rules per media type (`loans_settings` table).
    pub async fn get_global_loan_settings(&self) -> AppResult<Vec<LoanSettingsDto>> {
        let rows = self.repository.loans_get_settings().await?;
        Ok(rows
            .into_iter()
            .map(|row| LoanSettingsDto {
                media_type: row.media_type,
                max_loans: row.nb_max.unwrap_or(5),
                max_renewals: row.nb_renews.unwrap_or(2),
                duration_days: row.duration.unwrap_or(21),
                renew_at: row.renew_at.unwrap_or(LoanSettingsRenewAt::AtDueDate),
                renew_window_days: row.renew_window_days,
            })
            .collect())
    }

    /// Update global loan rules per media type.
    pub async fn update_global_loan_settings(
        &self,
        request: UpdateLoanSettingsRequest,
    ) -> AppResult<Vec<LoanSettingsDto>> {
        if let Some(loan_settings) = &request.loan_settings {
            for setting in loan_settings {
                if !(0..=366).contains(&setting.renew_window_days) {
                    return Err(AppError::Validation(
                        "renewWindowDays must be between 0 and 366".to_string(),
                    ));
                }
            }
        }

        self.repository.loans_settings_delete_rows().await?;

        if let Some(loan_settings) = request.loan_settings {
            for setting in loan_settings {
                let media_key = setting
                    .media_type
                    .as_ref()
                    .map(|m| m.as_db_str().to_string());
                self.repository
                    .loans_settings_upsert_row(
                        media_key,
                        setting.max_loans,
                        setting.max_renewals,
                        setting.duration_days,
                        setting.renew_at,
                        setting.renew_window_days,
                    )
                    .await?;
            }
        }
        self.get_global_loan_settings().await
    }

    /// Build a downloadable MARC export for all active or archived loans of a user (no pagination).
    /// Caller must enforce `require_self_or_staff`; this method only checks the user exists.
    pub async fn export_user_loans_marc_file(
        &self,
        user_id: i64,
        archived: bool,
        format: LoanMarcExportFormat,
        encoding: LoanMarcExportEncoding,
    ) -> AppResult<(Vec<u8>, &'static str, &'static str)> {
        self.repository.users_get_by_id(user_id).await?;
        let rows = self
            .repository
            .loans_get_for_marc_export(user_id, archived)
            .await?;
        if rows.len() > LOANS_MARC_EXPORT_MAX {
            return Err(AppError::Validation(format!(
                "Too many loans to export ({} > max {})",
                rows.len(),
                LOANS_MARC_EXPORT_MAX
            )));
        }
        let mut records: Vec<MarcRecord> = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(marc_record_for_loan_export(
                &row.biblio,
                row.start_date,
                row.expiry_at,
                row.returned_at,
            ));
        }
        let bytes = serialize_marc_export_records(&records, format, encoding)?;
        let (ct, name) = export_marc_content_type_filename(format);
        Ok((bytes, ct, name))
    }
}

fn export_marc_content_type_filename(format: LoanMarcExportFormat) -> (&'static str, &'static str) {
    match format {
        LoanMarcExportFormat::Json => ("application/json", "loans-export.json"),
        LoanMarcExportFormat::Marc21 => ("application/marc", "loans-export.mrc"),
        LoanMarcExportFormat::Unimarc => ("application/marc", "loans-export.mrc"),
        LoanMarcExportFormat::Marcxml => ("application/xml", "loans-export.xml"),
    }
}

fn serialize_marc_export_records(
    records: &[MarcRecord],
    format: LoanMarcExportFormat,
    encoding: LoanMarcExportEncoding,
) -> AppResult<Vec<u8>> {
    let marc_enc = match encoding {
        LoanMarcExportEncoding::Utf8 => MarcEncoding::Utf8,
        LoanMarcExportEncoding::Marc8 => MarcEncoding::Marc8,
    };
    match format {
        LoanMarcExportFormat::Json => serde_json::to_vec(records)
            .map_err(|e| AppError::Internal(format!("MARC JSON export serialization: {}", e))),
        LoanMarcExportFormat::Marc21 => {
            let mut buf = Vec::new();
            let fmt = MarcFormat::Marc21(marc_enc);
            {
                let mut w = BinaryWriter::new(&mut buf);
                for r in records {
                    let mut rec = r.clone();
                    w.write_record(&fmt, &mut rec)
                        .map_err(|e| AppError::Internal(format!("MARC21 binary write: {}", e)))?;
                }
                w.flush()
                    .map_err(|e| AppError::Internal(format!("MARC21 binary flush: {}", e)))?;
            }
            Ok(buf)
        }
        LoanMarcExportFormat::Unimarc => {
            let mut buf = Vec::new();
            let fmt = MarcFormat::Unimarc(marc_enc);
            {
                let mut w = BinaryWriter::new(&mut buf);
                for r in records {
                    let mut rec = r.clone();
                    w.write_record(&fmt, &mut rec)
                        .map_err(|e| AppError::Internal(format!("UNIMARC binary write: {}", e)))?;
                }
                w.flush()
                    .map_err(|e| AppError::Internal(format!("UNIMARC binary flush: {}", e)))?;
            }
            Ok(buf)
        }
        LoanMarcExportFormat::Marcxml => {
            let mut buf = Vec::new();
            // MARC-XML output is always UTF-8; semantic record is serialized via the chosen format.
            let fmt = MarcFormat::Marc21(MarcEncoding::Utf8);
            {
                let mut w = XmlWriter::new(&mut buf);
                w.start_collection()
                    .map_err(|e| AppError::Internal(format!("MARC-XML collection start: {}", e)))?;
                for r in records {
                    w.write_record(&fmt, r)
                        .map_err(|e| AppError::Internal(format!("MARC-XML record: {}", e)))?;
                }
                w.end_collection()
                    .map_err(|e| AppError::Internal(format!("MARC-XML collection end: {}", e)))?;
                w.flush()
                    .map_err(|e| AppError::Internal(format!("MARC-XML flush: {}", e)))?;
            }
            Ok(buf)
        }
    }
}

// =============================================================================
// Unit tests — use manual test doubles to avoid mockall lifetime issues
// with async_trait + &str parameters.
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::AppError,
        models::{
            loan::CreateLoan,
            user::{AccountTypeSlug, User, UserStatus},
        },
        repository::{LoansRepository, UsersRepository},
    };
    // ----- Minimal test double implementing both required traits -----

    struct FakeRepo {
        /// Pre-loaded user to return for `users_get_by_id`
        user: Option<User>,
        /// Return value for `loans_create`
        loan_id: i64,
        /// Active loan returned by `loans_get_by_id` / `loans_get_by_item_identification`
        loan: Option<crate::models::loan::Loan>,
    }

    struct FakeFinesRepo {
        unpaid: rust_decimal::Decimal,
        threshold: Option<rust_decimal::Decimal>,
    }

    impl Default for FakeFinesRepo {
        fn default() -> Self {
            Self {
                unpaid: rust_decimal::Decimal::ZERO,
                threshold: None,
            }
        }
    }

    fn make_user(
        id: i64,
        status: Option<UserStatus>,
        expiry_at: Option<chrono::DateTime<Utc>>,
    ) -> User {
        User {
            id,
            // NULL status in DB is treated as active; tests pass None for the default happy path.
            status: status.or(Some(UserStatus::Active)),
            expiry_at,
            account_type: AccountTypeSlug::Reader,
            group_id: None,
            barcode: None,
            login: None,
            password: None,
            firstname: None,
            lastname: None,
            email: None,
            addr_street: None,
            addr_zip_code: None,
            addr_city: None,
            phone: None,
            birthdate: None,
            created_at: None,
            update_at: None,
            fee: None,
            archived_at: None,
            language: None,
            sex: None,
            staff_type: None,
            hours_per_week: None,
            staff_start_date: None,
            staff_end_date: None,
            public_type: None,
            birth_date_missing: true,
            notes: None,
            two_factor_enabled: None,
            two_factor_method: None,
            totp_secret: None,
            recovery_codes: None,
            recovery_codes_used: None,
            receive_reminders: true,
            must_change_password: false,
            token_version: 0,
            guardian_id: None,
            events_consent: false,
            events_consent_at: None,
            events_consent_source: None,
        }
    }

    #[async_trait::async_trait]
    impl LoansRepository for FakeRepo {
        async fn loans_settings_delete_rows(&self) -> AppResult<()> {
            Ok(())
        }
        async fn library_timezone_name(&self) -> AppResult<String> {
            Ok(crate::services::calendar::DEFAULT_TIMEZONE.to_string())
        }
        async fn loans_get_by_id(&self, _: i64) -> AppResult<crate::models::loan::Loan> {
            self.loan
                .clone()
                .ok_or_else(|| AppError::NotFound("loan not found".into()))
        }
        async fn loans_get_borrower_for_loan(
            &self,
            _: i64,
        ) -> AppResult<crate::models::user::UserShort> {
            unimplemented!()
        }
        async fn loans_active_borrowers_by_item_ids(
            &self,
            _: &[i64],
        ) -> AppResult<std::collections::HashMap<i64, crate::models::user::UserShort>> {
            Ok(std::collections::HashMap::new())
        }
        async fn loans_get_by_item_identification(
            &self,
            _: &str,
        ) -> AppResult<crate::models::loan::Loan> {
            self.loan
                .clone()
                .ok_or_else(|| AppError::NotFound("loan not found".into()))
        }
        async fn loans_get_for_user(
            &self,
            _: i64,
            _: i64,
            _: i64,
        ) -> AppResult<(Vec<LoanDetails>, i64)> {
            Ok((vec![], 0))
        }
        async fn loans_archives_get_for_user(
            &self,
            _: i64,
            _: i64,
            _: i64,
        ) -> AppResult<(Vec<LoanDetails>, i64)> {
            Ok((vec![], 0))
        }
        async fn loans_get_for_marc_export(
            &self,
            _: i64,
            _: bool,
        ) -> AppResult<Vec<crate::models::loan::LoanMarcExportRow>> {
            Ok(vec![])
        }
        async fn loans_create(
            &self,
            _: &CreateLoan,
            progress: &mut crate::checkout_refusal::CheckoutProgress,
            _: &crate::models::user::UserRights,
        ) -> AppResult<LoanCreateOutcome> {
            crate::checkout_refusal::finish_checkout_refusals(progress)?;
            Ok(LoanCreateOutcome {
                loan_id: self.loan_id,
                expiry_at: Utc::now(),
                site_id: None,
                fulfilled_hold_id: None,
                preserved_hold_id: progress.preserved_hold_id,
            })
        }
        async fn loans_return(&self, _: i64) -> AppResult<LoanReturnOutcome> {
            unimplemented!()
        }
        async fn loans_lost_notice_for_closed_loan(
            &self,
            _: i64,
        ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
            Ok(None)
        }
        async fn loans_lost_scan_by_barcode(
            &self,
            _: &str,
        ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
            Ok(None)
        }
        async fn loans_complete_to_pick_by_identification(
            &self,
            _: &str,
        ) -> AppResult<Option<crate::models::dto::holds::ToPickCompletion>> {
            Ok(None)
        }
        async fn loans_recover_to_search_by_identification(
            &self,
            _: &str,
        ) -> AppResult<Option<crate::models::dto::holds::ShelfFound>> {
            Ok(None)
        }
        async fn loans_item_exists_by_barcode(&self, _: &str) -> AppResult<bool> {
            Ok(false)
        }
        async fn loans_return_scan_copy(
            &self,
            _: &str,
        ) -> AppResult<Option<crate::models::item::ReturnScanCopy>> {
            Ok(None)
        }
        async fn loans_check_in_shelf_copy(
            &self,
            _: &str,
        ) -> AppResult<Option<crate::models::dto::holds::ShelfCheckIn>> {
            Ok(None)
        }
        async fn loans_set_aside_unassigned_title_hold(
            &self,
            _: &str,
            _: i64,
            _: i64,
        ) -> AppResult<crate::models::dto::holds::ShelfCheckIn> {
            unimplemented!("set aside is not used by these service tests")
        }
        async fn loans_renewal_snapshot(
            &self,
            loan_id: i64,
        ) -> AppResult<crate::renewal::LoanRenewalSnapshot> {
            let loan = self
                .loan
                .clone()
                .ok_or_else(|| AppError::NotFound(format!("Loan with id {loan_id} not found")))?;
            let due_at = loan
                .expiry_at
                .unwrap_or_else(|| Utc::now() + chrono::Duration::days(7));
            Ok(crate::renewal::LoanRenewalSnapshot {
                user_id: loan.user_id,
                returned: loan.returned_at.is_some(),
                loan: crate::renewal::RenewalLoan {
                    renewal_count: loan.nb_renews.unwrap_or(0),
                    due_at,
                    withdrawn: false,
                    claimed_returned: false,
                    hold_waiting: false,
                },
                max_renewals: 2,
                renew_window_days: 7,
            })
        }
        async fn loans_renew(
            &self,
            _: i64,
            _: crate::renewal::RenewalOverride,
        ) -> AppResult<crate::renewal::LoanRenewalResult> {
            Ok(crate::renewal::LoanRenewalResult {
                expiry_at: Utc::now() + chrono::Duration::days(21),
                renew_count: 1,
                lifted: Vec::new(),
            })
        }
        async fn loans_get_settings(&self) -> AppResult<Vec<crate::models::loan::LoanSettings>> {
            Ok(vec![])
        }
        async fn loans_count_active(&self) -> AppResult<i64> {
            Ok(0)
        }
        async fn loans_count_overdue(&self) -> AppResult<i64> {
            Ok(0)
        }
        async fn loans_get_active_ids_for_item(&self, _: i64) -> AppResult<Vec<i64>> {
            Ok(vec![])
        }
        async fn loans_count_active_for_item(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn loans_get_active_ids_for_biblio(&self, _: i64) -> AppResult<Vec<i64>> {
            Ok(vec![])
        }
        async fn loans_get_active_ids_for_user(&self, _: i64) -> AppResult<Vec<i64>> {
            Ok(vec![])
        }
        async fn loans_count_active_for_biblio(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn loans_count_active_for_user(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn loans_get_overdue_for_reminders(
            &self,
            _: crate::repository::loans::ReminderTierDelays,
        ) -> AppResult<Vec<crate::repository::loans::OverdueLoanRow>> {
            Ok(vec![])
        }
        async fn loans_get_overdue(
            &self,
            _: i64,
            _: i64,
        ) -> AppResult<(Vec<crate::repository::loans::OverdueLoanRow>, i64)> {
            Ok((vec![], 0))
        }
        async fn loans_update_reminder_sent(&self, _: &[i64]) -> AppResult<()> {
            Ok(())
        }
        async fn loans_settings_upsert_row(
            &self,
            _: Option<String>,
            _: i16,
            _: i16,
            _: i16,
            _: LoanSettingsRenewAt,
            _: i16,
        ) -> AppResult<()> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl UsersRepository for FakeRepo {
        async fn users_count(&self) -> AppResult<i64> {
            Ok(0)
        }
        async fn users_set_must_change_password(&self, _: i64, _: bool) -> AppResult<()> {
            Ok(())
        }
        async fn users_get_by_id(&self, _: i64) -> AppResult<User> {
            self.user
                .clone()
                .ok_or_else(|| AppError::NotFound("user not found".into()))
        }
        async fn users_get_token_version(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn users_get_by_login(&self, _: &str) -> AppResult<Option<User>> {
            Ok(None)
        }
        async fn users_get_by_email(&self, _: &str) -> AppResult<Option<User>> {
            Ok(None)
        }
        async fn users_update_password(&self, _: i64, _: &str) -> AppResult<()> {
            Ok(())
        }
        async fn users_email_exists(&self, _: &str, _: Option<i64>) -> AppResult<bool> {
            Ok(false)
        }
        async fn users_login_exists(&self, _: &str, _: Option<i64>) -> AppResult<bool> {
            Ok(false)
        }
        async fn users_get_rights(
            &self,
            _: &AccountTypeSlug,
        ) -> AppResult<crate::models::user::UserRights> {
            unimplemented!()
        }
        async fn users_search(
            &self,
            _: &crate::models::user::UserQuery,
        ) -> AppResult<(Vec<crate::models::user::UserShort>, i64)> {
            Ok((vec![], 0))
        }
        async fn users_create(
            &self,
            _: &crate::models::user::UserPayload,
            _: Option<String>,
        ) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_update(
            &self,
            _: i64,
            _: &crate::models::user::UserPayload,
            _: Option<String>,
        ) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_delete(
            &self,
            _: i64,
            _: bool,
        ) -> AppResult<crate::models::user::UserErasureResult> {
            Ok(crate::models::user::UserErasureResult::default())
        }
        async fn users_list_due_for_auto_erasure(&self, _: u32) -> AppResult<Vec<i64>> {
            Ok(Vec::new())
        }
        async fn users_block(&self, _: i64) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_unblock(&self, _: i64) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_update_profile(
            &self,
            _: i64,
            _: &crate::models::user::UpdateProfile,
            _: Option<String>,
        ) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_update_account_type(&self, _: i64, _: &AccountTypeSlug) -> AppResult<User> {
            unimplemented!()
        }
        async fn users_update_2fa_settings(
            &self,
            _: i64,
            _: bool,
            _: Option<&str>,
            _: Option<&str>,
            _: Option<&str>,
        ) -> AppResult<()> {
            Ok(())
        }
        async fn users_mark_recovery_code_used(&self, _: i64, _: &str) -> AppResult<()> {
            Ok(())
        }
        async fn users_get_emails_by_public_type(
            &self,
            _: Option<i64>,
        ) -> AppResult<Vec<crate::repository::users::UserEmailTarget>> {
            Ok(vec![])
        }
        async fn users_list_announcement_recipients(
            &self,
            _: bool,
            _: &[String],
        ) -> AppResult<Vec<crate::repository::users::AnnouncementRecipient>> {
            Ok(vec![])
        }
        async fn users_hold_ready_contact(
            &self,
            _: i64,
        ) -> AppResult<Option<crate::repository::users::HoldReadyUserContact>> {
            Ok(None)
        }
        async fn users_card_lost(&self, _: i64) -> AppResult<bool> {
            Ok(false)
        }
    }

    // LoansServiceRepository has a blanket impl for T: LoansRepository + UsersRepository + Send + Sync,
    // so FakeRepo already implements it — no explicit impl needed.

    #[async_trait::async_trait]
    impl crate::repository::FinesRepository for FakeFinesRepo {
        async fn fines_list_for_user(&self, _: i64) -> AppResult<Vec<crate::models::fine::Fine>> {
            Ok(vec![])
        }
        async fn fines_get_by_id(&self, _: i64) -> AppResult<crate::models::fine::Fine> {
            unimplemented!()
        }
        async fn fines_create(
            &self,
            _: i64,
            _: i64,
            _: rust_decimal::Decimal,
            _: Option<&str>,
        ) -> AppResult<crate::models::fine::Fine> {
            unimplemented!()
        }
        async fn fines_pay(
            &self,
            _: i64,
            _: rust_decimal::Decimal,
            _: Option<&str>,
        ) -> AppResult<crate::models::fine::Fine> {
            unimplemented!()
        }
        async fn fines_waive(
            &self,
            _: i64,
            _: Option<&str>,
        ) -> AppResult<crate::models::fine::Fine> {
            unimplemented!()
        }
        async fn fines_list_rules(&self) -> AppResult<Vec<crate::models::fine::FineRule>> {
            Ok(vec![])
        }
        async fn fines_upsert_rule(
            &self,
            _: Option<&str>,
            _: rust_decimal::Decimal,
            _: Option<rust_decimal::Decimal>,
            _: i32,
        ) -> AppResult<crate::models::fine::FineRule> {
            unimplemented!()
        }
        async fn fines_total_unpaid(&self, _: i64) -> AppResult<rust_decimal::Decimal> {
            Ok(self.unpaid)
        }
        async fn fines_get_unpaid_threshold(
            &self,
            _: Option<i64>,
        ) -> AppResult<Option<rust_decimal::Decimal>> {
            Ok(self.threshold)
        }
        async fn fines_get_global_unpaid_threshold(
            &self,
        ) -> AppResult<crate::models::fine::StoredGlobalUnpaidThreshold> {
            Ok(crate::models::fine::StoredGlobalUnpaidThreshold {
                mode: crate::models::fine::GlobalUnpaidThresholdMode::None,
                amount: self.threshold.unwrap_or(rust_decimal::Decimal::ZERO),
            })
        }
        async fn fines_set_global_unpaid_threshold(
            &self,
            mode: crate::models::fine::GlobalUnpaidThresholdMode,
            amount: Option<rust_decimal::Decimal>,
        ) -> AppResult<crate::models::fine::StoredGlobalUnpaidThreshold> {
            Ok(crate::models::fine::StoredGlobalUnpaidThreshold {
                mode,
                amount: amount.unwrap_or(rust_decimal::Decimal::ZERO),
            })
        }
        async fn fines_get_open_for_loan(
            &self,
            _: i64,
        ) -> AppResult<Option<crate::models::fine::Fine>> {
            unimplemented!()
        }
        async fn fines_upsert_open(
            &self,
            _: i64,
            _: i64,
            _: rust_decimal::Decimal,
            _: Option<&str>,
        ) -> AppResult<(
            crate::models::fine::Fine,
            crate::models::dto::fines::AccrueOutcome,
        )> {
            unimplemented!()
        }
        async fn fines_get_accrual_loan(
            &self,
            _: i64,
        ) -> AppResult<crate::models::fine::FineAccrualLoan> {
            unimplemented!()
        }
        async fn fines_list_accrual_loans(
            &self,
            _: Option<i64>,
        ) -> AppResult<Vec<crate::models::fine::FineAccrualLoan>> {
            unimplemented!()
        }
    }

    fn make_service(user: Option<User>, loan_id: i64) -> LoansService {
        make_service_with_fines(user, loan_id, FakeFinesRepo::default(), None)
    }

    fn make_service_with_fines(
        user: Option<User>,
        loan_id: i64,
        fines: FakeFinesRepo,
        loan: Option<crate::models::loan::Loan>,
    ) -> LoansService {
        let pool = sqlx::Pool::connect_lazy("postgres://localhost/unused").unwrap();
        let audit = AuditService::new(crate::repository::Repository::new(pool.clone(), None));
        let dynamic_config = crate::DynamicConfig::new(crate::AppConfig::for_test());
        let email = crate::EmailService::new(dynamic_config, pool);
        let (tx, _) = tokio::sync::broadcast::channel(1);
        let events = crate::services::event_bus::EventBus::new(tx);
        LoansService::new(
            Arc::new(FakeRepo {
                user,
                loan_id,
                loan,
            }),
            FinesService::new(
                Arc::new(fines),
                Arc::new(crate::services::calendar::AlwaysOpenCalendar),
            ),
            audit,
            email,
            events,
        )
    }

    fn make_active_loan(id: i64, user_id: i64) -> crate::models::loan::Loan {
        crate::models::loan::Loan {
            id,
            user_id,
            item_id: 42,
            date: Utc::now(),
            renew_at: None,
            nb_renews: Some(0),
            expiry_at: Some(Utc::now() + chrono::Duration::days(7)),
            notes: None,
            returned_at: None,
            last_reminder_sent_at: None,
            reminder_count: None,
            site_id: None,
        }
    }

    fn decimal(s: &str) -> rust_decimal::Decimal {
        s.parse().expect("decimal")
    }

    fn make_loan(user_id: i64, force: bool) -> CreateLoan {
        CreateLoan {
            user_id,
            item_id: Some(42),
            item_identification: None,
            force,
            site_id: None,
            override_reason: None,
        }
    }

    fn make_loan_with_reason(user_id: i64, force: bool, reason: &str) -> CreateLoan {
        let mut loan = make_loan(user_id, force);
        loan.override_reason = Some(reason.to_string());
        loan
    }

    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_loan_active_user_succeeds() {
        let user = make_user(1, None, None);
        let svc = make_service(Some(user), 100);
        assert!(svc
            .create_loan(
                make_loan(1, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    fn assert_checkout_refusal(err: AppError, code: &str, forceable: bool) -> String {
        match err {
            AppError::CheckoutRefused(refusal) => {
                assert_eq!(refusal.code, code);
                assert_eq!(refusal.forceable(), forceable);
                refusal.message
            }
            other => panic!("expected {code}, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_loan_blocked_user_rejected() {
        let user = make_user(2, Some(UserStatus::Blocked), None);
        let svc = make_service(Some(user), 0);
        let err = svc
            .create_loan(
                make_loan(2, false),
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
            .expect_err("blocked");
        assert_checkout_refusal(err, "business_rule_violation", false);
    }

    #[tokio::test]
    async fn test_create_loan_blocked_user_with_force_still_rejected() {
        let user = make_user(3, Some(UserStatus::Blocked), None);
        let svc = make_service(Some(user), 101);
        let err = svc
            .create_loan(
                make_loan(3, true),
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
            .expect_err("force does not skip an inactive account");
        assert_checkout_refusal(err, "business_rule_violation", false);
    }

    #[tokio::test]
    async fn test_create_loan_deleted_user_always_rejected() {
        let user = make_user(4, Some(UserStatus::Deleted), None);
        let svc = make_service(Some(user), 0);
        let err = svc
            .create_loan(
                make_loan(4, true),
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
            .expect_err("deleted");
        assert_checkout_refusal(err, "business_rule_violation", false);
    }

    fn override_rights() -> crate::models::user::UserRights {
        crate::models::user::UserRights {
            circulation_override_rights: crate::models::user::Rights::Write,
            settings_rights: crate::models::user::Rights::None,
            ..crate::models::user::UserRights::default()
        }
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_rejected() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(5, None, Some(expired));
        let svc = make_service(Some(user), 0);
        let err = svc
            .create_loan(make_loan(5, false), None, None, &override_rights())
            .await
            .expect_err("expired");
        let message = assert_checkout_refusal(err, "subscription_expired", true);
        assert!(message.contains("force=true"), "{message}");
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_with_force_succeeds() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(6, None, Some(expired));
        let svc = make_service(Some(user), 102);
        assert!(svc
            .create_loan(
                make_loan_with_reason(6, true, "membership renewal pending"),
                None,
                None,
                &override_rights()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_force_without_reason_rejected() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(18, None, Some(expired));
        let svc = make_service(Some(user), 0);
        let err = svc
            .create_loan(make_loan(18, true), None, None, &override_rights())
            .await
            .expect_err("reason");
        match err {
            AppError::Coded { status, code, .. } => {
                assert_eq!(status, 422);
                assert_eq!(code, "override_reason_required");
            }
            other => panic!("expected override_reason_required, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_force_without_override_right_rejected() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(17, None, Some(expired));
        let svc = make_service(Some(user), 0);
        let err = svc
            .create_loan(
                make_loan(17, true),
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
            .expect_err("desk force");
        assert_checkout_refusal(err, "subscription_expired", false);
    }

    #[tokio::test]
    async fn test_create_loan_user_not_found() {
        let svc = make_service(None, 0); // no user pre-loaded
        assert!(matches!(
            svc.create_loan(
                make_loan(99, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_valid_subscription_not_expired() {
        let future_date = Utc::now() + chrono::Duration::days(30);
        let user = make_user(7, None, Some(future_date)); // subscription valid
        let svc = make_service(Some(user), 103);
        assert!(svc
            .create_loan(
                make_loan(7, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_null_threshold_does_not_block() {
        let user = make_user(15, None, None);
        let svc = make_service_with_fines(
            Some(user),
            107,
            FakeFinesRepo {
                unpaid: decimal("40"),
                threshold: None,
            },
            None,
        );
        assert!(svc
            .create_loan(
                make_loan(15, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_unpaid_under_threshold_succeeds() {
        let user = make_user(8, None, None);
        let svc = make_service_with_fines(
            Some(user),
            104,
            FakeFinesRepo {
                unpaid: decimal("4.99"),
                threshold: Some(decimal("5")),
            },
            None,
        );
        assert!(svc
            .create_loan(
                make_loan(8, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_unpaid_over_threshold_rejected() {
        let user = make_user(9, None, None);
        let svc = make_service_with_fines(
            Some(user),
            0,
            FakeFinesRepo {
                unpaid: decimal("15"),
                threshold: Some(decimal("10")),
            },
            None,
        );
        let err = svc
            .create_loan(
                make_loan(9, false),
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
            .expect_err("over-threshold checkout must fail");
        let message = assert_checkout_refusal(err, "fines_over_threshold", false);
        assert!(message.contains("15"), "{message}");
        assert!(message.contains("10"), "{message}");
        assert!(message.contains("force=true"), "{message}");
        let offered = svc
            .create_loan(make_loan(9, false), None, None, &loans_write_rights())
            .await
            .expect_err("loans write still refuses without force");
        assert_checkout_refusal(offered, "fines_over_threshold", true);
    }

    fn loans_write_rights() -> crate::models::user::UserRights {
        crate::models::user::UserRights {
            loans_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        }
    }

    fn settings_write_rights() -> crate::models::user::UserRights {
        crate::models::user::UserRights {
            settings_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        }
    }

    #[tokio::test]
    async fn test_create_loan_unpaid_over_threshold_force_without_loans_write_rejected() {
        let user = make_user(18, None, None);
        let svc = make_service_with_fines(
            Some(user),
            0,
            FakeFinesRepo {
                unpaid: decimal("15"),
                threshold: Some(decimal("10")),
            },
            None,
        );
        let err = svc
            .create_loan(
                make_loan(18, true),
                Some(99),
                None,
                &settings_write_rights(),
            )
            .await
            .expect_err("settings write must not lift the fine gate");
        assert_checkout_refusal(err, "fines_over_threshold", false);
    }

    #[tokio::test]
    async fn test_create_loan_unpaid_over_threshold_force_succeeds() {
        let user = make_user(10, None, None);
        let svc = make_service_with_fines(
            Some(user),
            105,
            FakeFinesRepo {
                unpaid: decimal("15"),
                threshold: Some(decimal("10")),
            },
            None,
        );
        assert!(svc
            .create_loan(
                make_loan(10, true),
                Some(99),
                Some("127.0.0.1".into()),
                &loans_write_rights()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_zero_unpaid_never_blocks() {
        let user = make_user(11, None, None);
        let svc = make_service_with_fines(
            Some(user),
            106,
            FakeFinesRepo {
                unpaid: decimal("0"),
                threshold: Some(decimal("0")),
            },
            None,
        );
        assert!(svc
            .create_loan(
                make_loan(11, false),
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_renew_loan_unpaid_under_threshold_succeeds() {
        let user = make_user(12, None, None);
        let loan = make_active_loan(200, 12);
        let svc = make_service_with_fines(
            Some(user),
            200,
            FakeFinesRepo {
                unpaid: decimal("1"),
                threshold: Some(decimal("5")),
            },
            Some(loan),
        );
        assert!(svc
            .renew_loan(
                200,
                false,
                None,
                None,
                None,
                &crate::models::user::UserRights::default()
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_renew_loan_unpaid_over_threshold_rejected() {
        let user = make_user(13, None, None);
        let loan = make_active_loan(201, 13);
        let svc = make_service_with_fines(
            Some(user),
            201,
            FakeFinesRepo {
                unpaid: decimal("20"),
                threshold: Some(decimal("0")),
            },
            Some(loan),
        );
        match svc
            .renew_loan(
                201,
                false,
                None,
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
        {
            Err(AppError::RenewalDenied {
                code,
                message,
                forceable,
            }) => {
                assert_eq!(code, "fines_over_threshold");
                assert!(!forceable);
                assert!(message.contains("20"), "{message}");
                assert!(message.contains("force=true"), "{message}");
            }
            other => panic!("expected fines_over_threshold, got {other:?}"),
        }
        let desk = crate::models::user::UserRights {
            loans_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        };
        match svc.renew_loan(201, false, None, None, None, &desk).await {
            Err(AppError::RenewalDenied {
                code, forceable, ..
            }) => {
                assert_eq!(code, "fines_over_threshold");
                assert!(forceable);
            }
            other => panic!("expected forceable fines_over_threshold, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_renew_loan_unpaid_over_threshold_force_succeeds() {
        let user = make_user(14, None, None);
        let loan = make_active_loan(202, 14);
        let svc = make_service_with_fines(
            Some(user),
            202,
            FakeFinesRepo {
                unpaid: decimal("20"),
                threshold: Some(decimal("0")),
            },
            Some(loan),
        );
        let desk = crate::models::user::UserRights {
            loans_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        };
        assert!(svc
            .renew_loan(202, true, None, Some(99), Some("127.0.0.1".into()), &desk)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_renew_loan_expired_subscription_rejected() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(15, None, Some(expired));
        let loan = make_active_loan(203, 15);
        let svc = make_service_with_fines(Some(user), 203, FakeFinesRepo::default(), Some(loan));
        match svc
            .renew_loan(
                203,
                false,
                None,
                None,
                None,
                &crate::models::user::UserRights::default(),
            )
            .await
        {
            Err(AppError::RenewalDenied {
                code,
                message,
                forceable,
            }) => {
                assert_eq!(code, "subscription_expired");
                assert!(!forceable);
                assert!(message.contains("force=true"), "{message}");
            }
            other => panic!("expected subscription_expired, got {other:?}"),
        }
        let desk = crate::models::user::UserRights {
            loans_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        };
        match svc.renew_loan(203, true, None, Some(99), None, &desk).await {
            Err(AppError::RenewalDenied {
                code, forceable, ..
            }) => {
                assert_eq!(code, "subscription_expired");
                assert!(!forceable);
            }
            other => panic!("loans write must not force subscription_expired, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_renew_loan_expired_subscription_force_succeeds() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(16, None, Some(expired));
        let loan = make_active_loan(204, 16);
        let svc = make_service_with_fines(Some(user), 204, FakeFinesRepo::default(), Some(loan));
        let settings = crate::models::user::UserRights {
            loans_rights: crate::models::user::Rights::Write,
            settings_rights: crate::models::user::Rights::None,
            circulation_override_rights: crate::models::user::Rights::Write,
            ..crate::models::user::UserRights::default()
        };
        let missing = svc
            .renew_loan(204, true, None, Some(99), None, &settings)
            .await
            .expect_err("reason");
        match missing {
            AppError::Coded { code, .. } => assert_eq!(code, "override_reason_required"),
            other => panic!("expected override_reason_required, got {other:?}"),
        }
        assert!(svc
            .renew_loan(
                204,
                true,
                Some("still enrolled".into()),
                Some(99),
                None,
                &settings
            )
            .await
            .is_ok());
    }
}
