//! Library identity row (`library_info` table, single row `id = 1`).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;

use super::Repository;
use crate::error::AppResult;

/// Snapshot of `library_info` columns returned by [`LibraryInfoRepository::library_info_get`].
pub type LibraryInfoSnapshot = Option<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
    Option<String>,
    String,
    Option<DateTime<Utc>>,
)>;

/// DB access for `library_info`. Implemented by [`Repository`].
#[async_trait]
pub trait LibraryInfoRepository: Send + Sync {
    async fn library_info_get(&self) -> AppResult<LibraryInfoSnapshot>;
    #[allow(clippy::too_many_arguments)]
    async fn library_info_upsert(
        &self,
        name: &Option<String>,
        addr_line1: &Option<String>,
        addr_line2: &Option<String>,
        addr_postcode: &Option<String>,
        addr_city: &Option<String>,
        addr_country: &Option<String>,
        phones_json: Option<serde_json::Value>,
        email: &Option<String>,
        timezone: &Option<String>,
    ) -> AppResult<()>;
}

#[async_trait]
impl LibraryInfoRepository for Repository {
    async fn library_info_get(&self) -> AppResult<LibraryInfoSnapshot> {
        Repository::library_info_get(self).await
    }

    async fn library_info_upsert(
        &self,
        name: &Option<String>,
        addr_line1: &Option<String>,
        addr_line2: &Option<String>,
        addr_postcode: &Option<String>,
        addr_city: &Option<String>,
        addr_country: &Option<String>,
        phones_json: Option<serde_json::Value>,
        email: &Option<String>,
        timezone: &Option<String>,
    ) -> AppResult<()> {
        Repository::library_info_upsert(
            self,
            name,
            addr_line1,
            addr_line2,
            addr_postcode,
            addr_city,
            addr_country,
            phones_json,
            email,
            timezone,
        )
        .await
    }
}

impl Repository {
    /// Fetch library_info row or `None` if never inserted.
    pub async fn library_info_get(&self) -> AppResult<LibraryInfoSnapshot> {
        let result = sqlx::query(
            r#"
            SELECT name, addr_line1, addr_line2, addr_postcode, addr_city, addr_country,
                   phones, email, timezone, updated_at
            FROM library_info WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|row| {
            (
                row.get("name"),
                row.get("addr_line1"),
                row.get("addr_line2"),
                row.get("addr_postcode"),
                row.get("addr_city"),
                row.get("addr_country"),
                row.get("phones"),
                row.get("email"),
                row.get("timezone"),
                row.get("updated_at"),
            )
        }))
    }

    /// IANA name stored on the library row, or [`crate::services::calendar::DEFAULT_TIMEZONE`].
    pub async fn library_timezone_name(&self) -> AppResult<String> {
        let name: Option<String> =
            sqlx::query_scalar("SELECT timezone FROM library_info WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        Ok(name.unwrap_or_else(|| crate::services::calendar::DEFAULT_TIMEZONE.to_string()))
    }

    /// Parsed library zone. A corrupt stored name falls back to Europe/Paris.
    pub async fn library_zone(&self) -> AppResult<chrono_tz::Tz> {
        let name = self.library_timezone_name().await?;
        match crate::services::calendar::parse_timezone(&name) {
            Ok(zone) => Ok(zone),
            Err(_) => {
                tracing::error!(
                    timezone = %name,
                    "invalid library timezone stored; using Europe/Paris"
                );
                Ok(crate::services::calendar::default_zone())
            }
        }
    }

    /// Upsert library_info (partial update via COALESCE in SQL).
    #[allow(clippy::too_many_arguments)]
    pub async fn library_info_upsert(
        &self,
        name: &Option<String>,
        addr_line1: &Option<String>,
        addr_line2: &Option<String>,
        addr_postcode: &Option<String>,
        addr_city: &Option<String>,
        addr_country: &Option<String>,
        phones_json: Option<serde_json::Value>,
        email: &Option<String>,
        timezone: &Option<String>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO library_info (id, name, addr_line1, addr_line2, addr_postcode,
                                      addr_city, addr_country, phones, email, timezone, updated_at)
            VALUES (1, $1, $2, $3, $4, $5, $6, COALESCE($7::jsonb, '[]'::jsonb), $8,
                    COALESCE($9, 'Europe/Paris'), NOW())
            ON CONFLICT (id) DO UPDATE SET
                name          = COALESCE($1, library_info.name),
                addr_line1    = COALESCE($2, library_info.addr_line1),
                addr_line2    = COALESCE($3, library_info.addr_line2),
                addr_postcode = COALESCE($4, library_info.addr_postcode),
                addr_city     = COALESCE($5, library_info.addr_city),
                addr_country  = COALESCE($6, library_info.addr_country),
                phones        = COALESCE($7::jsonb, library_info.phones),
                email         = COALESCE($8, library_info.email),
                timezone      = COALESCE($9, library_info.timezone),
                updated_at    = NOW()
            "#,
        )
        .bind(name)
        .bind(addr_line1)
        .bind(addr_line2)
        .bind(addr_postcode)
        .bind(addr_city)
        .bind(addr_country)
        .bind(phones_json)
        .bind(email)
        .bind(timezone)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
