//! Library information service

use crate::{
    error::AppResult,
    models::dto::library_info::{LibraryInfo, UpdateLibraryInfoRequest},
    repository::Repository,
    services::{
        calendar::{self, DEFAULT_TIMEZONE},
        country_code,
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
        let timezone = match req.timezone.as_deref() {
            Some(name) => Some(calendar::parse_timezone(name)?.name().to_string()),
            None => None,
        };
        let addr_country = country_code::normalize_optional_country(req.addr_country)?;

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
