//! Overdue loan reminder service.
//!
//! Groups overdue loans by patron and next reminder tier, enqueues one email per
//! (patron, tier) listing those loans, and records audit events. Loan
//! `reminder_count` advances after SMTP delivery (outbox `sent`).

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    config::RemindersConfig,
    dynamic_config::DynamicConfig,
    error::AppResult,
    models::{circulation::CirculationStatus, Language},
    repository::{loans::ReminderTierDelays, LoansRepository},
    services::{
        audit::{self, AuditService},
        email::EmailService,
        email_templates,
    },
};

/// Graduated overdue-notice tier stored as `loans.reminder_count` (0/1/2 → next tier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ReminderTier {
    First,
    Second,
    FormalNotice,
}

impl ReminderTier {
    /// Next tier to send for a loan that has already received `reminder_count` notices.
    /// `None` when every tier has been sent (`reminder_count >= 3`).
    #[must_use]
    pub fn from_count(reminder_count: i32) -> Option<Self> {
        match reminder_count {
            0 => Some(Self::First),
            1 => Some(Self::Second),
            2 => Some(Self::FormalNotice),
            _ => None,
        }
    }

    #[must_use]
    pub fn delay_days(self, cfg: &RemindersConfig) -> u32 {
        match self {
            Self::First => cfg.first_reminder_delay_days,
            Self::Second => cfg.second_reminder_delay_days,
            Self::FormalNotice => cfg.formal_notice_delay_days,
        }
    }

    #[must_use]
    pub fn template_id(self, cfg: &RemindersConfig) -> &str {
        match self {
            Self::First => cfg.first_reminder_template.as_str(),
            Self::Second => cfg.second_reminder_template.as_str(),
            Self::FormalNotice => cfg.formal_notice_template.as_str(),
        }
    }

    #[must_use]
    pub fn is_enabled(self, cfg: &RemindersConfig) -> bool {
        match self {
            Self::First => cfg.first_reminder_enabled,
            Self::Second => cfg.second_reminder_enabled,
            Self::FormalNotice => cfg.formal_notice_enabled,
        }
    }

    /// Whether `overdue_days` meets this tier's delay. Does not check prior tiers.
    #[must_use]
    pub fn is_due(self, overdue_days: i64, cfg: &RemindersConfig) -> bool {
        overdue_days >= i64::from(self.delay_days(cfg))
    }
}

/// Next tier shown on the overdue list, or `None` when sending must stop.
#[must_use]
pub fn next_reminder_tier(
    reminder_count: i32,
    circulation_status: Option<i16>,
    cfg: &RemindersConfig,
) -> Option<ReminderTier> {
    match CirculationStatus::from_db(circulation_status) {
        CirculationStatus::Lost | CirculationStatus::ClaimedReturned => None,
        _ => {
            let next = ReminderTier::from_count(reminder_count)?;
            next.is_enabled(cfg).then_some(next)
        }
    }
}

