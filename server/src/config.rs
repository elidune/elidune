//! Configuration management for Elidune server

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: String,
    pub smtp_from_name: Option<String>,
    pub smtp_use_tls: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub redis: RedisConfig,
}

impl AppConfig {
    /// Load configuration from files and environment variables
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let config = Config::builder()
            // Start with default configuration
            .add_source(File::with_name("config/default"))
            // Layer on the environment-specific file
            .add_source(File::with_name(&format!("config/{}", run_mode)).required(false))
            // Add environment variables (with prefix ELIDUNE_)
            .add_source(
                Environment::with_prefix("ELIDUNE")
                    .separator("_")
                    .try_parsing(true),
            )
            // Override database URL from DATABASE_URL env var if present
            .set_override_option(
                "database.url",
                env::var("DATABASE_URL").ok(),
            )?
            // Override JWT secret from JWT_SECRET env var if present
            .set_override_option(
                "auth.jwt_secret",
                env::var("JWT_SECRET").ok(),
            )?
            // Override Redis URL from REDIS_URL env var if present
            .set_override_option(
                "redis.url",
                env::var("REDIS_URL").ok(),
            )?
            .build()?;

        config.try_deserialize()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://elidune:elidune@localhost:5432/elidune".to_string(),
            max_connections: 10,
            min_connections: 2,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-this-secret-in-production".to_string(),
            jwt_expiration_hours: 24,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: "localhost".to_string(),
            smtp_port: 587,
            smtp_username: None,
            smtp_password: None,
            smtp_from: "noreply@elidune.org".to_string(),
            smtp_from_name: Some("Elidune".to_string()),
            smtp_use_tls: true,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
        }
    }
}


