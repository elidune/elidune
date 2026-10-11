//! Email notification when a hold becomes `ready` (after a loan return).

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::{
    email::EmailService,
    email_templates,
    error::AppResult,
    models::{hold::Hold, loan::LoanDetails, Language},
    repository::users::HoldReadyUserContact,
    services::calendar,
};

/// Queue a "hold ready" email for the patron via the outbox. No-op if user has no email.
#[tracing::instrument(skip_all, fields(hold_id = hold.id, user_id = hold.user_id))]
pub async fn send_hold_ready(
    email_svc: &EmailService,
    contact: Option<HoldReadyUserContact>,
    hold: &Hold,
    loan_details: &LoanDetails,
    zone: Tz,
) -> AppResult<()> {
    let Some(row) = contact else {
        tracing::warn!(
            user_id = hold.user_id,
            "User not found for hold ready email"
        );
        return Ok(());
    };

    let addr: Option<String> = row.email.clone();
    let to = match addr.as_deref().map(str::trim) {
        Some(e) if !e.is_empty() => e,
        _ => {
            tracing::debug!(
                user_id = hold.user_id,
                "No email — skipping hold ready notification"
            );
            return Ok(());
        }
    };

    let firstname: String = row.firstname.clone().unwrap_or_default();
    let lastname: String = row.lastname.clone().unwrap_or_default();
    let lang = row.language.as_deref().map(Language::from);

    let title = loan_details
        .biblio
        .title
        .as_deref()
        .unwrap_or("(unknown title)");

    let barcode = loan_details
        .biblio
        .items
        .first()
        .and_then(|i| i.barcode.as_deref());

    enqueue_hold_ready(
        email_svc,
        hold,
        HoldReadyMail {
            to,
            firstname: &firstname,
            lastname: &lastname,
            lang,
            title,
            barcode,
            zone,
        },
    )
    .await
}

/// Queue a hold-ready email when the title/barcode are already resolved (transit receive).
#[tracing::instrument(skip_all, fields(hold_id = hold.id, user_id = hold.user_id))]
pub async fn send_hold_ready_simple(
    email_svc: &EmailService,
    contact: Option<HoldReadyUserContact>,
    hold: &Hold,
    title: &str,
    barcode: Option<&str>,
    zone: Tz,
) -> AppResult<()> {
    let Some(row) = contact else {
        return Ok(());
    };
    let addr: Option<String> = row.email.clone();
    let to = match addr.as_deref().map(str::trim) {
        Some(e) if !e.is_empty() => e,
        _ => return Ok(()),
    };
    let firstname: String = row.firstname.clone().unwrap_or_default();
    let lastname: String = row.lastname.clone().unwrap_or_default();
    let lang = row.language.as_deref().map(Language::from);
    enqueue_hold_ready(
        email_svc,
        hold,
        HoldReadyMail {
            to,
            firstname: &firstname,
            lastname: &lastname,
            lang,
            title,
            barcode,
            zone,
        },
    )
    .await
}

/// Pickup deadline shown to the patron, in the library timezone.
#[must_use]
pub fn format_pickup_deadline(expires_at: Option<DateTime<Utc>>, zone: Tz) -> String {
    expires_at
        .map(|instant| calendar::format_patron_datetime(zone, instant))
        .unwrap_or_else(|| "—".to_string())
}

struct HoldReadyMail<'a> {
    to: &'a str,
    firstname: &'a str,
    lastname: &'a str,
    lang: Option<Language>,
    title: &'a str,
    barcode: Option<&'a str>,
    zone: Tz,
}

async fn enqueue_hold_ready(
    email_svc: &EmailService,
    hold: &Hold,
    mail: HoldReadyMail<'_>,
) -> AppResult<()> {
    let barcode_line = mail
        .barcode
        .map(|b| format!("Barcode: {b}"))
        .unwrap_or_default();
    let barcode_line_html = mail
        .barcode
        .map(|b| format!("Barcode: <code>{b}</code>"))
        .unwrap_or_default();

    let expires_at = format_pickup_deadline(hold.expires_at, mail.zone);

    let template = email_svc.load_template("hold_ready", mail.lang).await?;
    let vars: Vec<(&str, &str)> = vec![
        ("firstname", mail.firstname),
        ("lastname", mail.lastname),
        ("title", mail.title),
        ("barcode_line", barcode_line.as_str()),
        ("barcode_line_html", barcode_line_html.as_str()),
        ("expires_at", expires_at.as_str()),
    ];
    let (subject, body_plain, body_html) = email_templates::substitute(&template, &vars);

    email_svc
        .enqueue(mail.to, &subject, &body_plain, &body_html)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn pickup_deadline_is_library_local_time_not_utc() {
        let zone = chrono_tz::Europe::Paris;
        let expires = Utc.with_ymd_and_hms(2026, 7, 15, 22, 30, 0).unwrap();
        let label = format_pickup_deadline(Some(expires), zone);
        assert_eq!(label, "16/07/2026 00:30");
        assert!(!label.contains("UTC"));
    }

    #[test]
    fn pickup_deadline_crosses_the_paris_dst_boundary() {
        let zone = chrono_tz::Europe::Paris;
        let before = Utc.with_ymd_and_hms(2026, 3, 29, 0, 30, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2026, 3, 29, 1, 30, 0).unwrap();
        assert_eq!(
            format_pickup_deadline(Some(before), zone),
            "29/03/2026 01:30"
        );
        assert_eq!(
            format_pickup_deadline(Some(after), zone),
            "29/03/2026 03:30"
        );
    }
}
