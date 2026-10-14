//! Email template loading and variable substitution.
//!
//! Templates are stored in the `email_templates` DB table (editable via
//! `/settings/email-templates` API). The on-disk JSON files under
//! [`crate::config::EmailConfig::templates_dir`] act as a packaged fallback
//! and as the source for the one-time bootstrap performed at startup.

use std::path::{Path, PathBuf};

use sqlx::{Pool, Postgres};

use crate::{
    error::{AppError, AppResult},
    models::Language,
    repository::Repository,
};

#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub body_plain: String,
    pub body_html: Option<String>,
}

/// Canonical list of template ids the server expects on disk / in DB.
pub const KNOWN_TEMPLATE_IDS: &[&str] = &[
    "2fa_code",
    "recovery_code",
    "password_reset",
    "hold_ready",
    "inventory_loan_closed",
    "overdue_reminder",
    "overdue_second_reminder",
    "overdue_formal_notice",
    "event_announcement",
];

/// Languages bootstrapped and accepted by the template API.
///
/// `german` and `spanish` are the library UI languages `de` and `es`.
pub const SUPPORTED_LANGUAGES: &[&str] = &["english", "french", "german", "spanish"];

#[derive(serde::Deserialize)]
struct RawTemplate {
    name: String,
    subject: String,
    body_plain: String,
    #[serde(default)]
    body_html: Option<String>,
}

/// Reader preference, then the library language, then French, then English.
///
/// A language outside the notice set (for example Italian) is skipped. French is
/// always present. English is only reached when the French template is missing.
/// Duplicates are dropped, so a French reader in a French library is tried once.
///
/// The first language in this list that has a template is the language of the
/// whole notice, including barcode and loan-list labels.
#[must_use]
pub(crate) fn notice_language_order(
    patron: Option<Language>,
    library: Option<Language>,
) -> Vec<Language> {
    let mut order = Vec::with_capacity(4);
    for candidate in [
        patron,
        library,
        Some(Language::French),
        Some(Language::English),
    ]
    .into_iter()
    .flatten()
    {
        if !is_notice_language(candidate) || order.contains(&candidate) {
            continue;
        }
        order.push(candidate);
    }
    order
}

fn is_notice_language(language: Language) -> bool {
    matches!(
        language,
        Language::French | Language::English | Language::German | Language::Spanish
    )
}

/// Load a patron notice, returning the language the template was found in.
///
/// Each candidate is looked up in the database, then on disk, before the next
/// language is tried. Callers must render labels in the returned language.
pub(crate) async fn load_notice_template_async(
    pool: &Pool<Postgres>,
    templates_dir: &Path,
    template_id: &str,
    patron: Option<Language>,
    library: Option<Language>,
) -> AppResult<(Language, EmailTemplate)> {
    let order = notice_language_order(patron, library);
    for language in &order {
        let key = language.as_db_str();
        if let Some(template) = load_from_db(pool, template_id, key).await? {
            return Ok((*language, template));
        }
        if let Some(raw) = load_from_file(templates_dir, template_id, key)? {
            return Ok((*language, raw_to_email_template(&raw)));
        }
    }

    Err(missing_notice(template_id, &order))
}

/// On-disk notice resolution. Same order as [`load_notice_template_async`].
#[cfg(test)]
fn load_notice_template_from_files(
    templates_dir: &Path,
    template_id: &str,
    patron: Option<Language>,
    library: Option<Language>,
) -> AppResult<(Language, EmailTemplate)> {
    let order = notice_language_order(patron, library);
    for language in &order {
        if let Some(raw) = load_from_file(templates_dir, template_id, language.as_db_str())? {
            return Ok((*language, raw_to_email_template(&raw)));
        }
    }

    Err(missing_notice(template_id, &order))
}

