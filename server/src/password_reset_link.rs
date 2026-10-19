//! Server-built password-reset links.
//!
//! The client `resetUrl` is never consulted. Resolution order:
//! 1. `users.password_reset_url_template` when it is set (admin-controlled).
//! 2. Otherwise `{public_base_url}/reset-password?token=<token>` from `users` then `server`
//!    config (and matching `ELIDUNE_*` env vars when those fields are empty).
//! 3. Otherwise no link, and the caller must not send mail.
//!
//! `PUBLIC_BASE_URL` is the configured origin only. This module does not read
//! the `Host` or `X-Forwarded-Host` header.

use crate::config::{ServerConfig, UsersConfig};

/// UI route (`ui/src/App.tsx`) appended to `PUBLIC_BASE_URL`.
/// `<token>` stands for the URL-encoded reset token.
pub const PASSWORD_RESET_UI_PATH: &str = "/reset-password?token=";

/// How the reset link will be built, before the token is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordResetLink {
    /// Admin template that still contains the literal `<token>` placeholder.
    Template(String),
    /// Public origin with any trailing slash already removed.
    PublicBase(String),
}

/// Why a reset link cannot be built. The caller logs this and returns the
/// generic acceptance response without sending mail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordResetLinkUnavailable {
    /// `password_reset_url_template` is set but has no `<token>` placeholder.
    TemplateMissingPlaceholder,
    /// Neither the template nor `PUBLIC_BASE_URL` is configured.
    NotConfigured,
}

/// Choose the link source. An empty or whitespace-only value counts as unset.
pub fn plan_password_reset_link(
    template: Option<&str>,
    public_base_url: Option<&str>,
) -> Result<PasswordResetLink, PasswordResetLinkUnavailable> {
    if let Some(template) = nonempty(template) {
        if !template.contains("<token>") {
            return Err(PasswordResetLinkUnavailable::TemplateMissingPlaceholder);
        }
        return Ok(PasswordResetLink::Template(template.to_string()));
    }
    let Some(base) = nonempty(public_base_url) else {
        return Err(PasswordResetLinkUnavailable::NotConfigured);
    };
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return Err(PasswordResetLinkUnavailable::NotConfigured);
    }
    Ok(PasswordResetLink::PublicBase(base.to_string()))
}

/// Fill the token into a planned link.
///
/// The admin template is substituted literally. The default UI path
/// percent-encodes the token so the result is exactly
/// `{origin}/reset-password?token=<token>`.
#[must_use]
pub fn render_password_reset_url(link: &PasswordResetLink, token: &str) -> String {
    match link {
        PasswordResetLink::Template(template) => template.replace("<token>", token),
        PasswordResetLink::PublicBase(base) => {
            format!(
                "{base}{PASSWORD_RESET_UI_PATH}{}",
                encode_query_component(token)
            )
        }
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn trimmed_config(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trimmed_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Public UI origin for the default password-reset link. Never uses the bind
/// address or a request `Host` header.
#[must_use]
pub fn password_reset_public_base(users: &UsersConfig, server: &ServerConfig) -> Option<String> {
    trimmed_config(users.public_base_url.as_deref())
        .or_else(|| trimmed_env("ELIDUNE_USERS__PUBLIC_BASE_URL"))
        .or_else(|| trimmed_config(server.public_base_url.as_deref()))
        .or_else(|| trimmed_env("ELIDUNE_SERVER__PUBLIC_BASE_URL"))
}

/// Log why reset mail was skipped. The client URL is not included.
pub fn log_unavailable(reason: PasswordResetLinkUnavailable) {
    match reason {
        PasswordResetLinkUnavailable::TemplateMissingPlaceholder => {
            tracing::error!(
                "password reset email not sent: users.password_reset_url_template is set but does not contain <token>"
            );
        }
        PasswordResetLinkUnavailable::NotConfigured => {
            tracing::error!(
                "password reset email not sent: set users.password_reset_url_template, users.public_base_url (ELIDUNE_USERS__PUBLIC_BASE_URL), or server.public_base_url (ELIDUNE_SERVER__PUBLIC_BASE_URL)"
            );
        }
    }
}

/// RFC 3986 percent-encoding for a query component. Space is `%20`.
fn encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::config::{ServerConfig, UsersConfig};

    use super::{
        password_reset_public_base, plan_password_reset_link, render_password_reset_url,
        PasswordResetLink, PasswordResetLinkUnavailable, PASSWORD_RESET_UI_PATH,
    };

    fn test_users(public_base_url: Option<&str>) -> UsersConfig {
        UsersConfig {
            jwt_secret: "test-jwt-secret-at-least-32-characters".into(),
            jwt_expiration_hours: 24,
            public_base_url: public_base_url.map(str::to_string),
            password_reset_url_template: None,
        }
    }

    #[test]
    fn password_reset_public_base_reads_server_env_when_config_is_none() {
        let previous = std::env::var("ELIDUNE_SERVER__PUBLIC_BASE_URL").ok();
        std::env::set_var(
            "ELIDUNE_SERVER__PUBLIC_BASE_URL",
            "https://library.example.org",
        );
        let users = test_users(None);
        let server = ServerConfig {
            public_base_url: None,
            ..Default::default()
        };
        assert_eq!(
            password_reset_public_base(&users, &server).as_deref(),
            Some("https://library.example.org")
        );
        match previous {
            Some(value) => std::env::set_var("ELIDUNE_SERVER__PUBLIC_BASE_URL", value),
            None => std::env::remove_var("ELIDUNE_SERVER__PUBLIC_BASE_URL"),
        }
    }

    #[test]
    fn default_reset_url_is_the_ui_route_with_an_encoded_token() {
        assert_eq!(PASSWORD_RESET_UI_PATH, "/reset-password?token=");
        let link = plan_password_reset_link(None, Some("https://library.example.org/"))
            .expect("public origin");
        assert_eq!(
            link,
            PasswordResetLink::PublicBase("https://library.example.org".into())
        );
        assert_eq!(
            render_password_reset_url(&link, "a b+c/d?"),
            "https://library.example.org/reset-password?token=a%20b%2Bc%2Fd%3F"
        );
    }

    #[test]
    fn template_overrides_public_base_url() {
        let link = plan_password_reset_link(
            Some(" https://staff.example/reset?token=<token> "),
            Some("https://library.example.org"),
        )
        .expect("template");
        assert_eq!(
            render_password_reset_url(&link, "plain-token"),
            "https://staff.example/reset?token=plain-token"
        );
    }

    #[test]
    fn missing_template_and_public_base_url_cannot_build_a_link() {
        assert_eq!(
            plan_password_reset_link(None, None),
            Err(PasswordResetLinkUnavailable::NotConfigured)
        );
        assert_eq!(
            plan_password_reset_link(Some("  "), Some("///")),
            Err(PasswordResetLinkUnavailable::NotConfigured)
        );
        assert_eq!(
            plan_password_reset_link(
                Some("https://staff.example/reset"),
                Some("https://library.example.org")
            ),
            Err(PasswordResetLinkUnavailable::TemplateMissingPlaceholder)
        );
    }
}
