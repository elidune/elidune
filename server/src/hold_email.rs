//! Email notification when a hold becomes `ready` (after a loan return).

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::{
    email::EmailService,
    email_templates::{self, EmailTemplate},
    error::AppResult,
    models::{hold::Hold, loan::LoanDetails, Language},
    repository::{users::HoldReadyUserContact, Repository},
    services::{calendar, i18n},
};

/// Queue a "hold ready" email for the patron via the outbox. No-op if user has no email.
#[tracing::instrument(skip_all, fields(hold_id = hold.id, user_id = hold.user_id))]
pub async fn send_hold_ready(email_svc: &EmailService, contact: Option<HoldReadyUserContact>, hold: &Hold, loan_details: &LoanDetails, zone: Tz) -> AppResult<()> {
    let Some(row) = contact else {
        tracing::warn!(user_id = hold.user_id, "User not found for hold ready email");
        return Ok(());
    };

    let addr: Option<String> = row.email.clone();
    let to = match addr.as_deref().map(str::trim) {
        Some(e) if !e.is_empty() => e,
        _ => {
            tracing::debug!(user_id = hold.user_id, "No email — skipping hold ready notification");
            return Ok(());
        }
    };

    let firstname: String = row.firstname.clone().unwrap_or_default();
    let lastname: String = row.lastname.clone().unwrap_or_default();
    let lang = row.language.as_deref().map(Language::from);

    let title = loan_details.biblio.title.as_deref().unwrap_or("");

    let barcode = loan_details.biblio.items.first().and_then(|i| i.barcode.as_deref());

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
pub async fn send_hold_ready_simple(email_svc: &EmailService, contact: Option<HoldReadyUserContact>, hold: &Hold, title: &str, barcode: Option<&str>, zone: Tz) -> AppResult<()> {
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
    expires_at.map(|instant| calendar::format_patron_datetime(zone, instant)).unwrap_or_else(|| "—".to_string())
}

/// Plain and HTML barcode lines. Empty when the copy has no barcode.
#[must_use]
fn barcode_lines(lang: Option<Language>, barcode: Option<&str>) -> (String, String) {
    let Some(barcode) = barcode.map(str::trim).filter(|b| !b.is_empty()) else {
        return (String::new(), String::new());
    };
    let plain = i18n::labeled(lang, i18n::Label::Barcode, barcode);
    let html = i18n::labeled(lang, i18n::Label::Barcode, &format!("<code>{}</code>", html_escape(barcode)));
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
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
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

/// Site name written into `{{pickup_site}}`.
///
/// Only the hold's pickup site is used. `item_home_site` is the copy's current
/// site (`items.site_id`) and is never a fallback.
#[must_use]
fn displayed_pickup_site(pickup_site: Option<&str>, _item_home_site: Option<&str>) -> String {
    pickup_site.map(str::trim).filter(|name| !name.is_empty()).map(str::to_string).unwrap_or_else(|| "—".to_string())
}

struct HoldReadyRender<'a> {
    lang: Option<Language>,
    firstname: &'a str,
    lastname: &'a str,
    title: &'a str,
    barcode: Option<&'a str>,
    expires_at: &'a str,
    pickup_site: Option<&'a str>,
    item_home_site: Option<&'a str>,
}

/// Render a hold-ready notice. `{{pickup_site}}` is the hold pickup site.
fn compose_hold_ready(template: &EmailTemplate, input: &HoldReadyRender<'_>) -> (String, String, String) {
    let (barcode_line, barcode_line_html) = barcode_lines(input.lang, input.barcode);
    let title = patron_title(input.lang, input.title);
    let pickup = displayed_pickup_site(input.pickup_site, input.item_home_site);
    let pickup_html = html_escape(&pickup);
    let shared = [
        ("firstname", input.firstname),
        ("lastname", input.lastname),
        ("title", title.as_str()),
        ("barcode_line", barcode_line.as_str()),
        ("barcode_line_html", barcode_line_html.as_str()),
        ("expires_at", input.expires_at),
    ];
    let mut plain_vars = shared.to_vec();
    plain_vars.push(("pickup_site", pickup.as_str()));
    let (subject, body_plain, _) = email_templates::substitute(template, &plain_vars);
    let mut html_vars = shared.to_vec();
    html_vars.push(("pickup_site", pickup_html.as_str()));
    let (_, _, body_html) = email_templates::substitute(template, &html_vars);
    (subject, body_plain, body_html)
}

async fn enqueue_hold_ready(email_svc: &EmailService, hold: &Hold, mail: HoldReadyMail<'_>) -> AppResult<()> {
    let library = email_svc.library_notice_language().await;
    let (language, template) = email_svc.load_notice_template("hold_ready", mail.lang, library).await?;
    let expires_at = format_pickup_deadline(hold.expires_at, mail.zone);
    let repo = Repository::new(email_svc.pool().clone(), None);
    let (pickup_site, item_home_site) = repo.holds_notice_sites(hold.id).await?;
    // Labels and the pickup site follow the template that was actually loaded.
    let (subject, body_plain, body_html) = compose_hold_ready(
        &template,
        &HoldReadyRender {
            lang: Some(language),
            firstname: mail.firstname,
            lastname: mail.lastname,
            title: mail.title,
            barcode: mail.barcode,
            expires_at: expires_at.as_str(),
            pickup_site: pickup_site.as_deref(),
            item_home_site: item_home_site.as_deref(),
        },
    );

    email_svc.enqueue(mail.to, &subject, &body_plain, &body_html).await.map(|_| ())
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
        assert_eq!(format_pickup_deadline(Some(before), zone), "29/03/2026 01:30");
        assert_eq!(format_pickup_deadline(Some(after), zone), "29/03/2026 03:30");
    }

    #[test]
    fn barcode_line_follows_the_patron_language() {
        let cases = [
            (Language::French, "Code-barres : ABC-1", "Code-barres : <code>ABC-1</code>"),
            (Language::English, "Barcode: ABC-1", "Barcode: <code>ABC-1</code>"),
            (Language::German, "Barcode: ABC-1", "Barcode: <code>ABC-1</code>"),
            (Language::Spanish, "Código de barras: ABC-1", "Código de barras: <code>ABC-1</code>"),
            (Language::Italian, "Code-barres : ABC-1", "Code-barres : <code>ABC-1</code>"),
        ];
        for (lang, plain, html) in cases {
            assert_eq!(barcode_lines(Some(lang), Some("ABC-1")), (plain.into(), html.into()));
        }
        assert_eq!(barcode_lines(None, Some("ABC-1")).0, "Code-barres : ABC-1");
        assert_eq!(barcode_lines(Some(Language::French), None), (String::new(), String::new()));
        assert_eq!(barcode_lines(Some(Language::English), Some("  ")), (String::new(), String::new()));
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
        assert_eq!(patron_title(Some(Language::French), "(unknown title)"), "(titre inconnu)");
        assert_eq!(patron_title(Some(Language::English), "(unknown title)"), "(unknown title)");
        assert_eq!(patron_title(Some(Language::German), "  "), "(unbekannter Titel)");
        assert_eq!(patron_title(Some(Language::Spanish), ""), "(título desconocido)");
        assert_eq!(patron_title(Some(Language::French), "  Le Petit Prince  "), "Le Petit Prince");
    }

    #[test]
    fn french_hold_ready_template_says_reservation_disponible() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/email_templates/hold_ready.french.json");
        let raw = std::fs::read_to_string(path).expect("french hold_ready template");
        let template: serde_json::Value = serde_json::from_str(&raw).expect("french hold_ready json");
        let text = raw.to_lowercase();
        assert!(!text.contains("mise en attente"), "hold ready copy must not use « mise en attente »");
        assert_eq!(template["name"], "Réservation disponible");
        assert_eq!(template["subject"], "Votre réservation est disponible");
        let plain = template["body_plain"].as_str().unwrap_or("");
        let html = template["body_html"].as_str().unwrap_or("");
        assert!(plain.contains("Le document que vous avez réservé vous attend à {{pickup_site}}, à retirer jusqu'au {{expires_at}} inclus."));
        assert!(html.contains("vous attend à <strong>{{pickup_site}}</strong>, à retirer jusqu'au <strong>{{expires_at}}</strong> inclus."));
        assert!(!plain.contains("disponible au retrait"));
        assert!(!html.contains("disponible au retrait"));
    }

    #[test]
    fn migration_rewrites_only_the_shipped_french_default() {
        let sql = include_str!("../migrations/044_hold_ready_french_wording.sql");
        assert!(sql.contains("Réservation disponible"));
        assert!(sql.contains("Votre réservation est disponible"));
        assert!(sql.contains("Le document que vous avez réservé vous attend :"));
        assert!(sql.contains("À retirer jusqu'au {{expires_at}} inclus."));
        assert!(sql.contains("Votre mise en attente est prête au retrait"));
        assert!(sql.contains("AND name = 'Réservation prête au retrait'"));
        assert!(sql.contains("que vous aviez mis en attente"));
        assert!(!sql.contains("{{pickup_site}}"), "pickup site wording belongs to migration 056");
    }

    fn hold_ready_file(language: &str) -> EmailTemplate {
        let path = format!("{}/data/email_templates/hold_ready.{language}.json", env!("CARGO_MANIFEST_DIR"));
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {path}"));
        let template: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|_| panic!("parse {path}"));
        EmailTemplate {
            subject: template["subject"].as_str().unwrap_or_else(|| panic!("{language} subject")).to_string(),
            body_plain: template["body_plain"].as_str().unwrap_or_else(|| panic!("{language} body")).to_string(),
            body_html: Some(template["body_html"].as_str().unwrap_or_else(|| panic!("{language} html")).to_string()),
        }
    }

    fn render_for(language: &str, lang: Language, pickup: &str, home: &str) -> (String, String, String) {
        compose_hold_ready(
            &hold_ready_file(language),
            &HoldReadyRender {
                lang: Some(lang),
                firstname: "Ada",
                lastname: "Lovelace",
                title: "Le Petit Prince",
                barcode: Some("ABC-1"),
                expires_at: "16/07/2026 00:30",
                pickup_site: Some(pickup),
                item_home_site: Some(home),
            },
        )
    }

    #[test]
    fn render_shows_pickup_site_when_it_differs_from_the_item_site() {
        let pickup = "Médiathèque Centre";
        let home = "Annexe Nord";
        let cases = [
            ("french", Language::French, "vous attend à Médiathèque Centre"),
            ("english", Language::English, "waiting for you at Médiathèque Centre"),
            ("german", Language::German, "wartet in Médiathèque Centre"),
            ("spanish", Language::Spanish, "le espera en Médiathèque Centre"),
        ];
        for (file_lang, lang, snippet) in cases {
            let (_, plain, html) = render_for(file_lang, lang, pickup, home);
            assert!(plain.contains(snippet), "{file_lang} plain missing pickup wording: {plain}");
            assert!(plain.contains(pickup) && html.contains(pickup), "{file_lang} dropped the pickup site");
            assert!(!plain.contains(home) && !html.contains(home), "{file_lang} showed the item site: {plain}");
            assert!(plain.contains("16/07/2026 00:30"));
        }
    }

    #[test]
    fn missing_pickup_site_does_not_fall_back_to_the_item_site() {
        let template = hold_ready_file("french");
        let (_, plain, html) = compose_hold_ready(
            &template,
            &HoldReadyRender {
                lang: Some(Language::French),
                firstname: "Ada",
                lastname: "Lovelace",
                title: "Le Petit Prince",
                barcode: None,
                expires_at: "16/07/2026 00:30",
                pickup_site: Some("   "),
                item_home_site: Some("Annexe Nord"),
            },
        );
        assert!(plain.contains("vous attend à —, à retirer"));
        assert!(html.contains("vous attend à <strong>—</strong>"));
        assert!(!plain.contains("Annexe Nord"));
        assert!(!html.contains("Annexe Nord"));
        assert_eq!(displayed_pickup_site(None, Some("Annexe Nord")), "—");
    }

    #[test]
    fn pickup_site_is_escaped_in_html_only() {
        let (_, plain, html) = render_for("english", Language::English, "A & B <C>", "Home Site");
        assert!(plain.contains("at A & B <C>"));
        assert!(html.contains("at <strong>A &amp; B &lt;C&gt;</strong>"));
        assert!(!html.contains("A & B"));
        assert!(!plain.contains("Home Site"));
    }

    #[test]
    fn migration_056_rewrites_only_unmodified_hold_ready_rows() {
        let sql = include_str!("../migrations/056_hold_ready_pickup_site.sql");
        for language in ["french", "english"] {
            let template = hold_ready_file(language);
            assert!(sql.contains(template.subject.as_str()), "{language} subject missing from migration");
            assert!(sql.contains(template.body_plain.as_str()), "{language} plain body missing from migration");
            let html = template.body_html.as_deref().unwrap_or("");
            assert!(sql.contains(html), "{language} html missing from migration");
        }
        assert!(sql.contains("template_id = 'hold_ready'"));
        assert!(sql.contains("AND language = 'french'"));
        assert!(sql.contains("AND language = 'english'"));
        assert!(sql.contains("Le document que vous avez réservé vous attend :"));
        assert!(sql.contains("The following item you placed on hold is now available:"));
        assert!(sql.contains("{{pickup_site}}"));
        assert!(!sql.to_lowercase().contains("account_type"));
    }
}
