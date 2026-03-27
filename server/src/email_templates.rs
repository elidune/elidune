//! Email template loading and variable substitution

use std::path::Path;

use crate::{
    error::{AppError, AppResult},
    models::Language,
};

#[derive(Debug, Clone)]
pub struct EmailTemplate {
    pub subject: String,
    pub body_plain: String,
    pub body_html: Option<String>,
}

/// Load template for given id and language, with fallback: lang -> french -> english
pub fn load_template(
    templates_dir: &Path,
    template_id: &str,
    lang: Option<Language>,
) -> AppResult<EmailTemplate> {
    let candidates: Vec<&str> = match lang {
        Some(l) => {
            let s = l.as_db_str();
            if s == "french" || s == "english" {
                vec![s]
            } else {
                vec![s, "french", "english"]
            }
        }
        None => vec!["french", "english"],
    };

    for lang_key in &candidates {
        let path = templates_dir.join(format!("{}.{}.json", template_id, lang_key));
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| AppError::Internal(format!("Failed to read template {:?}: {}", path, e)))?;
            let raw: RawTemplate = serde_json::from_str(&content)
                .map_err(|e| AppError::Internal(format!("Invalid template {:?}: {}", path, e)))?;
            return Ok(EmailTemplate {
                subject: raw.subject,
                body_plain: raw.body_plain,
                body_html: raw.body_html,
            });
        }
    }

    Err(AppError::Internal(format!(
        "No template found for {} (tried: {})",
        template_id,
        candidates.join(", ")
    )))
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
        .unwrap_or_else(|| format!("<html><body><pre>{}</pre></body></html>", plain.replace('\n', "<br>")));
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

#[derive(serde::Deserialize)]
struct RawTemplate {
    subject: String,
    body_plain: String,
    #[serde(default)]
    body_html: Option<String>,
}
