//! Country → IANA zone list shared with the UI.
//!
//! chrono-tz 0.10 exposes timezone names and offsets only. It has no country
//! filter, so the list is generated once from tzdata `zone1970.tab` by
//! `ui/scripts/generate-country-timezones.mjs`. The JSON file is the single
//! source: the UI imports it, and this module embeds the same bytes.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

const ZONES_JSON: &str = include_str!("../../../ui/src/data/countryTimezones.json");

#[derive(Deserialize)]
struct ZoneEntry {
    id: String,
}

fn zones_by_country() -> &'static HashMap<String, Vec<String>> {
    static ZONES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    ZONES.get_or_init(|| {
        let raw: HashMap<String, Vec<ZoneEntry>> =
            serde_json::from_str(ZONES_JSON).expect("country time zone table");
        raw.into_iter()
            .map(|(code, zones)| (code, zones.into_iter().map(|zone| zone.id).collect()))
            .collect()
    })
}

/// `code` is an ISO alpha-2 key of the shared zone table.
#[must_use]
pub fn is_known_country_code(code: &str) -> bool {
    let code = code.trim().to_ascii_uppercase();
    code.len() == 2 && zones_by_country().contains_key(&code)
}

/// `timezone` is one of the IANA names listed for `country` (already an alpha-2 code).
#[must_use]
pub fn timezone_belongs_to_country(timezone: &str, country: &str) -> bool {
    let country = country.trim().to_ascii_uppercase();
    let timezone = timezone.trim();
    zones_by_country()
        .get(&country)
        .is_some_and(|zones| zones.iter().any(|zone| zone == timezone))
}

/// Default zone for `country`: the first entry in the shared table.
#[must_use]
pub fn default_timezone_for_country(country: &str) -> Option<&'static str> {
    let country = country.trim().to_ascii_uppercase();
    zones_by_country()
        .get(&country)
        .and_then(|zones| zones.first().map(String::as_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn france_has_a_single_zone_and_the_united_states_default_to_new_york() {
        assert!(is_known_country_code("FR"));
        assert!(is_known_country_code("fr"));
        assert!(!is_known_country_code("XX"));
        assert!(!is_known_country_code("en"));
        assert_eq!(default_timezone_for_country("FR"), Some("Europe/Paris"));
        assert!(timezone_belongs_to_country("Europe/Paris", "FR"));
        assert!(!timezone_belongs_to_country("America/New_York", "FR"));
        assert_eq!(default_timezone_for_country("US"), Some("America/New_York"));
        assert!(timezone_belongs_to_country("America/Chicago", "US"));
        assert!(!timezone_belongs_to_country("Europe/Paris", "US"));
    }

    #[test]
    fn migration_keeps_only_codes_from_this_table() {
        let sql = include_str!("../../migrations/043_library_country_iso.sql");
        assert!(sql.contains("SET addr_country = NULL"));
        let executable = sql
            .lines()
            .map(|line| line.split("--").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!executable.to_lowercase().contains("timezone"));
        for code in zones_by_country().keys() {
            assert!(
                sql.contains(&format!("'{code}'")),
                "migration missing country {code}"
            );
        }
    }
}
