//! Configuration management for Elidune server

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;
use std::path::Path;

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
pub struct UsersConfig {
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
    #[serde(default = "default_z3950_cache_ttl")]
    pub z3950_cache_ttl_seconds: u64,
}

fn default_z3950_cache_ttl() -> u64 {
    7 * 24 * 3600 // 7 days in seconds
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub users: UsersConfig,
    pub logging: LoggingConfig,
    pub email: EmailConfig,
    pub redis: RedisConfig,
}

impl AppConfig {
  

    /// Load configuration from the given file path (and environment overrides).
    /// When path is None, uses default paths (config/default + config/{RUN_MODE}).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
    
        
        let config = Config::builder().add_source(File::from(path.as_ref().to_path_buf().as_path()).required(true)).build()?;

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


impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