fn missing_notice(template_id: &str, order: &[Language]) -> AppError {
    AppError::Internal(format!(
        "No template found for {template_id} (tried languages: {})",
        order
            .iter()
            .map(Language::as_db_str)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Build the language fallback chain for non-notice mail: requested → french → english.
fn language_chain(lang: Option<Language>) -> Vec<&'static str> {
    match lang {
        Some(l) => match l.as_db_str() {
            "french" => vec!["french", "english"],
            "english" => vec!["english", "french"],
            _ => vec!["french", "english"],
        },
        None => vec!["french", "english"],
    }
}

/// Load template for given id and language from the DB, with cascade `lang → french → english`.
/// Falls back to the on-disk JSON file (same cascade) if no DB row matches.
pub async fn load_template_async(
    pool: &Pool<Postgres>,
    templates_dir: &Path,
    template_id: &str,
    lang: Option<Language>,
) -> AppResult<EmailTemplate> {
    let candidates = language_chain(lang);

    for lang_key in &candidates {
        if let Some(tpl) = load_from_db(pool, template_id, lang_key).await? {
            return Ok(tpl);
        }
    }

    for lang_key in &candidates {
        if let Some(raw) = load_from_file(templates_dir, template_id, lang_key)? {
            return Ok(raw_to_email_template(&raw));
        }
    }

    Err(AppError::Internal(format!(
        "No template found for {} (tried languages: {})",
        template_id,
        candidates.join(", ")
    )))
}

/// Synchronous on-disk loader kept for callers that do not (yet) own a DB pool.
/// Newer code should prefer [`load_template_async`].
pub fn load_template(
    templates_dir: &Path,
    template_id: &str,
    lang: Option<Language>,
) -> AppResult<EmailTemplate> {
    let candidates = language_chain(lang);

    for lang_key in &candidates {
        if let Some(raw) = load_from_file(templates_dir, template_id, lang_key)? {
            return Ok(raw_to_email_template(&raw));
        }
    }

    Err(AppError::Internal(format!(
        "No template found for {} (tried languages: {})",
        template_id,
        candidates.join(", ")
    )))
}

async fn load_from_db(
    pool: &Pool<Postgres>,
    template_id: &str,
    language: &str,
) -> AppResult<Option<EmailTemplate>> {
    let repo = Repository::new(pool.clone(), None);
    let row = repo.email_templates_get(template_id, language).await?;
    Ok(row.map(|r| EmailTemplate {
        subject: r.subject,
        body_plain: r.body_plain,
        body_html: r.body_html,
    }))
}

fn load_from_file(
    templates_dir: &Path,
    template_id: &str,
    language: &str,
) -> AppResult<Option<RawTemplate>> {
    let path: PathBuf = templates_dir.join(format!("{}.{}.json", template_id, language));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Internal(format!("Failed to read template {:?}: {}", path, e)))?;
    let raw: RawTemplate = serde_json::from_str(&content)
        .map_err(|e| AppError::Internal(format!("Invalid template {:?}: {}", path, e)))?;
    Ok(Some(raw))
}

fn raw_to_email_template(raw: &RawTemplate) -> EmailTemplate {
    EmailTemplate {
        subject: raw.subject.clone(),
        body_plain: raw.body_plain.clone(),
        body_html: raw.body_html.clone(),
    }
}

/// Load one on-disk template language, with no fallback chain.
pub fn load_template_file(
    templates_dir: &Path,
    template_id: &str,
    language: &str,
) -> AppResult<Option<EmailTemplate>> {
    match load_from_file(templates_dir, template_id, language)? {
        Some(raw) => Ok(Some(raw_to_email_template(&raw))),
        None => Ok(None),
    }
}

/// Substitute {{var}} placeholders in subject and body.
/// Returns (subject, body_plain, body_html). HTML is generated from plain if not in template.
pub fn substitute(template: &EmailTemplate, vars: &[(&str, &str)]) -> (String, String, String) {
    let subj = substitute_str(&template.subject, vars);
    let plain = substitute_str(&template.body_plain, vars);
    let html = template
        .body_html
        .as_ref()
        .map(|h| substitute_str(h, vars))
        .unwrap_or_else(|| {
            format!(
                "<html><body><pre>{}</pre></body></html>",
                plain.replace('\n', "<br>")
            )
        });
    (subj, plain, html)
}

fn substitute_str(s: &str, vars: &[(&str, &str)]) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        let placeholder = format!("{{{{{}}}}}", k);
        out = out.replace(&placeholder, v);
    }
    out
}

