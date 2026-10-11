//! Server-built password-reset links.
//!
//! The client `resetUrl` is never consulted. Resolution order:
//! 1. `users.password_reset_url_template` when it is set (admin-controlled).
//! 2. Otherwise `{PUBLIC_BASE_URL}/reset-password?token=<token>`.
//! 3. Otherwise no link, and the caller must not send mail.
//!
//! `PUBLIC_BASE_URL` is the configured origin only. This module does not read
//! the `Host` or `X-Forwarded-Host` header.

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

/// Origin for the default reset path.
///
/// Reads `public_base_url` when the config value has that key. A missing key
/// (builds whose server config has no such field) falls back to
/// `ELIDUNE_SERVER__PUBLIC_BASE_URL`. An explicit null does not. Never uses
/// the bind address or a request `Host` header.
pub fn configured_public_base(server: &impl serde::Serialize) -> Option<String> {
    let value = serde_json::to_value(server).ok()?;
    match value.get("public_base_url") {
        Some(serde_json::Value::String(url)) if !url.trim().is_empty() => Some(url.clone()),
        Some(_) => None,
        None => std::env::var("ELIDUNE_SERVER__PUBLIC_BASE_URL")
            .ok()
            .filter(|url| !url.trim().is_empty()),
    }
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
                "password reset email not sent: set users.password_reset_url_template or ELIDUNE_SERVER__PUBLIC_BASE_URL"
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
    use super::{
        plan_password_reset_link, render_password_reset_url, PasswordResetLink,
        PasswordResetLinkUnavailable, PASSWORD_RESET_UI_PATH,
    };

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
