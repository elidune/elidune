//! Library info DTOs shared by API and services.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Public UI language stored on the library (`fr`, `en`, `de`, `es`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LibraryUiLanguage {
    Fr,
    En,
    De,
    Es,
}

/// A library UI language outside `fr`, `en`, `de`, and `es`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidLibraryUiLanguage;

impl LibraryUiLanguage {
    /// Exact wire/database code.
    #[must_use]
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
            Self::De => "de",
            Self::Es => "es",
        }
    }
}

impl From<LibraryUiLanguage> for crate::models::Language {
    fn from(value: LibraryUiLanguage) -> Self {
        match value {
            LibraryUiLanguage::Fr => Self::French,
            LibraryUiLanguage::En => Self::English,
            LibraryUiLanguage::De => Self::German,
            LibraryUiLanguage::Es => Self::Spanish,
        }
    }
}

impl TryFrom<&str> for LibraryUiLanguage {
    type Error = InvalidLibraryUiLanguage;

    /// Accept only `fr`, `en`, `de`, and `es` with no trimming or case folding.
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "fr" => Ok(Self::Fr),
            "en" => Ok(Self::En),
            "de" => Ok(Self::De),
            "es" => Ok(Self::Es),
            _ => Err(InvalidLibraryUiLanguage),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub name: Option<String>,
    pub addr_line1: Option<String>,
    pub addr_line2: Option<String>,
    pub addr_postcode: Option<String>,
    pub addr_city: Option<String>,
    /// ISO 3166-1 alpha-2 code (for example `FR`). Unmapped legacy free text may still be stored.
    pub addr_country: Option<String>,
    pub phones: Vec<String>,
    pub email: Option<String>,
    /// IANA timezone name (for example `Europe/Paris`).
    pub timezone: String,
    /// Default public UI language. `null` means unset; clients fall back to `fr`.
    pub language: Option<LibraryUiLanguage>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryInfoRequest {
    pub name: Option<String>,
    pub addr_line1: Option<String>,
    pub addr_line2: Option<String>,
    pub addr_postcode: Option<String>,
    pub addr_city: Option<String>,
    /// Country. French and English names are stored as ISO 3166-1 alpha-2 (`France` → `FR`).
    /// A 2-letter code is kept only when it is in the shared zone table. Any other non-empty
    /// value is a 400 `validation_error`. Omitted or blank leaves the stored value unchanged.
    /// When a country is in effect, `timezone` must be one of that country's zones
    /// (`timezone_country_mismatch`). A country with one zone stores that zone when
    /// `timezone` is omitted or blank. A country with several zones has no default:
    /// a missing choice is 400 `timezone_required`.
    pub addr_country: Option<String>,
    pub phones: Option<Vec<String>>,
    pub email: Option<String>,
    /// IANA timezone name. Omitted fields are left unchanged unless the country
    /// requires a choice. Invalid names return 400 `invalid_timezone`.
    pub timezone: Option<String>,
    /// Default public UI language. Omit the field to leave the stored value unchanged.
    /// JSON `null` clears it. Allowed values are exactly `fr`, `en`, `de`, and `es`.
    /// Any other value, including a blank string, is 400 `validation_error`.
    #[serde(default, with = "serde_with::rust::double_option")]
    #[schema(value_type = Option<LibraryUiLanguage>)]
    pub language: Option<Option<String>>,
}

#[cfg(test)]
mod tests {
    use super::{LibraryInfo, LibraryUiLanguage, UpdateLibraryInfoRequest};
    use crate::models::Language;

    #[test]
    fn library_ui_language_maps_onto_the_notice_language() {
        assert_eq!(Language::from(LibraryUiLanguage::Fr), Language::French);
        assert_eq!(Language::from(LibraryUiLanguage::En), Language::English);
        assert_eq!(Language::from(LibraryUiLanguage::De), Language::German);
        assert_eq!(Language::from(LibraryUiLanguage::Es), Language::Spanish);
    }

    #[test]
    fn language_json_is_the_code_or_null() {
        let set = serde_json::to_value(LibraryUiLanguage::De).expect("code");
        assert_eq!(set, "de");
        let cleared = serde_json::to_value(Option::<LibraryUiLanguage>::None).expect("null");
        assert!(cleared.is_null());
    }

    #[test]
    fn update_omits_language_until_the_field_is_present() {
        let omitted: UpdateLibraryInfoRequest = serde_json::from_str("{}").expect("empty");
        assert!(omitted.language.is_none());

        let cleared: UpdateLibraryInfoRequest =
            serde_json::from_str(r#"{"language":null}"#).expect("null");
        assert!(matches!(cleared.language, Some(None)));

        let german: UpdateLibraryInfoRequest =
            serde_json::from_str(r#"{"language":"de"}"#).expect("de");
        assert_eq!(german.language, Some(Some("de".to_string())));
    }

    #[test]
    fn library_info_serializes_language_next_to_timezone() {
        let info = LibraryInfo {
            name: None,
            addr_line1: None,
            addr_line2: None,
            addr_postcode: None,
            addr_city: None,
            addr_country: None,
            phones: vec![],
            email: None,
            timezone: "Europe/Paris".into(),
            language: Some(LibraryUiLanguage::Es),
            updated_at: None,
        };
        let json = serde_json::to_value(info).expect("json");
        assert_eq!(json["language"], "es");
        assert_eq!(json["timezone"], "Europe/Paris");
    }
}
