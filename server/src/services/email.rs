//! Email service for sending 2FA codes and notifications

use lettre::{
    message::{header::ContentType, Mailbox, Message, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    SmtpTransport, Transport,
};
use std::path::Path;
use std::str::FromStr;

use crate::{
    config::EmailConfig,
    error::{AppError, AppResult},
    models::Language,
    services::email_templates,
};

#[derive(Clone)]
pub struct EmailService {
    config: EmailConfig,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    fn templates_dir(&self) -> &Path {
        Path::new(&self.config.templates_dir)
    }

    /// Send a 2FA code via email
    pub async fn send_2fa_code(
        &self,
        to: &str,
        code: &str,
        lang: Option<Language>,
    ) -> AppResult<()> {
        let template = email_templates::load_template(
            self.templates_dir(),
            "2fa_code",
            lang,
        )?;
        let (subject, body_plain, body_html) =
            email_templates::substitute(&template, &[("code", code)]);
        self.send_email_with_html(to, &subject, &body_plain, &body_html).await
    }

    /// Send a recovery code via email
    pub async fn send_recovery_code(
        &self,
        to: &str,
        code: &str,
        lang: Option<Language>,
    ) -> AppResult<()> {
        let template = email_templates::load_template(
            self.templates_dir(),
            "recovery_code",
            lang,
        )?;
        let (subject, body_plain, body_html) =
            email_templates::substitute(&template, &[("code", code)]);
        self.send_email_with_html(to, &subject, &body_plain, &body_html).await
    }

    /// Send password reset email with a reset token.
    /// `reset_url` is the full URL with token substituted (for the reset link in the email).
    pub async fn send_password_reset(
        &self,
        to: &str,
        token: &str,
        lang: Option<Language>,
        reset_url: Option<&str>,
    ) -> AppResult<()> {
        let template = email_templates::load_template(
            self.templates_dir(),
            "password_reset",
            lang,
        )?;
        let vars: Vec<(&str, &str)> = match reset_url {
            Some(url) => vec![("token", token), ("reset_url", url)],
            None => vec![("token", token), ("reset_url", "")],
        };
        let (subject, body_plain, body_html) =
            email_templates::substitute(&template, &vars);
        self.send_email_with_html(to, &subject, &body_plain, &body_html).await
    }

    async fn send_email_with_html(
        &self,
        to: &str,
        subject: &str,
        body_plain: &str,
        body_html: &str,
    ) -> AppResult<()> {
        let from_name = self
            .config
            .smtp_from_name
            .as_deref()
            .unwrap_or("Elidune");
        let from_mailbox = Mailbox::from_str(&format!("{} <{}>", from_name, self.config.smtp_from))
            .map_err(|e| AppError::Internal(format!("Invalid from address: {}", e)))?;

        let to_mailbox = Mailbox::from_str(to)
            .map_err(|e| AppError::Internal(format!("Invalid to address: {}", e)))?;

        let email = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(body_plain.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(body_html.to_string()),
                    ),
            )
            .map_err(|e| AppError::Internal(format!("Failed to build email: {}", e)))?;

        let mailer_builder = if self.config.smtp_use_tls {
            // Use STARTTLS for secure connection
            SmtpTransport::starttls_relay(&self.config.smtp_host)
                .map_err(|e| AppError::Internal(format!("Failed to create SMTP transport: {}", e)))?
        } else {
            SmtpTransport::builder_dangerous(&self.config.smtp_host)
        }
        .port(self.config.smtp_port);

        let mailer_builder = if let (Some(username), Some(password)) = (
            &self.config.smtp_username,
            &self.config.smtp_password,
        ) {
            mailer_builder.credentials(Credentials::new(
                username.clone(),
                password.clone(),
            ))
        } else {
            mailer_builder
        };

        let mailer = mailer_builder.build();

        mailer
            .send(&email)
            .map_err(|e| AppError::Internal(format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}
