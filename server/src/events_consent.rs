//! Event-announcement consent: signed unsubscribe tokens and localized copy.
//!
//! The token is an HMAC-SHA256 over `events_unsubscribe:{user_id}`, keyed with
//! the server JWT secret. It carries a purpose tag so an auth JWT cannot be
//! replayed here. There is no expiry: the link in an old announcement must
//! still withdraw consent.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{
    error::{AppError, AppResult},
    models::Language,
};

type HmacSha256 = Hmac<Sha256>;

const PURPOSE: &str = "events_unsubscribe";

/// Plain-text body returned for a missing, unknown, or tampered token.
/// The same sentence is used for every failure so the response does not
/// reveal whether the account exists.
pub const INVALID_REQUEST: &str = "Invalid request.\n";

/// Sign an unsubscribe token for `user_id`.
pub fn sign(secret: &[u8], user_id: i64) -> AppResult<String> {
    let payload = format!("{PURPOSE}:{user_id}");
    let mac = hmac_sha256(secret, payload.as_bytes())?;
    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload.as_bytes()),
        URL_SAFE_NO_PAD.encode(mac)
    ))
}

/// Recover the user id from a token signed with `secret`.
/// Returns `None` when the token is missing a part, the MAC does not match,
/// or the purpose tag is not [`PURPOSE`].
#[must_use]
pub fn verify(secret: &[u8], token: &str) -> Option<i64> {
    let (payload_b64, mac_b64) = token.split_once('.')?;
    if payload_b64.is_empty() || mac_b64.is_empty() || mac_b64.contains('.') {
        return None;
    }
    let payload = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let presented = URL_SAFE_NO_PAD.decode(mac_b64).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(&payload);
    mac.verify_slice(&presented).ok()?;
    let payload = std::str::from_utf8(&payload).ok()?;
    let user_id = payload.strip_prefix(&format!("{PURPOSE}:"))?;
    if user_id.is_empty()
        || !user_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    user_id.parse().ok()
}

fn hmac_sha256(secret: &[u8], data: &[u8]) -> AppResult<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|err| {
        AppError::Internal(format!("events unsubscribe token key rejected: {err}"))
    })?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

