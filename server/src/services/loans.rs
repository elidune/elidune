//! Loan management service

use chrono::{DateTime, Utc};

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    marc::{marc_record_for_loan_export, MarcRecord},
    models::{
        dto::loans::{LoanSettingsDto, UpdateLoanSettingsRequest},
        loan::{
            CreateLoan, HoldReadyEmailOutcome, LoanCreateOutcome, LoanDetails,
            LoanMarcExportEncoding, LoanMarcExportFormat, LoanReturnOutcome, LoanSettingsRenewAt,
            LOANS_MARC_EXPORT_MAX,
        },
        user::UserStatus,
        Loan,
    },
    renewal::{renewal_eligibility, RenewalContext, RenewalUser},
    repository::LoansServiceRepository,
    services::{
        audit::{self, AuditLogMeta, AuditService},
        email::EmailService,
        event_bus::EventBus,
        fines::FinesService,
    },
};
use z3950_rs::marc_rs::{BinaryWriter, Encoding as MarcEncoding, MarcFormat, XmlWriter};

/// Desk scan of a barcode: a loan return, a `to_pick` check-in, a found `to_search` copy,
/// or a lost copy that is no longer on loan.
#[derive(Debug, Clone)]
pub struct ItemCheckIn {
    pub loan: Option<LoanDetails>,
    pub hold: Option<crate::models::hold::Hold>,
    pub transit_started: bool,
    pub destination_site_id: Option<i64>,
    pub destination_site_name: Option<String>,
    /// Set when the copy is lost. Status is unchanged.
    pub lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
}

/// Result of `POST /loans/{id}/return`. `loan` is absent when the loan was already
/// closed by a lost declaration and the copy is still lost.
#[derive(Debug, Clone)]
pub struct DeskReturn {
    pub loan: Option<LoanDetails>,
    pub lost: Option<crate::models::dto::circulation::LostItemOnReturn>,
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

    /// Block checkout/renew when unpaid fines are strictly above the effective threshold.
    /// A null threshold does not block on amount.
    /// `force=true` bypasses the gate and is audited (actor, unpaid total, threshold).
    async fn enforce_unpaid_fine_threshold(
        &self,
        user_id: i64,
        public_type_id: Option<i64>,
        force: bool,
        operation: &'static str,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<()> {
        let check = self
            .fines
            .check_unpaid_threshold(user_id, public_type_id)
            .await?;
        if !check.blocks_circulation() {
            return Ok(());
        }
        if !force {
            return Err(AppError::BusinessRule(check.desk_block_message()));
        }
        self.audit.log(
            audit::event::LOAN_FINE_THRESHOLD_OVERRIDDEN,
            audit_actor,
            Some("user"),
            Some(user_id),
            client_ip,
            Some(serde_json::json!({
                "unpaid": check.unpaid,
                "threshold": check.threshold,
                "operation": operation,
            })),
            AuditLogMeta::success(),
        );
        Ok(())
    }

