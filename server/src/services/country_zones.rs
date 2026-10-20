//! Country → IANA zone list shared with the UI.
//!
//! chrono-tz 0.10 exposes timezone names and offsets only. It has no country
//! filter, so the list is generated once from tzdata `zone.tab` (checked
//! against `iso3166.tab`) by `ui/scripts/generate-country-timezones.mjs`.
//! The JSON file is the single source: `server/data/countryTimezones.json`.
//! This module embeds that file and does not read anything under `ui/`.
//! The UI imports the same path. A country with one zone uses that zone.
//! A country with several zones has no default. The server images copy
//! `server/data/` next to the crate manifest so the path below resolves
//! inside the image as well as in the repository.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

const ZONES_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/countryTimezones.json"));

#[derive(Deserialize)]
struct ZoneEntry {
    id: String,
}

fn zones_by_country() -> &'static HashMap<String, Vec<String>> {
    static ZONES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    ZONES.get_or_init(|| {
        let raw: HashMap<String, Vec<ZoneEntry>> = serde_json::from_str(ZONES_JSON).expect("country time zone table");
        raw.into_iter().map(|(code, zones)| (code, zones.into_iter().map(|zone| zone.id).collect())).collect()
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
    zones_by_country().get(&country).is_some_and(|zones| zones.iter().any(|zone| zone == timezone))
}

/// Zones listed for `country`, in `zone.tab` order. Empty when the code is unknown.
#[must_use]
pub fn zones_for_country(country: &str) -> &'static [String] {
    let country = country.trim().to_ascii_uppercase();
    zones_by_country().get(&country).map(Vec::as_slice).unwrap_or(&[])
}

/// The only zone of a single-zone country.
#[must_use]
pub fn single_zone_for_country(country: &str) -> Option<&'static str> {
    let zones = zones_for_country(country);
    if zones.len() == 1 {
        Some(zones[0].as_str())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn france_has_one_zone_and_territories_keep_their_own() {
        assert!(is_known_country_code("FR"));
        assert!(is_known_country_code("fr"));
        assert!(!is_known_country_code("XX"));
        assert!(!is_known_country_code("en"));
        assert!(!is_known_country_code("el"));
        assert_eq!(single_zone_for_country("FR"), Some("Europe/Paris"));
        assert!(timezone_belongs_to_country("Europe/Paris", "FR"));
        assert!(!timezone_belongs_to_country("America/New_York", "FR"));
        assert!(zones_for_country("US").len() > 1);
        assert_eq!(single_zone_for_country("US"), None);
        assert!(timezone_belongs_to_country("America/Chicago", "US"));
        assert!(timezone_belongs_to_country("America/New_York", "US"));
        assert!(!timezone_belongs_to_country("Europe/Paris", "US"));
        assert_eq!(single_zone_for_country("RE"), Some("Indian/Reunion"));
        assert_eq!(single_zone_for_country("GP"), Some("America/Guadeloupe"));
        assert_eq!(single_zone_for_country("MQ"), Some("America/Martinique"));
    }

    #[test]
    fn every_zone_parses_with_the_embedded_chrono_tz() {
        for (country, zones) in zones_by_country() {
            assert!(!zones.is_empty(), "{country} has no zones");
            for zone in zones {
                assert!(zone.parse::<chrono_tz::Tz>().is_ok(), "{zone} ({country}) is not a chrono-tz timezone");
            }
        }
    }

    #[test]
    fn migration_keeps_only_codes_from_this_table() {
        let sql = include_str!("../../migrations/043_library_country_iso.sql");
        assert!(sql.contains("SET addr_country = NULL"));
        let executable = sql.lines().map(|line| line.split("--").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        assert!(!executable.to_lowercase().contains("timezone"));
        for code in zones_by_country().keys() {
            assert!(sql.contains(&format!("'{code}'")), "migration missing country {code}");
        }
    }
}
