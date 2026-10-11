//! Library information service

use crate::{
    error::{AppError, AppResult},
    models::dto::library_info::{LibraryInfo, UpdateLibraryInfoRequest},
    repository::Repository,
    services::{
        calendar::{self, DEFAULT_TIMEZONE},
        country_code, country_zones,
    },
};

#[derive(Clone)]
pub struct LibraryInfoService {
    repository: Repository,
}

impl LibraryInfoService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Get library information (always returns a record, empty if not yet set)
    #[tracing::instrument(skip(self), err)]
    pub async fn get(&self) -> AppResult<LibraryInfo> {
        let result = self.repository.library_info_get().await?;

        match result {
            Some((
                name,
                addr_line1,
                addr_line2,
                addr_postcode,
                addr_city,
                addr_country,
                phones_val,
                email,
                timezone,
                updated_at,
            )) => {
                let phones: Vec<String> = phones_val
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                Ok(LibraryInfo {
                    name,
                    addr_line1,
                    addr_line2,
                    addr_postcode,
                    addr_city,
                    addr_country,
                    phones,
                    email,
                    timezone,
                    updated_at,
                })
            }
            None => Ok(LibraryInfo {
                name: None,
                addr_line1: None,
                addr_line2: None,
                addr_postcode: None,
                addr_city: None,
                addr_country: None,
                phones: vec![],
                email: None,
                timezone: DEFAULT_TIMEZONE.to_string(),
                updated_at: None,
            }),
        }
    }

    /// Update library information (partial update: only provided fields are changed)
    #[tracing::instrument(skip(self), err)]
    pub async fn update(&self, req: UpdateLibraryInfoRequest) -> AppResult<LibraryInfo> {
        let phones_json = req
            .phones
            .map(|p| serde_json::to_value(p).unwrap_or(serde_json::json!([])));
        let submitted = submitted_timezone(req.timezone.as_deref())?;
        let addr_country = country_code::normalize_optional_country(req.addr_country)?;

        // A name-only update must not read or rewrite the stored zone.
        let timezone = if addr_country.is_some() || !matches!(submitted, SubmittedTimezone::Omitted)
        {
            let current = self.get().await?;
            let country = addr_country.clone().or_else(|| {
                current
                    .addr_country
                    .and_then(|stored| country_code::normalize_country_code(&stored).ok())
            });
            resolve_timezone(&submitted, country.as_deref(), &current.timezone)?
        } else {
            None
        };

        self.repository
            .library_info_upsert(
                &req.name,
                &req.addr_line1,
                &req.addr_line2,
                &req.addr_postcode,
                &req.addr_city,
                &addr_country,
                phones_json,
                &req.email,
                &timezone,
            )
            .await?;

        self.get().await
    }
}

enum SubmittedTimezone {
    /// Field absent: leave the stored zone unless the country forces a choice.
    Omitted,
    /// Present but empty: the user cleared the choice.
    Blank,
    /// A name chrono-tz accepts.
    Set(String),
}

fn submitted_timezone(value: Option<&str>) -> AppResult<SubmittedTimezone> {
    match value {
        None => Ok(SubmittedTimezone::Omitted),
        Some(name) if name.trim().is_empty() => Ok(SubmittedTimezone::Blank),
        Some(name) => Ok(SubmittedTimezone::Set(
            calendar::parse_timezone(name)?.name().to_string(),
        )),
    }
}

