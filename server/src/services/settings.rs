//! Settings service

use sqlx::Row;

use crate::{
    api::settings::{LoanSettings, SettingsResponse, UpdateSettingsRequest, Z3950ServerConfig},
    error::AppResult,
    repository::Repository,
};

#[derive(Clone)]
pub struct SettingsService {
    repository: Repository,
}

impl SettingsService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Get current settings
    pub async fn get_settings(&self) -> AppResult<SettingsResponse> {
        let pool = &self.repository.pool;

        // Get loan settings
        let loan_settings = sqlx::query(
            "SELECT media_type, nb_max, nb_renews, duration FROM loans_settings ORDER BY media_type"
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| LoanSettings {
            media_type: row.get::<Option<String>, _>("media_type").unwrap_or_default(),
            max_loans: row.get::<Option<i16>, _>("nb_max").unwrap_or(5),
            max_renewals: row.get::<Option<i16>, _>("nb_renews").unwrap_or(2),
            duration_days: row.get::<Option<i16>, _>("duration").unwrap_or(21),
        })
        .collect();

        // Get Z39.50 servers
        let z3950_servers = sqlx::query(
            "SELECT id, name, address, port, database, format, activated FROM z3950servers ORDER BY name"
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| Z3950ServerConfig {
            id: row.get("id"),
            name: row.get::<Option<String>, _>("name").unwrap_or_default(),
            address: row.get::<Option<String>, _>("address").unwrap_or_default(),
            port: row.get::<Option<i32>, _>("port").unwrap_or(2200),
            database: row.get("database"),
            format: row.get("format"),
            is_active: row.get::<Option<i32>, _>("activated").unwrap_or(0) == 1,
        })
        .collect();

        Ok(SettingsResponse {
            loan_settings,
            z3950_servers,
        })
    }

    /// Update settings
    pub async fn update_settings(&self, request: UpdateSettingsRequest) -> AppResult<SettingsResponse> {
        let pool = &self.repository.pool;

        // Update loan settings
        if let Some(loan_settings) = request.loan_settings {
            for setting in loan_settings {
                sqlx::query(
                    r#"
                    INSERT INTO loans_settings (media_type, nb_max, nb_renews, duration)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (media_type) DO UPDATE SET
                        nb_max = EXCLUDED.nb_max,
                        nb_renews = EXCLUDED.nb_renews,
                        duration = EXCLUDED.duration
                    "#,
                )
                .bind(&setting.media_type)
                .bind(setting.max_loans)
                .bind(setting.max_renewals)
                .bind(setting.duration_days)
                .execute(pool)
                .await?;
            }
        }

        // Update Z39.50 servers
        if let Some(z3950_servers) = request.z3950_servers {
            for server in z3950_servers {
                if server.id > 0 {
                    // Update existing
                    sqlx::query(
                        r#"
                        UPDATE z3950servers SET
                            name = $1, address = $2, port = $3, database = $4,
                            format = $5, activated = $6
                        WHERE id = $7
                        "#,
                    )
                    .bind(&server.name)
                    .bind(&server.address)
                    .bind(server.port)
                    .bind(&server.database)
                    .bind(&server.format)
                    .bind(if server.is_active { 1 } else { 0 })
                    .bind(server.id)
                    .execute(pool)
                    .await?;
                } else {
                    // Insert new
                    sqlx::query(
                        r#"
                        INSERT INTO z3950servers (name, address, port, database, format, activated)
                        VALUES ($1, $2, $3, $4, $5, $6)
                        "#,
                    )
                    .bind(&server.name)
                    .bind(&server.address)
                    .bind(server.port)
                    .bind(&server.database)
                    .bind(&server.format)
                    .bind(if server.is_active { 1 } else { 0 })
                    .execute(pool)
                    .await?;
                }
            }
        }

        self.get_settings().await
    }
}