/// Seed the `email_templates` table from the on-disk JSON files when it is empty.
///
/// This keeps `data/email_templates/*.json` as the authoritative initial source for new
/// installations while letting administrators edit the rows afterwards via the API.
/// Subsequent restarts are no-ops because the count check short-circuits.
pub async fn bootstrap_from_files(pool: &Pool<Postgres>, templates_dir: &Path) -> AppResult<usize> {
    let repo = Repository::new(pool.clone(), None);

    let mut inserted = 0usize;

    // insert templates that do not exist in the database
    for template_id in KNOWN_TEMPLATE_IDS {
        for language in SUPPORTED_LANGUAGES {
            // check if the template already exists in the database, if so, skip it
            if repo
                .email_templates_get(template_id, language)
                .await?
                .is_some()
            {
                continue;
            }

            match load_from_file(templates_dir, template_id, language)? {
                Some(raw) => {
                    repo.email_templates_upsert(
                        template_id,
                        language,
                        raw.name.trim(),
                        &raw.subject,
                        &raw.body_plain,
                        raw.body_html.as_deref(),
                    )
                    .await?;
                    inserted += 1;
                }
                None => {
                    tracing::debug!(
                        "Email template bootstrap: no file for {}.{}.json — skipped",
                        template_id,
                        language
                    );
                }
            }
        }
    }

    if inserted == 0 {
        tracing::warn!(
            "Email template bootstrap: no JSON files found under {:?}; the table stays empty",
            templates_dir
        );
    } else {
        tracing::info!(
            "Email template bootstrap: seeded {} rows from {:?}",
            inserted,
            templates_dir
        );
    }

    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_order_prefers_the_reader_then_the_library_then_french() {
        assert_eq!(
            notice_language_order(Some(Language::English), Some(Language::German)),
            vec![Language::English, Language::German, Language::French]
        );
        assert_eq!(
            notice_language_order(None, Some(Language::German)),
            vec![Language::German, Language::French, Language::English]
        );
        assert_eq!(
            notice_language_order(None, None),
            vec![Language::French, Language::English]
        );
        assert_eq!(
            notice_language_order(Some(Language::French), Some(Language::French)),
            vec![Language::French, Language::English]
        );
        // Italian has no notice template, so the library language is next.
        assert_eq!(
            notice_language_order(Some(Language::Italian), Some(Language::English)),
            vec![Language::English, Language::French]
        );
    }

    #[test]
    fn missing_patron_template_uses_the_library_language_before_french() {
        let dir = std::env::temp_dir().join(format!("elidune-notices-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        write_notice(&dir, "hold_ready", "english", "English hold");
        write_notice(&dir, "hold_ready", "french", "French hold");

        let (language, template) = load_notice_template_from_files(
            &dir,
            "hold_ready",
            Some(Language::German),
            Some(Language::English),
        )
        .expect("english library template");
        assert_eq!(language, Language::English);
        assert_eq!(template.subject, "English hold");

        let (language, template) =
            load_notice_template_from_files(&dir, "hold_ready", Some(Language::German), None)
                .expect("french fallback");
        assert_eq!(language, Language::French);
        assert_eq!(template.subject, "French hold");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shipped_notice_templates_share_placeholders() {
        let dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data/email_templates"));
        for template_id in [
            "hold_ready",
            "password_reset",
            "overdue_reminder",
            "overdue_second_reminder",
            "overdue_formal_notice",
        ] {
            let english = read_raw(dir, template_id, "english");
            for language in ["french", "german", "spanish"] {
                let other = read_raw(dir, template_id, language);
                assert_eq!(
                    placeholders(&english.body_plain),
                    placeholders(&other.body_plain),
                    "{template_id}.{language} plain placeholders"
                );
                assert_eq!(
                    placeholders(english.body_html.as_deref().unwrap_or("")),
                    placeholders(other.body_html.as_deref().unwrap_or("")),
                    "{template_id}.{language} html placeholders"
                );
                assert!(!other.subject.trim().is_empty());
            }
        }
    }

    fn write_notice(dir: &Path, template_id: &str, language: &str, subject: &str) {
        let body = format!(
            r#"{{"name":"{subject}","subject":"{subject}","body_plain":"Hello","body_html":null}}"#
        );
        std::fs::write(dir.join(format!("{template_id}.{language}.json")), body)
            .expect("write notice");
    }

    fn read_raw(dir: &Path, template_id: &str, language: &str) -> RawTemplate {
        let path = dir.join(format!("{template_id}.{language}.json"));
        let content = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("missing template {}", path.display());
        });
        serde_json::from_str(&content).unwrap_or_else(|err| {
            panic!("invalid template {}: {err}", path.display());
        })
    }

    fn placeholders(text: &str) -> Vec<&str> {
        let mut names = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                break;
            };
            names.push(&after[..end]);
            rest = &after[end + 2..];
        }
        names.sort_unstable();
        names.dedup();
        names
    }
}