/// Zone to write. `None` leaves the stored value (`COALESCE`).
///
/// A single-zone country always stores that zone when the request omits or
/// clears the field. A multi-zone country has no default: a blank field, or an
/// omitted field whose stored zone is outside the country, is `timezone_required`.
/// An explicit zone outside the country is `timezone_country_mismatch`.
fn resolve_timezone(
    submitted: &SubmittedTimezone,
    country: Option<&str>,
    stored: &str,
) -> AppResult<Option<String>> {
    let Some(country) = country else {
        return match submitted {
            SubmittedTimezone::Omitted => Ok(None),
            SubmittedTimezone::Blank => Err(AppError::InvalidTimezone(
                "Timezone is required".to_string(),
            )),
            SubmittedTimezone::Set(zone) => Ok(Some(zone.clone())),
        };
    };

    if let Some(only) = country_zones::single_zone_for_country(country) {
        return match submitted {
            SubmittedTimezone::Omitted | SubmittedTimezone::Blank => Ok(Some(only.to_string())),
            SubmittedTimezone::Set(zone)
                if country_zones::timezone_belongs_to_country(zone, country) =>
            {
                Ok(Some(zone.clone()))
            }
            SubmittedTimezone::Set(zone) => Err(AppError::TimezoneCountryMismatch(format!(
                "Timezone '{zone}' does not belong to country {country}"
            ))),
        };
    }

    match submitted {
        SubmittedTimezone::Blank => Err(timezone_required(country)),
        SubmittedTimezone::Omitted
            if country_zones::timezone_belongs_to_country(stored, country) =>
        {
            Ok(None)
        }
        SubmittedTimezone::Omitted => Err(timezone_required(country)),
        SubmittedTimezone::Set(zone)
            if country_zones::timezone_belongs_to_country(zone, country) =>
        {
            Ok(Some(zone.clone()))
        }
        SubmittedTimezone::Set(zone) => Err(AppError::TimezoneCountryMismatch(format!(
            "Timezone '{zone}' does not belong to country {country}"
        ))),
    }
}

fn timezone_required(country: &str) -> AppError {
    AppError::TimezoneRequired(format!("Choose a time zone for country {country}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(zone: &str) -> SubmittedTimezone {
        SubmittedTimezone::Set(zone.to_string())
    }

    #[test]
    fn single_zone_country_fills_its_only_zone() {
        assert_eq!(
            resolve_timezone(&SubmittedTimezone::Omitted, Some("FR"), "America/New_York").unwrap(),
            Some("Europe/Paris".to_string())
        );
        assert_eq!(
            resolve_timezone(&SubmittedTimezone::Blank, Some("RE"), "Europe/Paris").unwrap(),
            Some("Indian/Reunion".to_string())
        );
        assert_eq!(
            resolve_timezone(&set("Europe/Paris"), Some("FR"), "Europe/Paris").unwrap(),
            Some("Europe/Paris".to_string())
        );
    }

    #[test]
    fn multi_zone_country_has_no_default() {
        assert!(matches!(
            resolve_timezone(&SubmittedTimezone::Blank, Some("US"), "America/Chicago"),
            Err(AppError::TimezoneRequired(_))
        ));
        assert!(matches!(
            resolve_timezone(&SubmittedTimezone::Omitted, Some("US"), "Europe/Paris"),
            Err(AppError::TimezoneRequired(_))
        ));
        assert!(matches!(
            resolve_timezone(&SubmittedTimezone::Omitted, Some("DE"), "America/Chicago"),
            Err(AppError::TimezoneRequired(_))
        ));
        assert_eq!(
            resolve_timezone(&SubmittedTimezone::Omitted, Some("US"), "America/Chicago").unwrap(),
            None
        );
        assert_eq!(
            resolve_timezone(&set("America/New_York"), Some("US"), "America/Chicago").unwrap(),
            Some("America/New_York".to_string())
        );
    }

    #[test]
    fn explicit_foreign_zone_is_a_mismatch() {
        assert!(matches!(
            resolve_timezone(&set("America/New_York"), Some("FR"), "Europe/Paris"),
            Err(AppError::TimezoneCountryMismatch(_))
        ));
        assert!(matches!(
            resolve_timezone(&set("Europe/Paris"), Some("US"), "America/Chicago"),
            Err(AppError::TimezoneCountryMismatch(_))
        ));
    }

    #[test]
    fn no_country_stores_a_parsed_zone_and_rejects_a_blank_one() {
        assert_eq!(
            resolve_timezone(&set("Europe/Paris"), None, "Europe/Paris").unwrap(),
            Some("Europe/Paris".to_string())
        );
        assert!(matches!(
            resolve_timezone(&SubmittedTimezone::Blank, None, "Europe/Paris"),
            Err(AppError::InvalidTimezone(_))
        ));
        assert_eq!(
            resolve_timezone(&SubmittedTimezone::Omitted, None, "Europe/Paris").unwrap(),
            None
        );
    }
}