    /// Create a new loan (borrow an item).
    ///
    /// Enforces user-level rules before delegating to the repository:
    /// - blocked users cannot borrow unless `force` is set
    /// - a lost or stolen library card is always rejected (`422` `card_lost`), including `force` and any role
    /// - expired subscriptions are rejected unless `force` is set
    /// - unpaid fines strictly above the effective threshold are rejected unless `force` is set
    ///   (a null threshold does not block on amount)
    ///
    /// The repository enforces the hold queue on the copy: only the patron whose turn it is
    /// (`ready`, else first `pending`) may borrow unless `force=true` (staff clears active holds on that copy).
    /// `force` never closes or replaces an open loan. A copy already on loan is 409.
    pub async fn create_loan(
        &self,
        loan: CreateLoan,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<LoanCreateOutcome> {
        let user = self.repository.users_get_by_id(loan.user_id).await?;

        let status = user.status.unwrap_or(UserStatus::Active);
        if status == UserStatus::Deleted {
            return Err(AppError::BusinessRule(
                "Cannot create a loan for a deleted user account".to_string(),
            ));
        }

        if !user.can_borrow() && !loan.force {
            return Err(AppError::BusinessRule(
                "User account is not active or cannot borrow — use force=true to override"
                    .to_string(),
            ));
        }

        if self.repository.users_card_lost(user.id).await? {
            return Err(AppError::coded(
                422,
                crate::error::error_code::CARD_LOST,
                crate::error::error_code::CARD_LOST_MESSAGE,
            ));
        }

        if let Some(expiry_at) = user.expiry_at {
            if expiry_at < Utc::now() && !loan.force {
                let zone = crate::services::calendar::parse_timezone(
                    &self.repository.library_timezone_name().await?,
                )
                .unwrap_or_else(|_| crate::services::calendar::default_zone());
                return Err(AppError::BusinessRule(format!(
                    "User subscription expired on {} — use force=true to override",
                    crate::services::calendar::format_patron_iso_date(zone, expiry_at)
                )));
            }
        }

        self.enforce_unpaid_fine_threshold(
            loan.user_id,
            user.public_type,
            loan.force,
            "checkout",
            audit_actor,
            client_ip.clone(),
        )
        .await?;

        let outcome = self.repository.loans_create(&loan).await?;
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
    pub async fn return_loan_by_item(
        &self,
        item_identification: &str,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<ItemCheckIn> {
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
                    });
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
                    });
                }
                if let Some(found) = self
                    .repository
                    .loans_recover_to_search_by_identification(item_identification)
                    .await?
                {
                    if let Some(hold) = found.hold.as_ref() {
                        if !found.transit_started
                            && hold.status == crate::models::hold::HoldStatus::Ready
                        {
                            self.notify_picked_hold_ready(
                                hold,
                                found.title.as_deref().unwrap_or("(unknown title)"),
                                found.barcode.as_deref(),
                                audit_actor,
                                client_ip.clone(),
                            )
                            .await;
                        }
                    }
                    self.audit.log(
                        audit::event::ITEM_FOUND_ON_SHELF,
                        audit_actor,
                        Some("item"),
                        found.hold.as_ref().and_then(|h| h.item_id),
                        client_ip,
                        Some(serde_json::json!({
                            "found": true,
                            "barcode": item_identification,
                        })),
                        AuditLogMeta::success(),
                    );
                    return Ok(ItemCheckIn {
                        loan: None,
                        hold: found.hold,
                        transit_started: found.transit_started,
                        destination_site_id: found.destination_site_id,
                        destination_site_name: found.destination_site_name,
                        lost: None,
                    });
                }
                if self
                    .repository
                    .loans_item_exists_by_barcode(item_identification)
                    .await?
                {
                    return Err(AppError::ItemNotOnLoan(format!(
                        "Item {item_identification} is not on loan"
                    )));
                }
                Err(AppError::NotFound(format!(
                    "No active loan found for item {item_identification}"
                )))
            }
            Err(e) => Err(e),
        }
    }

    async fn notify_picked_hold_ready(
        &self,
        hold: &crate::models::hold::Hold,
        title: &str,
        barcode: Option<&str>,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
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
                "trigger": "pick_check_in",
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

    /// Renew a loan. `force` bypasses account and unpaid-fine gates (fines are audited).
    /// A lost or stolen card is always `422` `card_lost`, including `force`.
    ///
    /// [`renewal_eligibility`] runs before any write and again inside the locked update.
    pub async fn renew_loan(
        &self,
        loan_id: i64,
        force: bool,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
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
        if force && check.blocks_circulation() {
            self.audit.log(
                audit::event::LOAN_FINE_THRESHOLD_OVERRIDDEN,
                audit_actor,
                Some("user"),
                Some(user.id),
                client_ip,
                Some(serde_json::json!({
                    "unpaid": check.unpaid,
                    "threshold": check.threshold,
                    "operation": "renew",
                })),
                AuditLogMeta::success(),
            );
        }
        let ctx = RenewalContext {
            bypass_account_gates: force,
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
        };
        if let Err(refusal) = renewal_eligibility(&snapshot.loan, &renewal_user, &ctx) {
            return Err(refusal.into_app_error(&snapshot.loan, &ctx));
        }
        self.repository.loans_renew(loan_id, force).await
    }

    /// Renew a loan by item identification (barcode or call number)
    pub async fn renew_loan_by_item(
        &self,
        item_identification: &str,
        force: bool,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<(i64, DateTime<Utc>, i16)> {
        let loan = self
            .repository
            .loans_get_by_item_identification(item_identification)
            .await?;
        let (new_expiry_date, renew_count) = self
            .renew_loan(loan.id, force, audit_actor, client_ip)
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
        async fn loans_create(&self, _: &CreateLoan) -> AppResult<LoanCreateOutcome> {
            Ok(LoanCreateOutcome {
                loan_id: self.loan_id,
                expiry_at: Utc::now(),
                fulfilled_hold_id: None,
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
        async fn loans_renew(&self, _: i64, _: bool) -> AppResult<(chrono::DateTime<Utc>, i16)> {
            Ok((Utc::now() + chrono::Duration::days(21), 1))
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
        ) -> AppResult<Option<rust_decimal::Decimal>> {
            Ok(self.threshold)
        }
        async fn fines_set_global_unpaid_threshold(
            &self,
            threshold: Option<rust_decimal::Decimal>,
        ) -> AppResult<Option<rust_decimal::Decimal>> {
            Ok(threshold)
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
        }
    }

    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_create_loan_active_user_succeeds() {
        let user = make_user(1, None, None);
        let svc = make_service(Some(user), 100);
        assert!(svc
            .create_loan(make_loan(1, false), None, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_blocked_user_rejected() {
        let user = make_user(2, Some(UserStatus::Blocked), None);
        let svc = make_service(Some(user), 0);
        assert!(matches!(
            svc.create_loan(make_loan(2, false), None, None).await,
            Err(AppError::BusinessRule(_))
        ));
    }

    #[tokio::test]
    async fn test_create_loan_blocked_user_with_force_succeeds() {
        let user = make_user(3, Some(UserStatus::Blocked), None);
        let svc = make_service(Some(user), 101);
        assert!(svc
            .create_loan(make_loan(3, true), None, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_deleted_user_always_rejected() {
        let user = make_user(4, Some(UserStatus::Deleted), None);
        let svc = make_service(Some(user), 0);
        // force=true should NOT override a deleted account
        assert!(matches!(
            svc.create_loan(make_loan(4, true), None, None).await,
            Err(AppError::BusinessRule(_))
        ));
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_rejected() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(5, None, Some(expired));
        let svc = make_service(Some(user), 0);
        assert!(matches!(
            svc.create_loan(make_loan(5, false), None, None).await,
            Err(AppError::BusinessRule(_))
        ));
    }

    #[tokio::test]
    async fn test_create_loan_expired_subscription_with_force_succeeds() {
        let expired = Utc::now() - chrono::Duration::days(1);
        let user = make_user(6, None, Some(expired));
        let svc = make_service(Some(user), 102);
        assert!(svc
            .create_loan(make_loan(6, true), None, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_create_loan_user_not_found() {
        let svc = make_service(None, 0); // no user pre-loaded
        assert!(matches!(
            svc.create_loan(make_loan(99, false), None, None).await,
            Err(AppError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_valid_subscription_not_expired() {
        let future_date = Utc::now() + chrono::Duration::days(30);
        let user = make_user(7, None, Some(future_date)); // subscription valid
        let svc = make_service(Some(user), 103);
        assert!(svc
            .create_loan(make_loan(7, false), None, None)
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
            .create_loan(make_loan(15, false), None, None)
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
            .create_loan(make_loan(8, false), None, None)
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
            .create_loan(make_loan(9, false), None, None)
            .await
            .expect_err("over-threshold checkout must fail");
        match err {
            AppError::BusinessRule(msg) => {
                assert!(msg.contains("15"), "{msg}");
                assert!(msg.contains("10"), "{msg}");
                assert!(msg.contains("force=true"), "{msg}");
            }
            other => panic!("expected BusinessRule, got {other:?}"),
        }
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
            .create_loan(make_loan(10, true), Some(99), Some("127.0.0.1".into()))
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
            .create_loan(make_loan(11, false), None, None)
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
        assert!(svc.renew_loan(200, false, None, None).await.is_ok());
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
        match svc.renew_loan(201, false, None, None).await {
            Err(AppError::RenewalRefused { code, message }) => {
                assert_eq!(code, "fines_over_threshold");
                assert!(message.contains("20"), "{message}");
                assert!(message.contains("force=true"), "{message}");
            }
            other => panic!("expected fines_over_threshold, got {other:?}"),
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
        assert!(svc
            .renew_loan(202, true, Some(99), Some("127.0.0.1".into()))
            .await
            .is_ok());
    }
}
