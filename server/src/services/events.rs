//! Events service

use std::path::Path;
use std::sync::Arc;

/// Maximum size for an event attachment (10 MiB).
pub const MAX_EVENT_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

use crate::{
    error::{AppError, AppResult},
    models::{
        event::{
            AudienceSelection, CreateEvent, Event, EventAttachmentInput, EventQuery, UpdateEvent,
        },
        Language,
    },
    repository::{events::EventAnnualStats, EventsServiceRepository},
    services::{
        audit::{self, AuditService},
        email::EmailService,
        email_templates,
    },
};

/// Request body for sending an event announcement email.
/// All fields are optional: if omitted, the default template is used.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendAnnouncementRequest {
    /// Override email subject (uses template if absent)
    pub subject: Option<String>,
    /// Override plain-text body (uses template if absent)
    pub body_plain: Option<String>,
    /// Override HTML body (derived from body_plain or template if absent)
    pub body_html: Option<String>,
}

/// Per-recipient error collected during a bulk send
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementError {
    pub user_id: i64,
    pub email: String,
    pub error_message: String,
}

/// Summary returned after sending an event announcement
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementReport {
    pub event_id: i64,
    /// Number of emails successfully sent
    pub emails_sent: u32,
    /// Number of recipients skipped (no email address)
    pub skipped: u32,
    /// Per-recipient send errors
    pub errors: Vec<AnnouncementError>,
}

#[derive(Clone)]
pub struct EventsService {
    repository: Arc<dyn EventsServiceRepository>,
    email: EmailService,
    audit: AuditService,
}