/// `List-Unsubscribe` and `List-Unsubscribe-Post` values for one announcement.
#[must_use]
pub fn list_unsubscribe_headers(url: &str) -> [(&'static str, String); 2] {
    [
        ("List-Unsubscribe", format!("<{url}>")),
        (
            "List-Unsubscribe-Post",
            "List-Unsubscribe=One-Click".to_string(),
        ),
    ]
}

struct ConsentCopy {
    link_label: &'static str,
    migration_notice: &'static str,
    /// Separator between a label and a URL. French keeps a space before the colon.
    label_separator: &'static str,
}

fn copy_for(language: Option<&str>) -> ConsentCopy {
    match language.map(Language::from).unwrap_or(Language::French) {
        Language::English => ConsentCopy {
            link_label: "Unsubscribe from event announcements",
            migration_notice: "You are receiving this message because your existing library account was opted in to the events programme. To refuse these messages, use the unsubscribe link.",
            label_separator: ": ",
        },
        Language::German => ConsentCopy {
            link_label: "Von Veranstaltungsankündigungen abmelden",
            migration_notice: "Sie erhalten diese Nachricht, weil Ihr bestehendes Bibliothekskonto für das Veranstaltungsprogramm angemeldet wurde. Um diese Nachrichten abzulehnen, verwenden Sie den Abmeldelink.",
            label_separator: ": ",
        },
        Language::Spanish => ConsentCopy {
            link_label: "Cancelar la suscripción a los anuncios de eventos",
            migration_notice: "Recibe este mensaje porque su cuenta existente fue inscrita en el programa de eventos. Para rechazar estos mensajes, use el enlace de cancelación.",
            label_separator: ": ",
        },
        _ => ConsentCopy {
            link_label: "Se désabonner des annonces d'événements",
            migration_notice: "Vous recevez ce message parce que votre compte existant a été inscrit au programme des événements. Pour refuser ces messages, utilisez le lien de désabonnement.",
            label_separator: " : ",
        },
    }
}

/// Append the migration explanation (once) and the unsubscribe link to both bodies.
pub fn append_announcement_footer(
    plain: &mut String,
    html: &mut String,
    language: Option<&str>,
    include_migration_notice: bool,
    unsubscribe_url: &str,
) {
    let copy = copy_for(language);
    if include_migration_notice {
        plain.push_str("\n\n");
        plain.push_str(copy.migration_notice);
        plain.push('\n');
        html.push_str("<p>");
        html.push_str(&html_escape(copy.migration_notice));
        html.push_str("</p>");
    }
    let line = format!(
        "{}{}{}",
        copy.link_label, copy.label_separator, unsubscribe_url
    );
    plain.push('\n');
    plain.push_str(&line);
    plain.push('\n');
    html.push_str("<p><a href=\"");
    html.push_str(&html_escape(unsubscribe_url));
    html.push_str("\">");
    html.push_str(&html_escape(copy.link_label));
    html.push_str("</a></p>");
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"integration-test-jwt-secret-min-32-chars";

    #[test]
    fn sign_and_verify_round_trip() {
        let token = sign(SECRET, 42).expect("sign");
        assert_eq!(verify(SECRET, &token), Some(42));
        assert!(token.contains('.'));
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let token = sign(SECRET, 7).expect("sign");
        let (payload, mac) = token.split_once('.').expect("two parts");
        let flipped = {
            let mut chars: Vec<u8> = mac.as_bytes().to_vec();
            let first = chars.first_mut().expect("mac");
            *first = if *first == b'A' { b'B' } else { b'A' };
            String::from_utf8(chars).expect("ascii mac")
        };
        assert_eq!(verify(SECRET, &format!("{payload}.{flipped}")), None);
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let token = sign(SECRET, 7).expect("sign");
        let (_payload, mac) = token.split_once('.').expect("two parts");
        let other = URL_SAFE_NO_PAD.encode(format!("{PURPOSE}:8").as_bytes());
        assert_eq!(verify(SECRET, &format!("{other}.{mac}")), None);
    }

    #[test]
    fn wrong_secret_is_rejected() {
        let token = sign(SECRET, 7).expect("sign");
        assert_eq!(verify(b"another-secret-that-is-long-enough", &token), None);
    }

    #[test]
    fn missing_parts_and_empty_token_are_rejected() {
        assert_eq!(verify(SECRET, ""), None);
        assert_eq!(verify(SECRET, "only-one-part"), None);
        assert_eq!(verify(SECRET, ".abc"), None);
        assert_eq!(verify(SECRET, "abc."), None);
    }

    #[test]
    fn negative_user_id_round_trips() {
        let token = sign(SECRET, -15).expect("sign");
        assert_eq!(verify(SECRET, &token), Some(-15));
    }

    #[test]
    fn french_link_has_a_space_before_the_colon() {
        let mut plain = String::new();
        let mut html = String::new();
        append_announcement_footer(
            &mut plain,
            &mut html,
            Some("french"),
            true,
            "https://library.example/unsub",
        );
        assert!(plain
            .contains("Se désabonner des annonces d'événements : https://library.example/unsub"));
        assert!(plain.contains("compte existant a été inscrit"));
        assert!(html.contains("compte existant a été inscrit"));
        assert!(!plain.contains("You are receiving this message"));
    }

    #[test]
    fn english_german_and_spanish_have_no_space_before_the_colon() {
        for (lang, label) in [
            (
                "english",
                "Unsubscribe from event announcements: https://library.example/unsub",
            ),
            (
                "german",
                "Von Veranstaltungsankündigungen abmelden: https://library.example/unsub",
            ),
            (
                "spanish",
                "Cancelar la suscripción a los anuncios de eventos: https://library.example/unsub",
            ),
        ] {
            let mut plain = String::new();
            let mut html = String::new();
            append_announcement_footer(
                &mut plain,
                &mut html,
                Some(lang),
                false,
                "https://library.example/unsub",
            );
            assert!(plain.contains(label), "{lang}: {plain}");
            assert!(
                !plain.contains("compte existant"),
                "{lang} must not include the migration notice"
            );
            assert!(html.contains("href=\"https://library.example/unsub\""));
        }
    }

    #[test]
    fn unknown_language_uses_french_link_label() {
        let mut plain = String::new();
        let mut html = String::new();
        append_announcement_footer(
            &mut plain,
            &mut html,
            Some("italian"),
            false,
            "https://library.example/events/unsubscribe?token=abc",
        );
        assert!(plain.contains("Se désabonner des annonces d'événements : https://library.example/events/unsubscribe?token=abc"));
        assert!(!plain.contains("Unsubscribe from event announcements:"));
    }

    #[test]
    fn list_unsubscribe_headers_follow_rfc_8058() {
        let headers =
            list_unsubscribe_headers("https://library.example/api/v1/events/unsubscribe?token=abc");
        assert_eq!(headers[0].0, "List-Unsubscribe");
        assert_eq!(
            headers[0].1,
            "<https://library.example/api/v1/events/unsubscribe?token=abc>"
        );
        assert_eq!(headers[1].0, "List-Unsubscribe-Post");
        assert_eq!(headers[1].1, "List-Unsubscribe=One-Click");
    }
}
