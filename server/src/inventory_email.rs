//! Email notifications when inventory consolidation closes active loans (`force: true`).

use std::collections::HashMap;

use crate::{
    email::EmailService,
    email_templates,
    models::Language,
    repository::inventory::InventoryLoanClosureRow,
    services::{
        audit::{self, AuditLogMeta, AuditService},
        i18n,
    },
};

/// Queue one email per reader listing all loans closed during consolidation.
pub async fn send_loan_closure_notifications(
    email_svc: &EmailService,
    audit: &AuditService,
    session_id: i64,
    session_name: &str,
    rows: &[InventoryLoanClosureRow],
) -> (u64, Vec<(i64, String, String)>) {
    let mut by_user: HashMap<i64, Vec<&InventoryLoanClosureRow>> = HashMap::new();
    for row in rows {
        by_user.entry(row.user_id).or_default().push(row);
    }

    let mut sent = 0u64;
    let mut errors = Vec::new();

    for (user_id, user_rows) in by_user {
        let first = user_rows[0];
        let to = match first.user_email.as_deref().map(str::trim) {
            Some(e) if !e.is_empty() => e,
            _ => {
                tracing::debug!(user_id, "No email — skipping inventory loan closure notification");
                continue;
            }
        };

        let firstname = first.user_firstname.clone().unwrap_or_default();
        let lastname = first.user_lastname.clone().unwrap_or_default();
        let lang = first.user_language.as_deref().map(Language::from);

        let items_list = user_rows.iter().map(|r| format_item_line(lang, r)).collect::<Vec<_>>().join("\n");

        let items_list_html = format!(
            "<ul>{}</ul>",
            user_rows.iter().map(|r| format!("<li>{}</li>", html_escape(&format_item_line(lang, r)))).collect::<String>()
        );

        match email_svc.load_template("inventory_loan_closed", lang).await {
            Ok(template) => {
                let vars: Vec<(&str, &str)> = vec![
                    ("firstname", firstname.as_str()),
                    ("lastname", lastname.as_str()),
                    ("session_name", session_name),
                    ("items_list", items_list.as_str()),
                    ("items_list_html", items_list_html.as_str()),
                ];
                let (subject, body_plain, body_html) = email_templates::substitute(&template, &vars);
                match email_svc.enqueue(to, &subject, &body_plain, &body_html).await {
                    Ok(outbox_id) => {
                        sent += 1;
                        let loan_ids: Vec<i64> = user_rows.iter().map(|r| r.loan_id).collect();
                        let item_ids: Vec<i64> = user_rows.iter().map(|r| r.item_id).collect();
                        audit.log(
                            audit::event::EMAIL_INVENTORY_LOAN_CLOSED,
                            None,
                            Some("inventory_session"),
                            Some(session_id),
                            None,
                            Some(serde_json::json!({
                                "session_name": session_name,
                                "user_id": user_id,
                                "email": to,
                                "loan_count": user_rows.len(),
                                "loan_ids": loan_ids,
                                "item_ids": item_ids,
                                "outbox_id": outbox_id,
                                "delivery": "queued",
                                "trigger": "inventory_consolidation",
                            })),
                            AuditLogMeta::success(),
                        );
                    }
                    Err(e) => {
                        errors.push((user_id, to.to_string(), e.to_string()));
                        audit.log(
                            audit::event::EMAIL_INVENTORY_LOAN_CLOSED,
                            None,
                            Some("user"),
                            Some(user_id),
                            None,
                            Some(serde_json::json!({
                                "session_id": session_id,
                                "session_name": session_name,
                                "email": to,
                                "loan_count": user_rows.len(),
                                "delivery": "queue_failed",
                                "trigger": "inventory_consolidation",
                            })),
                            AuditLogMeta::failure_background("email_queue_failed", e.to_string()),
                        );
                    }
                }
            }
            Err(e) => {
                errors.push((user_id, to.to_string(), e.to_string()));
                audit.log(
                    audit::event::EMAIL_INVENTORY_LOAN_CLOSED,
                    None,
                    Some("user"),
                    Some(user_id),
                    None,
                    Some(serde_json::json!({
                        "session_id": session_id,
                        "session_name": session_name,
                        "email": to,
                        "trigger": "inventory_consolidation",
                    })),
                    AuditLogMeta::failure_background("email_delivery_failed", e.to_string()),
                );
            }
        }
    }

    (sent, errors)
}

fn format_item_line(lang: Option<Language>, row: &InventoryLoanClosureRow) -> String {
    let fallback = i18n::label(lang, i18n::Label::UnknownTitle);
    let title = row.biblio_title.as_deref().map(str::trim).filter(|t| !t.is_empty()).unwrap_or(fallback);
    match row.barcode.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        Some(barcode) => format!("- {title} ({})", i18n::labeled(lang, i18n::Label::Barcode, barcode)),
        None => format!("- {title}"),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: Option<&str>, barcode: Option<&str>) -> InventoryLoanClosureRow {
        InventoryLoanClosureRow {
            item_id: 1,
            barcode: barcode.map(str::to_string),
            biblio_title: title.map(str::to_string),
            loan_id: 2,
            user_id: 3,
            user_email: None,
            user_firstname: None,
            user_lastname: None,
            user_language: None,
        }
    }

    #[test]
    fn item_line_localizes_barcode_and_missing_title() {
        assert_eq!(format_item_line(Some(Language::French), &row(Some("Kim"), Some("BC-9"))), "- Kim (Code-barres : BC-9)");
        assert_eq!(format_item_line(Some(Language::English), &row(Some("Kim"), Some("BC-9"))), "- Kim (Barcode: BC-9)");
        assert_eq!(format_item_line(Some(Language::German), &row(Some("Kim"), Some("BC-9"))), "- Kim (Barcode: BC-9)");
        assert_eq!(format_item_line(Some(Language::Spanish), &row(Some("Kim"), Some("BC-9"))), "- Kim (Código de barras: BC-9)");
        assert_eq!(format_item_line(Some(Language::Italian), &row(None, None)), "- (titre inconnu)");
        assert_eq!(format_item_line(Some(Language::English), &row(Some("Kim"), Some("  "))), "- Kim");
        assert!(!format_item_line(Some(Language::French), &row(Some("Kim"), Some("BC-9"))).contains("barcode:"));
    }
}