/// Summary returned by a reminder run
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReminderReport {
    /// Whether this was a dry run (no emails actually sent)
    pub dry_run: bool,
    /// Number of emails successfully queued (or that would be queued in dry-run)
    pub emails_sent: u32,
    /// Overdue loans covered by queued emails; DB tracking updates after SMTP delivery
    pub loans_reminded: u32,
    /// Per-user details
    pub details: Vec<ReminderDetail>,
    /// Errors encountered (email not sent for these users)
    pub errors: Vec<ReminderError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDetail {
    pub user_id: i64,
    pub email: String,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub loan_count: usize,
    /// Tier this mail covers (`first`, `second`, or `formalNotice`).
    pub tier: ReminderTier,
    /// Template id used (or that would be used on a dry run).
    pub template_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReminderError {
    pub user_id: i64,
    pub email: String,
    pub error_message: String,
}

/// Overdue loan item for the admin dashboard
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverdueLoanInfo {
    pub loan_id: i64,
    pub user_id: i64,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub user_email: Option<String>,
    pub biblio_id: i64,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub item_barcode: Option<String>,
    pub loan_date: DateTime<Utc>,
    pub expiry_at: Option<DateTime<Utc>>,
    pub last_reminder_sent_at: Option<DateTime<Utc>>,
    pub reminder_count: i32,
    /// Next notice that will be sent; `null` after the formal notice or when
    /// the item is lost / claimed-returned (sending has stopped).
    pub next_reminder_tier: Option<ReminderTier>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverdueLoansPage {
    pub loans: Vec<OverdueLoanInfo>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Clone)]
pub struct RemindersService {
    repository: Arc<dyn LoansRepository>,
    email: EmailService,
    audit: AuditService,
    dynamic_config: Arc<DynamicConfig>,
}

impl RemindersService {
    pub fn new(
        repository: Arc<dyn LoansRepository>,
        email: EmailService,
        audit: AuditService,
        dynamic_config: Arc<DynamicConfig>,
    ) -> Self {
        Self {
            repository,
            email,
            audit,
            dynamic_config,
        }
    }

    /// Get paginated overdue loans for the admin dashboard.
    #[tracing::instrument(skip(self), err)]
    pub async fn get_overdue_loans(&self, page: i64, per_page: i64) -> AppResult<OverdueLoansPage> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 200);
        let reminders_cfg = self.dynamic_config.read_reminders();
        let (rows, total) = self.repository.loans_get_overdue(page, per_page).await?;

        let loans = rows
            .into_iter()
            .map(|r| OverdueLoanInfo {
                loan_id: r.loan_id,
                user_id: r.user_id,
                firstname: r.firstname,
                lastname: r.lastname,
                user_email: r.user_email,
                biblio_id: r.biblio_id,
                title: r.title,
                authors: r.authors,
                item_barcode: r.item_barcode,
                loan_date: r.loan_date,
                expiry_at: r.expiry_at,
                last_reminder_sent_at: r.last_reminder_sent_at,
                reminder_count: r.reminder_count,
                next_reminder_tier: next_reminder_tier(
                    r.reminder_count,
                    r.circulation_status,
                    &reminders_cfg,
                ),
            })
            .collect();

        Ok(OverdueLoansPage {
            loans,
            total,
            page,
            per_page,
        })
    }

    /// Enqueue overdue reminder emails for delivery by the outbox worker.
    /// If `dry_run` is true, builds the report but does NOT enqueue emails or update the DB.
    #[tracing::instrument(skip(self), err)]
    pub async fn send_overdue_reminders(
        &self,
        dry_run: bool,
        triggered_by: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<ReminderReport> {
        let reminders_cfg = self.dynamic_config.read_reminders();
        let delays = ReminderTierDelays::from_config(&reminders_cfg);

        let overdue_rows = self
            .repository
            .loans_get_overdue_for_reminders(delays)
            .await?;

        if overdue_rows.is_empty() {
            return Ok(ReminderReport {
                dry_run,
                emails_sent: 0,
                loans_reminded: 0,
                details: vec![],
                errors: vec![],
            });
        }

        // One mail per patron per tier (same-tier loans share a template).
        let mut by_user_tier: HashMap<(i64, ReminderTier), Vec<_>> = HashMap::new();
        for row in &overdue_rows {
            let Some(tier) = ReminderTier::from_count(row.reminder_count)
                .filter(|t| t.is_enabled(&reminders_cfg))
            else {
                continue;
            };
            by_user_tier
                .entry((row.user_id, tier))
                .or_default()
                .push(row);
        }

        let mut details = Vec::new();
        let mut errors = Vec::new();
        let mut queued_loan_ids: Vec<i64> = Vec::new();

        for ((user_id, tier), loans) in &by_user_tier {
            let first = loans[0];
            let email_addr = match &first.user_email {
                Some(e) if !e.is_empty() => e.clone(),
                _ => continue,
            };

            let firstname = first.firstname.as_deref().unwrap_or("");
            let lastname = first.lastname.as_deref().unwrap_or("");
            let lang = first.user_language.as_deref().map(Language::from);
            let template_id = tier.template_id(&reminders_cfg).to_string();
            let loans_list = loan_list_plain(loans);
            let loans_table_html = loan_table_html(loans);

            if !dry_run {
                let template_result = self.email.load_template(&template_id, lang).await;

                match template_result {
                    Err(e) => {
                        errors.push(ReminderError {
                            user_id: *user_id,
                            email: email_addr.clone(),
                            error_message: format!("Template load error: {}", e),
                        });
                        continue;
                    }
                    Ok(template) => {
                        let vars: Vec<(&str, &str)> = vec![
                            ("firstname", firstname),
                            ("lastname", lastname),
                            ("loans_list", &loans_list),
                            ("loans_table_html", &loans_table_html),
                        ];
                        let (subject, body_plain, body_html) =
                            email_templates::substitute(&template, &vars);

                        match self
                            .email
                            .enqueue_overdue_reminder(
                                &email_addr,
                                &subject,
                                &body_plain,
                                &body_html,
                                &loans.iter().map(|l| l.loan_id).collect::<Vec<_>>(),
                            )
                            .await
                        {
                            Ok(outbox_id) => {
                                let loan_ids: Vec<i64> = loans.iter().map(|l| l.loan_id).collect();
                                queued_loan_ids.extend(&loan_ids);

                                self.audit.log(
                                    audit::event::EMAIL_OVERDUE_REMINDER_QUEUED,
                                    triggered_by,
                                    Some("user"),
                                    Some(*user_id),
                                    client_ip.clone(),
                                    Some(serde_json::json!({
                                        "email": email_addr,
                                        "loan_ids": loan_ids,
                                        "loan_count": loans.len(),
                                        "outbox_id": outbox_id,
                                        "tier": tier,
                                        "template_id": template_id,
                                    })),
                                    audit::AuditLogMeta::success(),
                                );

                                details.push(ReminderDetail {
                                    user_id: *user_id,
                                    email: email_addr.clone(),
                                    firstname: first.firstname.clone(),
                                    lastname: first.lastname.clone(),
                                    loan_count: loans.len(),
                                    tier: *tier,
                                    template_id: template_id.clone(),
                                });
                            }
                            Err(e) => {
                                self.audit.log(
                                    audit::event::EMAIL_OVERDUE_REMINDER_QUEUED,
                                    triggered_by,
                                    Some("user"),
                                    Some(*user_id),
                                    client_ip.clone(),
                                    Some(serde_json::json!({
                                        "email": email_addr,
                                        "loan_count": loans.len(),
                                        "tier": tier,
                                    })),
                                    audit::AuditLogMeta::from_app_error(&e),
                                );
                                errors.push(ReminderError {
                                    user_id: *user_id,
                                    email: email_addr.clone(),
                                    error_message: e.to_string(),
                                });
                            }
                        }
                    }
                }
            } else {
                details.push(ReminderDetail {
                    user_id: *user_id,
                    email: email_addr.clone(),
                    firstname: first.firstname.clone(),
                    lastname: first.lastname.clone(),
                    loan_count: loans.len(),
                    tier: *tier,
                    template_id,
                });
            }
        }

        let emails_sent = details.len() as u32;
        let loans_reminded = queued_loan_ids.len() as u32;

        Ok(ReminderReport {
            dry_run,
            emails_sent,
            loans_reminded,
            details,
            errors,
        })
    }
}

fn loan_list_plain(loans: &[&crate::repository::loans::OverdueLoanRow]) -> String {
    loans
        .iter()
        .map(|l| {
            let title = l.title.as_deref().unwrap_or("(unknown title)");
            let authors = l.authors.as_deref().unwrap_or("");
            let loan_date = l.loan_date.format("%d/%m/%Y").to_string();
            let due_date = l
                .expiry_at
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| "N/A".to_string());
            format!(
                "- {} ({}) — borrowed: {}, due: {}",
                title, authors, loan_date, due_date
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn loan_table_html(loans: &[&crate::repository::loans::OverdueLoanRow]) -> String {
    let table_rows = loans
        .iter()
        .map(|l| {
            let title = l.title.as_deref().unwrap_or("(unknown title)");
            let authors = l.authors.as_deref().unwrap_or("");
            let loan_date = l.loan_date.format("%d/%m/%Y").to_string();
            let due_date = l
                .expiry_at
                .map(|d| d.format("%d/%m/%Y").to_string())
                .unwrap_or_else(|| "N/A".to_string());
            format!(
                "<tr><td style=\"padding:4px 8px;border:1px solid #ccc\">{}</td>\
                 <td style=\"padding:4px 8px;border:1px solid #ccc\">{}</td>\
                 <td style=\"padding:4px 8px;border:1px solid #ccc\">{}</td>\
                 <td style=\"padding:4px 8px;border:1px solid #ccc;color:#c00\"><strong>{}</strong></td></tr>",
                title, authors, loan_date, due_date
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<table style=\"border-collapse:collapse;width:100%\">\
         <thead><tr>\
         <th style=\"padding:4px 8px;border:1px solid #ccc;background:#f5f5f5\">Title</th>\
         <th style=\"padding:4px 8px;border:1px solid #ccc;background:#f5f5f5\">Author(s)</th>\
         <th style=\"padding:4px 8px;border:1px solid #ccc;background:#f5f5f5\">Borrowed</th>\
         <th style=\"padding:4px 8px;border:1px solid #ccc;background:#f5f5f5\">Due date</th>\
         </tr></thead><tbody>{}</tbody></table>",
        table_rows
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RemindersConfig {
        RemindersConfig::default()
    }

    #[test]
    fn from_count_advances_one_tier_and_never_skips() {
        assert_eq!(ReminderTier::from_count(0), Some(ReminderTier::First));
        assert_eq!(ReminderTier::from_count(1), Some(ReminderTier::Second));
        assert_eq!(
            ReminderTier::from_count(2),
            Some(ReminderTier::FormalNotice)
        );
        assert_eq!(ReminderTier::from_count(3), None);
        assert_eq!(ReminderTier::from_count(99), None);
    }

    #[test]
    fn from_count_never_returns_a_prior_tier() {
        assert_ne!(ReminderTier::from_count(1), Some(ReminderTier::First));
        assert_ne!(ReminderTier::from_count(2), Some(ReminderTier::First));
        assert_ne!(ReminderTier::from_count(2), Some(ReminderTier::Second));
    }

    #[test]
    fn stop_when_lost_or_claimed_returned() {
        let cfg = cfg();
        assert_eq!(
            next_reminder_tier(0, Some(CirculationStatus::LOST), &cfg),
            None
        );
        assert_eq!(
            next_reminder_tier(0, Some(CirculationStatus::CLAIMED_RETURNED), &cfg),
            None
        );
        assert_eq!(
            next_reminder_tier(0, Some(CirculationStatus::DAMAGED), &cfg),
            Some(ReminderTier::First)
        );
        assert_eq!(next_reminder_tier(0, None, &cfg), Some(ReminderTier::First));
    }

    #[test]
    fn disabled_formal_notice_stops_at_second_and_does_not_loop() {
        let mut cfg = cfg();
        cfg.formal_notice_enabled = false;
        cfg.normalize_tier_enabled();
        assert!(cfg.second_reminder_enabled);
        assert!(!cfg.formal_notice_enabled);
        assert_eq!(
            next_reminder_tier(1, None, &cfg),
            Some(ReminderTier::Second)
        );
        assert_eq!(next_reminder_tier(2, None, &cfg), None);
        assert_eq!(cfg.max_sendable_reminder_count(), 2);
    }

    #[test]
    fn disabling_second_also_disables_formal_notice() {
        let mut cfg = cfg();
        cfg.second_reminder_enabled = false;
        cfg.formal_notice_enabled = true;
        cfg.normalize_tier_enabled();
        assert!(cfg.first_reminder_enabled);
        assert!(!cfg.second_reminder_enabled);
        assert!(!cfg.formal_notice_enabled);
        assert_eq!(next_reminder_tier(0, None, &cfg), Some(ReminderTier::First));
        assert_eq!(next_reminder_tier(1, None, &cfg), None);
        assert_eq!(next_reminder_tier(2, None, &cfg), None);
        assert_eq!(cfg.max_sendable_reminder_count(), 1);
    }

    #[test]
    fn loan_already_at_highest_enabled_tier_is_not_sent_again() {
        let mut cfg = cfg();
        cfg.formal_notice_enabled = false;
        cfg.normalize_tier_enabled();
        assert_eq!(next_reminder_tier(2, None, &cfg), None);
        assert_eq!(
            next_reminder_tier(1, None, &cfg),
            Some(ReminderTier::Second)
        );

        cfg.second_reminder_enabled = false;
        cfg.normalize_tier_enabled();
        assert_eq!(next_reminder_tier(1, None, &cfg), None);
        assert_eq!(next_reminder_tier(0, None, &cfg), Some(ReminderTier::First));
    }

    #[test]
    fn delay_does_not_skip_to_formal_on_first_send() {
        let cfg = cfg();
        let overdue = 40;
        let next = ReminderTier::from_count(0).expect("first");
        assert_eq!(next, ReminderTier::First);
        assert!(next.is_due(overdue, &cfg));
        assert!(ReminderTier::FormalNotice.is_due(overdue, &cfg));
    }

    #[test]
    fn templates_are_distinct_per_tier() {
        let cfg = cfg();
        assert_eq!(ReminderTier::First.template_id(&cfg), "overdue_reminder");
        assert_eq!(
            ReminderTier::Second.template_id(&cfg),
            "overdue_second_reminder"
        );
        assert_eq!(
            ReminderTier::FormalNotice.template_id(&cfg),
            "overdue_formal_notice"
        );
    }
}
