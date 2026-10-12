//! Email notification when a hold becomes `ready` (after a loan return).

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::{
    email::EmailService,
    email_templates,
    error::AppResult,
    models::{hold::Hold, loan::LoanDetails, Language},
    repository::users::HoldReadyUserContact,
    services::{calendar, i18n},
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

    let title = loan_details.biblio.title.as_deref().unwrap_or("");

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

/// Plain and HTML barcode lines. Empty when the copy has no barcode.
#[must_use]
fn barcode_lines(lang: Option<Language>, barcode: Option<&str>) -> (String, String) {
    let Some(barcode) = barcode.map(str::trim).filter(|b| !b.is_empty()) else {
        return (String::new(), String::new());
    };
    let plain = i18n::labeled(lang, i18n::Label::Barcode, barcode);
    let html = i18n::labeled(
        lang,
        i18n::Label::Barcode,
        &format!("<code>{}</code>", html_escape(barcode)),
    );
    (plain, html)
}

/// Catalog title shown to the patron. A blank title, or the historical English
/// sentinel `(unknown title)`, becomes the localized fallback.
#[must_use]
fn patron_title(lang: Option<Language>, title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed == "(unknown title)" {
        i18n::label(lang, i18n::Label::UnknownTitle).to_string()
    } else {
        trimmed.to_string()
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    let (barcode_line, barcode_line_html) = barcode_lines(mail.lang, mail.barcode);
    let title = patron_title(mail.lang, mail.title);

    let expires_at = format_pickup_deadline(hold.expires_at, mail.zone);

    let template = email_svc.load_template("hold_ready", mail.lang).await?;
    let vars: Vec<(&str, &str)> = vec![
        ("firstname", mail.firstname),
        ("lastname", mail.lastname),
        ("title", title.as_str()),
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

    #[test]
    fn barcode_line_follows_the_patron_language() {
        let cases = [
            (
                Language::French,
                "Code-barres : ABC-1",
                "Code-barres : <code>ABC-1</code>",
            ),
            (
                Language::English,
                "Barcode: ABC-1",
                "Barcode: <code>ABC-1</code>",
            ),
            (
                Language::German,
                "Barcode: ABC-1",
                "Barcode: <code>ABC-1</code>",
            ),
            (
                Language::Spanish,
                "Código de barras: ABC-1",
                "Código de barras: <code>ABC-1</code>",
            ),
            (
                Language::Italian,
                "Code-barres : ABC-1",
                "Code-barres : <code>ABC-1</code>",
            ),
        ];
        for (lang, plain, html) in cases {
            assert_eq!(
                barcode_lines(Some(lang), Some("ABC-1")),
                (plain.into(), html.into())
            );
        }
        assert_eq!(barcode_lines(None, Some("ABC-1")).0, "Code-barres : ABC-1");
        assert_eq!(
            barcode_lines(Some(Language::French), None),
            (String::new(), String::new())
        );
        assert_eq!(
            barcode_lines(Some(Language::English), Some("  ")),
            (String::new(), String::new())
        );
    }

    #[test]
    fn barcode_html_escapes_the_value() {
        let (_, html) = barcode_lines(Some(Language::English), Some("A&B<C>"));
        assert_eq!(html, "Barcode: <code>A&amp;B&lt;C&gt;</code>");
        assert!(!html.contains("A&B"));
    }

    #[test]
    fn missing_title_is_localized() {
        assert_eq!(patron_title(Some(Language::French), ""), "(titre inconnu)");
        assert_eq!(
            patron_title(Some(Language::French), "(unknown title)"),
            "(titre inconnu)"
        );
        assert_eq!(
            patron_title(Some(Language::English), "(unknown title)"),
            "(unknown title)"
        );
        assert_eq!(
            patron_title(Some(Language::German), "  "),
            "(unbekannter Titel)"
        );
        assert_eq!(
            patron_title(Some(Language::Spanish), ""),
            "(título desconocido)"
        );
        assert_eq!(
            patron_title(Some(Language::French), "  Le Petit Prince  "),
            "Le Petit Prince"
        );
    }

    #[test]
    fn french_hold_ready_template_says_reservation_disponible() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/email_templates/hold_ready.french.json"
        );
        let raw = std::fs::read_to_string(path).expect("french hold_ready template");
        let template: serde_json::Value =
            serde_json::from_str(&raw).expect("french hold_ready json");
        let text = raw.to_lowercase();
        assert!(
            !text.contains("mise en attente"),
            "hold ready copy must not use « mise en attente »"
        );
        assert_eq!(template["name"], "Réservation disponible");
        assert_eq!(template["subject"], "Votre réservation est disponible");
        let plain = template["body_plain"].as_str().unwrap_or("");
        let html = template["body_html"].as_str().unwrap_or("");
        assert!(plain.contains("Le document que vous avez réservé vous attend"));
        assert!(plain.contains("À retirer jusqu'au {{expires_at}} inclus"));
        assert!(html.contains("Le document que vous avez réservé vous attend"));
        assert!(html.contains("jusqu'au <strong>{{expires_at}}</strong> inclus"));
        assert!(!plain.contains("disponible au retrait"));
        assert!(!html.contains("disponible au retrait"));
    }

    #[test]
    fn migration_rewrites_only_the_shipped_french_default() {
        let sql = include_str!("../migrations/044_hold_ready_french_wording.sql");
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/email_templates/hold_ready.french.json"
        );
        let raw = std::fs::read_to_string(path).expect("french hold_ready template");
        let template: serde_json::Value =
            serde_json::from_str(&raw).expect("french hold_ready json");
        for field in ["name", "subject", "body_plain", "body_html"] {
            let value = template[field].as_str().expect(field);
            assert!(sql.contains(value), "{field} missing from migration");
        }
        assert!(sql.contains("Votre mise en attente est prête au retrait"));
        assert!(sql.contains("AND name = 'Réservation prête au retrait'"));
        assert!(sql.contains("que vous aviez mis en attente"));
    }
}
