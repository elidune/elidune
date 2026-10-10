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
    events_consent,
    models::{
        event::{
            AudienceSelection, CreateEvent, Event, EventAttachmentInput, EventQuery, UpdateEvent,
        },
        Language,
    },
    repository::{events::EventAnnualStats, users::AnnouncementChild, EventsServiceRepository},
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
    /// HMAC key for announcement unsubscribe tokens (the JWT secret).
    unsubscribe_secret: String,
    /// Absolute origin used in unsubscribe links, without a trailing slash.
    public_base_url: String,
}

impl EventsService {
    pub fn new(
        repository: Arc<dyn EventsServiceRepository>,
        email: EmailService,
        audit: AuditService,
        unsubscribe_secret: String,
        public_base_url: String,
    ) -> Self {
        Self {
            repository,
            email,
            audit,
            unsubscribe_secret,
            public_base_url,
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

    /// How many distinct recipients `send_announcement` would queue for this event.
    ///
    /// Uses the same recipient query as sending: audience filter, guardian routing,
    /// and `events_consent_at IS NOT NULL` on the adult or on the guardian.
    #[tracing::instrument(skip(self), err)]
    pub async fn count_announcement_recipients(&self, event_id: i64) -> AppResult<i64> {
        let event = self.repository.events_get_by_id(event_id).await?;
        let targets = self
            .repository
            .users_list_announcement_recipients(event.all_audiences, &event.public_types)
            .await?;
        i64::try_from(targets.len())
            .map_err(|_| AppError::Internal("announcement recipient count exceeds i64".into()))
    }

    /// Send an announcement email to patrons in any selected audience, once each.
    ///
    /// Recipients are selected in SQL. A non-`child` patron is emailed directly when
    /// `events_consent_at` is set and either `all_audiences` is set or their public
    /// type is one of the event audiences. A `child` is never emailed: each active
    /// child is routed to their major guardian, and that guardian's own
    /// `events_consent_at` is the opt-in. One email is sent per recipient. When the
    /// recipient stands in for one or more children, the message names them.
    ///
    /// Every message includes an unsubscribe link. The first message queued for a
    /// patron whose consent source is `migration` and whose notice timestamp is still
    /// null also explains why they were opted in. `receive_reminders` is not consulted.
    ///
    /// If the request provides `subject`/`body_plain`/`body_html`, those are used
    /// directly instead of the template. A guardian message still names the children
    /// when that text does not already include them.
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

        for recipient in &targets {
            let email_addr = match &recipient.email {
                Some(e) if !e.is_empty() => e.clone(),
                _ => {
                    skipped += 1;
                    continue;
                }
            };

            let firstname = recipient.firstname.as_deref().unwrap_or("");
            let (children_line, children_block) =
                announcement_children_fragments(recipient.language.as_deref(), &recipient.children);
            let child_ids: Vec<i64> = recipient.children.iter().map(|child| child.id).collect();

            let (subject, mut body_plain, mut body_html) =
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
                    let lang = recipient.language.as_deref().map(Language::from);
                    match self.email.load_template("event_announcement", lang).await {
                        Err(e) => {
                            errors.push(AnnouncementError {
                                user_id: recipient.id,
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
                                ("children_line", &children_line),
                                ("children_block", &children_block),
                            ];
                            let (s, p, h) = email_templates::substitute(&template, &vars);
                            (s, p, h)
                        }
                    }
                };
            append_children_if_unnamed(
                &mut body_plain,
                &mut body_html,
                &children_line,
                &children_block,
                &recipient.children,
            );
            let (ui_unsubscribe_url, api_unsubscribe_url) =
                match self.unsubscribe_urls(recipient.id) {
                    Ok(urls) => urls,
                    Err(e) => {
                        errors.push(AnnouncementError {
                            user_id: recipient.id,
                            email: email_addr.clone(),
                            error_message: e.to_string(),
                        });
                        continue;
                    }
                };
            events_consent::append_announcement_footer(
                &mut body_plain,
                &mut body_html,
                recipient.language.as_deref(),
                recipient.include_migration_notice,
                &ui_unsubscribe_url,
            );
            let header_values = events_consent::list_unsubscribe_headers(&api_unsubscribe_url);
            let headers: Vec<(&str, &str)> = header_values
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            let notice_user = recipient.include_migration_notice.then_some(recipient.id);

            match self
                .email
                .enqueue_event_announcement(
                    &email_addr,
                    &subject,
                    &body_plain,
                    &body_html,
                    event_id,
                    crate::email::EventAnnouncementExtras {
                        headers: &headers,
                        migration_notice_user_id: notice_user,
                    },
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
                            "user_id": recipient.id,
                            "email": email_addr,
                            "event_name": event.name,
                            "outbox_id": outbox_id,
                            "child_ids": child_ids,
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
                            "user_id": recipient.id,
                            "email": email_addr.clone(),
                            "event_name": event.name,
                            "child_ids": child_ids,
                        })),
                        audit::AuditLogMeta::from_app_error(&e),
                    );
                    errors.push(AnnouncementError {
                        user_id: recipient.id,
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

    /// UI page the patron opens, and the API URL used by `List-Unsubscribe`.
    ///
    /// Both sit on [`Self::public_base_url`]: nginx (and the Vite dev proxy) serve
    /// the SPA and `/api/v1` from the same origin.
    fn unsubscribe_urls(&self, user_id: i64) -> AppResult<(String, String)> {
        let token = events_consent::sign(self.unsubscribe_secret.as_bytes(), user_id)?;
        let base = &self.public_base_url;
        Ok((
            format!("{base}/events/unsubscribe?token={token}"),
            format!("{base}/api/v1/events/unsubscribe?token={token}"),
        ))
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

/// Plain and HTML blocks naming the children a guardian email concerns.
/// Empty for a direct send. The intro follows [`Language`]: English, German,
/// and Spanish each have a sentence; every other value (missing, unknown, or
/// a language without a packaged template) uses French, matching the
/// `event_announcement` cascade in [`crate::email_templates`].
pub(crate) fn announcement_children_fragments(
    language: Option<&str>,
    children: &[AnnouncementChild],
) -> (String, String) {
    if children.is_empty() {
        return (String::new(), String::new());
    }
    let names = children
        .iter()
        .map(child_display_name)
        .collect::<Vec<_>>()
        .join(", ");
    let intro = children_intro(language);
    (
        format!("\n{intro} {names}.\n"),
        format!("<p>{intro} {}.</p>", html_escape(&names)),
    )
}

/// Localized intro for [`announcement_children_fragments`].
/// Resolved with [`Language::from`], so aliases (`deu`, `spa`, …) match the
/// rest of the server. French is the fallback.
fn children_intro(language: Option<&str>) -> &'static str {
    match language.map(Language::from).unwrap_or(Language::French) {
        Language::English => "This invitation concerns:",
        Language::German => "Diese Einladung betrifft:",
        Language::Spanish => "Esta invitación se refiere a:",
        _ => "Cette invitation concerne :",
    }
}

fn child_display_name(child: &AnnouncementChild) -> String {
    let first = child.firstname.as_deref().unwrap_or("").trim();
    let last = child.lastname.as_deref().unwrap_or("").trim();
    match (first.is_empty(), last.is_empty()) {
        (false, false) => format!("{first} {last}"),
        (false, true) => first.to_string(),
        (true, false) => last.to_string(),
        (true, true) => format!("#{}", child.id),
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Append the localized children block when a stored or custom body does not
/// already name every child (older templates omit `{{children_line}}`).
fn append_children_if_unnamed(
    plain: &mut String,
    html: &mut String,
    line: &str,
    block: &str,
    children: &[AnnouncementChild],
) {
    let unnamed = children.iter().any(|child| {
        let name = child_display_name(child);
        !plain.contains(&name)
    });
    if unnamed {
        plain.push_str(line);
        html.push_str(block);
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

#[cfg(test)]
mod children_fragment_tests {
    use super::{announcement_children_fragments, append_children_if_unnamed, AnnouncementChild};

    fn child(id: i64, first: &str, last: &str) -> AnnouncementChild {
        AnnouncementChild {
            id,
            firstname: Some(first.into()),
            lastname: Some(last.into()),
        }
    }

    #[test]
    fn direct_send_has_an_empty_block() {
        let (plain, html) = announcement_children_fragments(Some("french"), &[]);
        assert_eq!(plain, "");
        assert_eq!(html, "");
    }

    #[test]
    fn french_block_names_every_child() {
        let (plain, html) = announcement_children_fragments(
            Some("french"),
            &[child(2, "Lea", "Martin"), child(3, "Noe", "Martin")],
        );
        assert!(plain.contains("Cette invitation concerne : Lea Martin, Noe Martin."));
        assert!(html.contains("Cette invitation concerne : Lea Martin, Noe Martin."));
        assert!(!plain.contains("This invitation concerns"));
    }

    #[test]
    fn english_block_names_every_child_and_escapes_html() {
        let (plain, html) =
            announcement_children_fragments(Some("english"), &[child(4, "Ada", "Lovelace & Co")]);
        assert!(plain.contains("This invitation concerns: Ada Lovelace & Co."));
        assert!(html.contains("This invitation concerns: Ada Lovelace &amp; Co."));
        assert!(!html.contains("Lovelace & Co"));
    }

    #[test]
    fn german_block_names_every_child() {
        for lang in ["german", "deu", "ger", "langDe", "3"] {
            let (plain, html) =
                announcement_children_fragments(Some(lang), &[child(5, "Lea", "Martin")]);
            assert!(
                plain.contains("Diese Einladung betrifft: Lea Martin."),
                "{lang}: {plain}"
            );
            assert!(html.contains("<p>Diese Einladung betrifft: Lea Martin.</p>"));
            assert!(!plain.contains("Cette invitation concerne"));
            assert!(!plain.contains("This invitation concerns"));
        }
    }

    #[test]
    fn spanish_block_names_every_child() {
        for lang in ["spanish", "spa", "langEs", "5"] {
            let (plain, html) =
                announcement_children_fragments(Some(lang), &[child(6, "Lea", "Martin")]);
            assert!(
                plain.contains("Esta invitación se refiere a: Lea Martin."),
                "{lang}: {plain}"
            );
            assert!(html.contains("<p>Esta invitación se refiere a: Lea Martin.</p>"));
            assert!(!plain.contains("Cette invitation concerne"));
            assert!(!plain.contains("This invitation concerns"));
        }
    }

    #[test]
    fn unknown_language_falls_back_to_french() {
        for lang in [
            None,
            Some("unknown"),
            Some("italian"),
            Some("not-a-language"),
        ] {
            let (plain, html) = announcement_children_fragments(lang, &[child(7, "Lea", "Martin")]);
            assert!(
                plain.contains("Cette invitation concerne : Lea Martin."),
                "{lang:?}: {plain}"
            );
            assert!(html.contains("<p>Cette invitation concerne : Lea Martin.</p>"));
            assert!(!plain.contains("Diese Einladung betrifft"));
            assert!(!plain.contains("Esta invitación"));
        }
    }

    #[test]
    fn unnamed_children_are_appended_once() {
        let children = vec![child(2, "Lea", "Martin")];
        let (line, block) = announcement_children_fragments(Some("english"), &children);
        let mut plain = "Hello.".to_string();
        let mut html = "<p>Hello.</p>".to_string();
        append_children_if_unnamed(&mut plain, &mut html, &line, &block, &children);
        append_children_if_unnamed(&mut plain, &mut html, &line, &block, &children);
        assert_eq!(plain.matches("Lea Martin").count(), 1);
        assert!(html.contains("Lea Martin"));
    }
}