impl EventsService {
    pub fn new(
        repository: Arc<dyn EventsServiceRepository>,
        email: EmailService,
        audit: AuditService,
    ) -> Self {
        Self {
            repository,
            email,
            audit,
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn list(&self, query: &EventQuery) -> AppResult<(Vec<Event>, i64)> {
        self.repository.events_list(query).await
    }

    /// Load event metadata only (no `attachment_data_base64`). Used internally, e.g. announcement emails.
    pub async fn get_by_id(&self, id: i64) -> AppResult<Event> {
        self.repository.events_get_by_id(id).await
    }

    /// Load event including `attachment_data_base64` when an attachment exists (single-resource API).
    #[tracing::instrument(skip(self), err)]
    pub async fn get_by_id_with_attachment(&self, id: i64) -> AppResult<Event> {
        let event = self.repository.events_get_by_id(id).await?;
        self.enrich_with_attachment_base64(event).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn create(&self, data: &CreateEvent) -> AppResult<Event> {
        let audiences = resolve_create_audiences(data).map_err(AudienceInputError::into_app)?;
        self.validate_audience_names(&audiences.public_types)
            .await?;
        let attachment = match &data.attachment {
            Some(a) => Some(decode_event_attachment_input(a)?),
            None => None,
        };
        let event = self
            .repository
            .events_create(data, &audiences, attachment)
            .await?;
        self.enrich_with_attachment_base64(event).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn update(&self, id: i64, data: &UpdateEvent) -> AppResult<Event> {
        let audiences = resolve_update_audiences(data).map_err(AudienceInputError::into_app)?;
        if let Some(selection) = &audiences {
            self.validate_audience_names(&selection.public_types)
                .await?;
        }
        let remove = data.remove_attachment == Some(true);
        let new_attachment = if !remove {
            match &data.attachment {
                Some(a) => Some(decode_event_attachment_input(a)?),
                None => None,
            }
        } else {
            None
        };

        let mut event = self.repository.events_update(id, data, audiences).await?;

        event = if remove {
            self.repository.events_delete_attachment(id).await?
        } else if let Some((bytes, fname, mime)) = new_attachment {
            self.repository
                .events_put_attachment(id, &bytes, &fname, &mime)
                .await?
        } else {
            event
        };

        self.enrich_with_attachment_base64(event).await
    }

    async fn enrich_with_attachment_base64(&self, mut event: Event) -> AppResult<Event> {
        if event.attachment_size.unwrap_or(0) > 0 {
            if let Some((bytes, _, _)) =
                self.repository.events_get_attachment_blob(event.id).await?
            {
                event.attachment_data_base64 = Some(B64.encode(&bytes));
            }
        }
        Ok(event)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn delete(&self, id: i64) -> AppResult<()> {
        self.repository.events_delete(id).await
    }

    /// Get annual event statistics (for annual report)
    #[tracing::instrument(skip(self), err)]
    pub async fn annual_stats(&self, year: i32) -> AppResult<EventAnnualStats> {
        self.repository.events_annual_stats(year).await
    }

    /// Send an announcement email to patrons in any selected audience, once each.
    ///
    /// Recipients are selected in SQL: `receive_reminders` is required, and either
    /// `all_audiences` is set or `public_types.name` is one of the event audiences.
    /// Each user is returned once.
    ///
    /// `main` has no dedicated GDPR communications-consent column. The only email
    /// opt-in is `users.receive_reminders` (overdue reminders, default true). Patrons
    /// with that flag false are not notified. No new consent model is introduced.
    ///
    /// If the request provides `subject`/`body_plain`/`body_html`, those are used
    /// directly instead of the template.
    #[tracing::instrument(skip(self), err)]
    pub async fn send_announcement(
        &self,
        event_id: i64,
        payload: &SendAnnouncementRequest,
        triggered_by: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<AnnouncementReport> {
        let event = self.repository.events_get_by_id(event_id).await?;

        let event_date = event.event_date.format("%d/%m/%Y").to_string();
        let event_type_label = match event.event_type {
            0 => "Animation",
            1 => "Visite scolaire / School visit",
            2 => "Exposition / Exhibition",
            3 => "Conférence / Conference",
            4 => "Atelier / Workshop",
            5 => "Spectacle / Show",
            _ => "Autre / Other",
        };

        let start_time_plain = event
            .start_time
            .map(|t| format!("\nHeure / Time: {}", t.format("%H:%M")))
            .unwrap_or_default();
        let start_time_row = event
            .start_time
            .map(|t| {
                format!(
                    "<tr><td style=\"padding:6px 12px;font-weight:bold;background:#f7f7f7;border:1px solid #e2e8f0\">Heure / Time</td>\
                 <td style=\"padding:6px 12px;border:1px solid #e2e8f0\">{}</td></tr>",
                    t.format("%H:%M")
                )
            })
            .unwrap_or_default();

        let description_plain = event
            .description
            .as_deref()
            .map(|d| format!("\n{}", d))
            .unwrap_or_default();
        let description_block = event
            .description
            .as_deref()
            .map(|d| format!("<p>{}</p>", d.replace('\n', "<br>")))
            .unwrap_or_default();

        let targets = self
            .repository
            .users_list_announcement_recipients(event.all_audiences, &event.public_types)
            .await?;

        let mut emails_sent: u32 = 0;
        let mut skipped: u32 = 0;
        let mut errors: Vec<AnnouncementError> = Vec::new();

        for user in &targets {
            let email_addr = match &user.email {
                Some(e) if !e.is_empty() => e.clone(),
                _ => {
                    skipped += 1;
                    continue;
                }
            };

            let firstname = user.firstname.as_deref().unwrap_or("");

            let (subject, body_plain, body_html) =
                if payload.subject.is_some() || payload.body_plain.is_some() {
                    // Use caller-supplied content
                    let subj = payload
                        .subject
                        .as_deref()
                        .unwrap_or(&event.name)
                        .to_string();
                    let plain = payload.body_plain.as_deref().unwrap_or("").to_string();
                    let html = payload
                        .body_html
                        .as_deref()
                        .map(|h| h.to_string())
                        .unwrap_or_else(|| {
                            format!(
                                "<html><body><pre>{}</pre></body></html>",
                                plain.replace('\n', "<br>")
                            )
                        });
                    (subj, plain, html)
                } else {
                    let lang = user.language.as_deref().map(Language::from);
                    match self.email.load_template("event_announcement", lang).await {
                        Err(e) => {
                            errors.push(AnnouncementError {
                                user_id: user.id,
                                email: email_addr.clone(),
                                error_message: format!("Template load error: {}", e),
                            });
                            continue;
                        }
                        Ok(template) => {
                            let vars: Vec<(&str, &str)> = vec![
                                ("firstname", firstname),
                                ("event_name", &event.name),
                                ("event_date", &event_date),
                                ("event_type", event_type_label),
                                ("start_time_line", &start_time_plain),
                                ("start_time_row", &start_time_row),
                                ("description_line", &description_plain),
                                ("description_block", &description_block),
                            ];
                            let (s, p, h) = email_templates::substitute(&template, &vars);
                            (s, p, h)
                        }
                    }
                };

            match self
                .email
                .enqueue_event_announcement(
                    &email_addr,
                    &subject,
                    &body_plain,
                    &body_html,
                    event_id,
                )
                .await
            {
                Ok(outbox_id) => {
                    emails_sent += 1;
                    self.audit.log(
                        audit::event::EVENT_ANNOUNCEMENT_SENT,
                        triggered_by,
                        Some("event"),
                        Some(event_id),
                        client_ip.clone(),
                        Some(serde_json::json!({
                            "user_id": user.id,
                            "email": email_addr,
                            "event_name": event.name,
                            "outbox_id": outbox_id,
                        })),
                        audit::AuditLogMeta::success(),
                    );
                }
                Err(e) => {
                    self.audit.log(
                        audit::event::EVENT_ANNOUNCEMENT_SENT,
                        triggered_by,
                        Some("event"),
                        Some(event_id),
                        client_ip.clone(),
                        Some(serde_json::json!({
                            "user_id": user.id,
                            "email": email_addr.clone(),
                            "event_name": event.name,
                        })),
                        audit::AuditLogMeta::from_app_error(&e),
                    );
                    errors.push(AnnouncementError {
                        user_id: user.id,
                        email: email_addr,
                        error_message: e.to_string(),
                    });
                }
            }
        }

        Ok(AnnouncementReport {
            event_id,
            emails_sent,
            skipped,
            errors,
        })
    }

    async fn validate_audience_names(&self, names: &[String]) -> AppResult<()> {
        for name in names {
            let exists = self.repository.public_types_find_id_by_name(name).await?;
            if exists.is_none() {
                return Err(AppError::Validation(format!(
                    "Unknown public_type name {name:?} (must match public_types.name)"
                )));
            }
        }
        Ok(())
    }
}

/// Why an audience payload was rejected. Mapped to [`AppError::Validation`] at the service boundary.
#[derive(Debug, PartialEq, Eq)]
pub enum AudienceInputError {
    Empty,
    Combined,
}

impl AudienceInputError {
    fn into_app(self) -> AppError {
        match self {
            Self::Empty => AppError::Validation(
                "Select at least one audience, or set allAudiences to true".into(),
            ),
            Self::Combined => {
                AppError::Validation("allAudiences cannot be combined with publicTypes".into())
            }
        }
    }
}

/// Build the audience selection for a create request.
pub fn resolve_create_audiences(
    data: &CreateEvent,
) -> Result<AudienceSelection, AudienceInputError> {
    let names = match &data.public_types {
        Some(list) => normalize_audience_names(list),
        None => legacy_public_type_names(data.public_type.as_deref()),
    };
    validate_audience_selection(data.all_audiences, names)
}

/// Audience replacement for an update. `None` means the stored audiences stay as they are.
pub fn resolve_update_audiences(
    data: &UpdateEvent,
) -> Result<Option<AudienceSelection>, AudienceInputError> {
    let legacy = data
        .public_type
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let touched = data.all_audiences.is_some() || data.public_types.is_some() || legacy.is_some();
    if !touched {
        return Ok(None);
    }
    let all_audiences = data.all_audiences.unwrap_or(false);
    let names = if let Some(list) = &data.public_types {
        normalize_audience_names(list)
    } else if let Some(name) = legacy {
        vec![name.to_string()]
    } else {
        Vec::new()
    };
    Ok(Some(validate_audience_selection(all_audiences, names)?))
}

fn normalize_audience_names(names: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in names {
        let name = raw.trim();
        if name.is_empty() || out.iter().any(|existing: &String| existing == name) {
            continue;
        }
        out.push(name.to_string());
    }
    out
}

fn legacy_public_type_names(public_type: Option<&str>) -> Vec<String> {
    match public_type.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => vec![name.to_string()],
        None => Vec::new(),
    }
}

fn validate_audience_selection(
    all_audiences: bool,
    public_types: Vec<String>,
) -> Result<AudienceSelection, AudienceInputError> {
    if all_audiences && !public_types.is_empty() {
        return Err(AudienceInputError::Combined);
    }
    if !all_audiences && public_types.is_empty() {
        return Err(AudienceInputError::Empty);
    }
    Ok(AudienceSelection {
        all_audiences,
        public_types: if all_audiences {
            Vec::new()
        } else {
            public_types
        },
    })
}

fn decode_event_attachment_input(
    input: &EventAttachmentInput,
) -> AppResult<(Vec<u8>, String, String)> {
    let bytes = B64
        .decode(input.data_base64.trim())
        .map_err(|_| AppError::Validation("Invalid Base64 in attachment".to_string()))?;
    if bytes.is_empty() {
        return Err(AppError::Validation(
            "Attachment payload is empty".to_string(),
        ));
    }
    if bytes.len() > MAX_EVENT_ATTACHMENT_BYTES {
        return Err(AppError::Validation(format!(
            "Attachment exceeds maximum size of {} bytes",
            MAX_EVENT_ATTACHMENT_BYTES
        )));
    }
    let fname = sanitize_attachment_filename(&input.file_name);
    let mime = normalize_mime_type(&input.mime_type);
    Ok((bytes, fname, mime))
}

fn sanitize_attachment_filename(name: &str) -> String {
    let base = Path::new(name.trim())
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("attachment");
    let cleaned: String = base
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .take(200)
        .collect();
    let trimmed = cleaned.trim_matches('.');
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_mime_type(mime: &str) -> String {
    let s = mime.trim();
    if s.is_empty() {
        return "application/octet-stream".to_string();
    }
    s.chars().take(255).collect()
}

#[cfg(test)]
mod audience_tests {
    use super::*;

    fn create_body(json: &str) -> CreateEvent {
        serde_json::from_str(json).expect("create event json")
    }

    fn update_body(json: &str) -> UpdateEvent {
        serde_json::from_str(json).expect("update event json")
    }

    #[test]
    fn rejects_empty_audience_selection() {
        let created = create_body(r#"{"name":"Talk","eventDate":"2026-05-01"}"#);
        let err = resolve_create_audiences(&created).unwrap_err();
        assert_eq!(err, AudienceInputError::Empty);

        let cleared = update_body(r#"{"allAudiences":false,"publicTypes":[]}"#);
        let err = resolve_update_audiences(&cleared).unwrap_err();
        assert_eq!(err, AudienceInputError::Empty);
    }

    #[test]
    fn explicit_all_audiences_stores_no_names() {
        let created =
            create_body(r#"{"name":"Talk","eventDate":"2026-05-01","allAudiences":true}"#);
        let selection = resolve_create_audiences(&created).unwrap();
        assert!(selection.all_audiences);
        assert!(selection.public_types.is_empty());

        let with_empty_list = create_body(
            r#"{"name":"Talk","eventDate":"2026-05-01","allAudiences":true,"publicTypes":[]}"#,
        );
        let selection = resolve_create_audiences(&with_empty_list).unwrap();
        assert!(selection.all_audiences);
    }

    #[test]
    fn rejects_all_audiences_combined_with_names() {
        let created = create_body(
            r#"{"name":"Talk","eventDate":"2026-05-01","allAudiences":true,"publicTypes":["adult"]}"#,
        );
        assert_eq!(
            resolve_create_audiences(&created),
            Err(AudienceInputError::Combined)
        );
    }

    #[test]
    fn legacy_public_type_maps_to_one_element_list() {
        let created =
            create_body(r#"{"name":"Talk","eventDate":"2026-05-01","publicType":" adult "}"#);
        let selection = resolve_create_audiences(&created).unwrap();
        assert!(!selection.all_audiences);
        assert_eq!(selection.public_types, vec!["adult".to_string()]);

        let updated = update_body(r#"{"publicType":"child"}"#);
        let selection = resolve_update_audiences(&updated).unwrap().unwrap();
        assert_eq!(selection.public_types, vec!["child".to_string()]);
    }

    #[test]
    fn multi_audience_create_dedupes_names() {
        let created = create_body(
            r#"{"name":"Talk","eventDate":"2026-05-01","publicTypes":["child"," adult ","child",""]}"#,
        );
        let selection = resolve_create_audiences(&created).unwrap();
        assert_eq!(
            selection.public_types,
            vec!["child".to_string(), "adult".to_string()]
        );
    }

    #[test]
    fn update_without_audience_fields_leaves_them_unchanged() {
        let updated = update_body(r#"{"name":"Renamed"}"#);
        assert!(resolve_update_audiences(&updated).unwrap().is_none());
    }

    #[test]
    fn migration_036_keeps_legacy_public_type_column() {
        let sql = include_str!("../../migrations/036_events_audiences.sql");
        let upper = sql.to_ascii_uppercase();
        assert!(
            !upper.contains("DROP COLUMN"),
            "legacy public_type stays until a later migration"
        );
        assert!(
            !upper.contains("DROP CONSTRAINT"),
            "legacy public_type FK stays until a later migration"
        );
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS event_audiences"));
        assert!(sql.contains("ADD COLUMN IF NOT EXISTS all_audiences"));
        assert!(sql.contains("WHERE public_type IS NULL"));
    }
}
