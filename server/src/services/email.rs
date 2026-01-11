//! Email service for sending 2FA codes and notifications

use lettre::{
    message::{header::ContentType, Mailbox, Message, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    SmtpTransport, Transport,
};
use std::str::FromStr;

use crate::{
    config::EmailConfig,
    error::{AppError, AppResult},
};

#[derive(Clone)]
pub struct EmailService {
    config: EmailConfig,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Self {
        Self { config }
    }

    /// Send a 2FA code via email
    pub async fn send_2fa_code(&self, to: &str, code: &str) -> AppResult<()> {
        let subject = "Your Elidune 2FA Code";
        let body = format!(
            r#"
Your two-factor authentication code is: {code}

This code will expire in 10 minutes.

If you didn't request this code, please ignore this email.
"#,
            code = code
        );

        self.send_email(to, subject, &body).await
    }

    /// Send a recovery code via email
    pub async fn send_recovery_code(&self, to: &str, code: &str) -> AppResult<()> {
        let subject = "Your Elidune Account Recovery Code";
        let body = format!(
            r#"
Your account recovery code is: {code}

Use this code to recover access to your account if you've lost access to your 2FA device.

This code can only be used once. If you didn't request this code, please contact support immediately.
"#,
            code = code
        );

        self.send_email(to, subject, &body).await
    }

    /// Generic email sending function
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> AppResult<()> {
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
                            .body(body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(format!(
                                r#"<html><body><pre>{}</pre></body></html>"#,
                                body.replace("\n", "<br>")
                            )),
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
