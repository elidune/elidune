//! Email notification when a hold becomes `ready` (after a loan return).

use crate::{
    email::EmailService,
    email_templates,
    error::AppResult,
    models::{hold::Hold, loan::LoanDetails, Language},
    repository::users::HoldReadyUserContact,
};

/// Queue a "hold ready" email for the patron via the outbox. No-op if user has no email.
#[tracing::instrument(skip_all, fields(hold_id = hold.id, user_id = hold.user_id))]
pub async fn send_hold_ready(
    email_svc: &EmailService,
    contact: Option<HoldReadyUserContact>,
    hold: &Hold,
    loan_details: &LoanDetails,
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
        },
    )
    .await
}

struct HoldReadyMail<'a> {
    to: &'a str,
    firstname: &'a str,
    lastname: &'a str,
    lang: Option<Language>,
    title: &'a str,
    barcode: Option<&'a str>,
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

    let expires_at = hold
        .expires_at
        .map(|d| d.format("%d/%m/%Y %H:%M UTC").to_string())
        .unwrap_or_else(|| "—".to_string());

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
