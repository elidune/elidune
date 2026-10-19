//! Configuration management for Elidune server

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Allowed CORS origins in production (e.g. `["https://app.example.com"]`).
    /// When absent or empty, all origins are allowed (development mode).
    #[serde(default)]
    pub cors_origins: Option<Vec<String>>,
    /// Reverse proxies allowed to supply the client address.
    /// Comma-separated CIDRs or single IPs (for example `127.0.0.1/32,10.0.0.0/8`).
    /// Environment: `ELIDUNE_SERVER__TRUSTED_PROXIES`.
    /// Unset or empty: `X-Forwarded-For`, `X-Real-IP`, and `Forwarded` are ignored
    /// and the TCP peer is the client.
    #[serde(default)]
    pub trusted_proxies: Option<String>,
    /// Strict per-IP ceiling on unauthenticated auth routes (login, 2FA,
    /// password reset, first setup), in requests per minute. Default: 30,
    /// sized for several desks behind one NAT.
    #[serde(default = "default_auth_ip_per_minute")]
    pub auth_ip_per_minute: u64,
    /// Burst for [`Self::auth_ip_per_minute`]. Default: 30.
    #[serde(default = "default_auth_ip_burst")]
    pub auth_ip_burst: u32,
    /// Failed sign-in attempts, and failed password-reset token submissions,
    /// allowed per identifier inside [`Self::auth_identifier_window_secs`].
    /// Default: 5. Password-reset *requests* use [`Self::auth_password_reset_max`]
    /// instead. A temporary block, not an account lockout.
    #[serde(default = "default_auth_identifier_max_failures")]
    pub auth_identifier_max_failures: u32,
    /// Sliding window for the per-identifier auth attempt limit, in seconds.
    /// Default: 900 (15 minutes).
    #[serde(default = "default_auth_identifier_window_secs")]
    pub auth_identifier_window_secs: u64,
    /// Password-reset emails allowed for one resolved account inside
    /// [`Self::auth_password_reset_window_secs`]. Default: 3. Login and email
    /// for that account share the window. An unknown identifier keeps its own
    /// window. On top of the per-IP cap. Past the quota the API still returns
    /// the generic 200 and sends no mail.
    #[serde(default = "default_auth_password_reset_max")]
    pub auth_password_reset_max: u32,
    /// Sliding window for password-reset requests, in seconds. Default: 3600.
    #[serde(default = "default_auth_password_reset_window_secs")]
    pub auth_password_reset_window_secs: u64,
    /// Requests per minute on authenticated auth routes (`/auth/me`,
    /// `/auth/profile`, …), keyed by user id. Default: 120.
    #[serde(default = "default_auth_session_per_minute")]
    pub auth_session_per_minute: u64,
    /// Burst for [`Self::auth_session_per_minute`]. Default: 60.
    #[serde(default = "default_auth_session_burst")]
    pub auth_session_burst: u32,
    /// Public OPAC / covers / library-info requests per minute per client IP.
    /// Default: 600.
    #[serde(default = "default_public_per_minute")]
    pub public_per_minute: u64,
    /// Burst for [`Self::public_per_minute`]. Default: 120.
    #[serde(default = "default_public_burst")]
    pub public_burst: u32,
    /// Public origin of the UI (no trailing slash). Announcement emails link to
    /// `{public_base_url}/events/unsubscribe?token=…`. `List-Unsubscribe` uses
    /// `{public_base_url}/api/v1/events/unsubscribe?token=…` on the same origin.
    /// When unset, both use `http://{host}:{port}`, with `0.0.0.0` and `::`
    /// rewritten to `localhost`.
    #[serde(default)]
    pub public_base_url: Option<String>,
}

fn default_max_connections() -> u32 {
    10
}

fn default_min_connections() -> u32 {
    2
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UsersConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    /// Public UI origin for password-reset links (no trailing slash). Environment:
    /// `ELIDUNE_USERS__PUBLIC_BASE_URL`. When unset, password reset falls back to
    /// `server.public_base_url` / `ELIDUNE_SERVER__PUBLIC_BASE_URL`.
    #[serde(default)]
    pub public_base_url: Option<String>,
    /// Optional admin override for the password-reset link. Must contain the literal
    /// `<token>` placeholder. When unset, the server uses
    /// `{public_base_url}/reset-password?token=<token>`. Client `resetUrl` is ignored.
    #[serde(default)]
    pub password_reset_url_template: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    /// "pretty" | "plain" | "json"
    pub format: String,
    /// "stdout" | "stderr" | "file" | "syslog"
    pub output: String,
    /// Path to log file; required when output = "file"
    pub file_path: Option<String>,
    /// "monthly" | "weekly" | "daily" | "never" (default: "daily"; only used when output = "file")
    pub file_rotation: Option<String>,
    /// Whether this section can be overridden via the DB settings table
    #[serde(default)]
    pub overridable: bool,
}

fn default_email_templates_dir() -> String {
    "data/email_templates".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from: String,
    pub smtp_from_name: Option<String>,
    pub smtp_use_tls: bool,
    #[serde(default = "default_email_templates_dir")]
    pub templates_dir: String,
    /// Whether this section can be overridden via the DB settings table
    #[serde(default)]
    pub overridable: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_z3950_cache_ttl")]
    pub z3950_cache_ttl_seconds: u64,
}

fn default_z3950_cache_ttl() -> u64 {
    7 * 24 * 3600
}

fn default_auth_ip_per_minute() -> u64 {
    30
}

fn default_auth_ip_burst() -> u32 {
    30
}

fn default_auth_identifier_max_failures() -> u32 {
    5
}

fn default_auth_identifier_window_secs() -> u64 {
    15 * 60
}

fn default_auth_password_reset_max() -> u32 {
    3
}

fn default_auth_password_reset_window_secs() -> u64 {
    60 * 60
}

fn default_auth_session_per_minute() -> u64 {
    120
}

fn default_auth_session_burst() -> u32 {
    60
}

fn default_public_per_minute() -> u64 {
    600
}

fn default_public_burst() -> u32 {
    120
}

fn default_meili_index() -> String {
    "items".to_string()
}

fn default_reminders_enabled() -> bool {
    true
}

fn default_reminder_frequency() -> u32 {
    7
}

fn default_first_reminder_delay_days() -> u32 {
    7
}

fn default_second_reminder_delay_days() -> u32 {
    14
}

fn default_formal_notice_delay_days() -> u32 {
    21
}

fn default_first_reminder_template() -> String {
    "overdue_reminder".to_string()
}

fn default_second_reminder_template() -> String {
    "overdue_second_reminder".to_string()
}

fn default_formal_notice_template() -> String {
    "overdue_formal_notice".to_string()
}

fn default_reminder_time() -> String {
    "09:00".to_string()
}

fn default_accrue_fines() -> bool {
    true
}

fn default_smtp_throttle_ms() -> u64 {
    100
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RemindersConfig {
    /// Whether the automatic reminder scheduler is enabled
    #[serde(default = "default_reminders_enabled")]
    pub enabled: bool,
    /// Legacy field kept so existing admin-config payloads still deserialize.
    /// Sending uses the three tier delays below (days after the due date).
    #[serde(default = "default_reminder_frequency")]
    pub frequency_days: u32,
    /// Days after due date before the first reminder may be sent.
    #[serde(default = "default_first_reminder_delay_days")]
    pub first_reminder_delay_days: u32,
    /// Days after due date before the second reminder may be sent.
    #[serde(default = "default_second_reminder_delay_days")]
    pub second_reminder_delay_days: u32,
    /// Days after due date before the formal notice may be sent.
    #[serde(default = "default_formal_notice_delay_days")]
    pub formal_notice_delay_days: u32,
    /// Email template id for the first reminder (`GET/PUT /settings/email-templates`).
    #[serde(default = "default_first_reminder_template")]
    pub first_reminder_template: String,
    /// Email template id for the second reminder.
    #[serde(default = "default_second_reminder_template")]
    pub second_reminder_template: String,
    /// Email template id for the formal notice (notification only; does not block the account).
    #[serde(default = "default_formal_notice_template")]
    pub formal_notice_template: String,
    /// When false, the first reminder is not sent and later tiers are off.
    #[serde(default = "default_reminders_enabled")]
    pub first_reminder_enabled: bool,
    /// When false, the second reminder is not sent and the formal notice is off.
    #[serde(default = "default_reminders_enabled")]
    pub second_reminder_enabled: bool,
    /// When false, the formal notice is not sent. The highest still-enabled tier is last.
    #[serde(default = "default_reminders_enabled")]
    pub formal_notice_enabled: bool,
    /// Time of day to send reminders automatically (HH:MM, 24h)
    #[serde(default = "default_reminder_time")]
    pub send_time: String,
    /// When true, the scheduler accrues overdue fines after the reminder clock.
    #[serde(default = "default_accrue_fines")]
    pub accrue_fines: bool,
    /// Delay in milliseconds between each email send to avoid SMTP rate limits
    #[serde(default = "default_smtp_throttle_ms")]
    pub smtp_throttle_ms: u64,
    /// Whether this section can be overridden via the DB settings table
    #[serde(default)]
    pub overridable: bool,
}

fn default_audit_retention() -> u32 {
    365
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuditConfig {
    /// Number of days to retain audit log entries (older entries are deleted by the scheduler)
    #[serde(default = "default_audit_retention")]
    pub retention_days: u32,
    /// Whether this section can be overridden via the DB settings table
    #[serde(default)]
    pub overridable: bool,
}

impl Default for RemindersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency_days: 7,
            first_reminder_delay_days: 7,
            second_reminder_delay_days: 14,
            formal_notice_delay_days: 21,
            first_reminder_template: "overdue_reminder".to_string(),
            second_reminder_template: "overdue_second_reminder".to_string(),
            formal_notice_template: "overdue_formal_notice".to_string(),
            first_reminder_enabled: true,
            second_reminder_enabled: true,
            formal_notice_enabled: true,
            send_time: "09:00".to_string(),
            accrue_fines: true,
            smtp_throttle_ms: 100,
            overridable: false,
        }
    }
}

impl RemindersConfig {
    /// A disabled tier turns off every later tier so the chain has no holes.
    pub fn normalize_tier_enabled(&mut self) {
        if !self.first_reminder_enabled {
            self.second_reminder_enabled = false;
            self.formal_notice_enabled = false;
        } else if !self.second_reminder_enabled {
            self.formal_notice_enabled = false;
        }
    }

    /// Exclusive upper bound on `loans.reminder_count` for sendable tiers.
    /// `0` means no tier is enabled.
    #[must_use]
    pub fn max_sendable_reminder_count(&self) -> i32 {
        let mut cfg = self.clone();
        cfg.normalize_tier_enabled();
        if !cfg.first_reminder_enabled {
            0
        } else if !cfg.second_reminder_enabled {
            1
        } else if !cfg.formal_notice_enabled {
            2
        } else {
            3
        }
    }
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            retention_days: 365,
            overridable: false,
        }
    }
}

fn default_auto_anonymize_years() -> u32 {
    0
}

/// Patron personal-data retention (RGPD). `0` disables the auto-erasure job.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PrivacyConfig {
    /// Years after `users.expiry_at` before the scheduler anonymizes remaining patrons.
    /// Typical municipal-library value is `3`. `0` (default) disables the job.
    #[serde(default = "default_auto_anonymize_years")]
    pub auto_anonymize_years_after_expiry: u32,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            auto_anonymize_years_after_expiry: default_auto_anonymize_years(),
        }
    }
}

fn default_hold_ready_expiry_days() -> u32 {
    7
}

/// Hold behaviour (pickup window when a copy becomes available).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HoldsConfig {
    /// Days a `ready` hold stays valid for pickup (`expires_at` after notification).
    #[serde(default = "default_hold_ready_expiry_days")]
    pub ready_expiry_days: u32,
    /// Whether this section can be overridden via the DB `settings` table and admin API
    #[serde(default)]
    pub overridable: bool,
}

impl Default for HoldsConfig {
    fn default() -> Self {
        Self {
            ready_expiry_days: 7,
            overridable: false,
        }
    }
}

fn default_skip_closed_days() -> bool {
    true
}

/// Circulation due-date policy (public-library default: skip closed days).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CirculationConfig {
    /// When true, checkout and renew push `expiry_at` to the next open day if the
    /// computed due date is a weekly closed weekday or a `schedule_closures` day.
    #[serde(default = "default_skip_closed_days")]
    pub skip_closed_days: bool,
    /// Whether this section can be overridden via the DB `settings` table and admin API
    #[serde(default)]
    pub overridable: bool,
}

impl Default for CirculationConfig {
    fn default() -> Self {
        Self {
            skip_closed_days: true,
            overridable: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MeilisearchConfig {
    /// Meilisearch server URL, e.g. "http://meilisearch:7700"
    pub url: String,
    /// Master key / API key (optional for development without auth)
    pub api_key: Option<String>,
    /// Name of the Meilisearch index to use for catalog search
    #[serde(default = "default_meili_index")]
    pub index_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub users: UsersConfig,
    pub logging: LoggingConfig,
    pub email: EmailConfig,
    pub redis: RedisConfig,
    #[serde(default)]
    pub reminders: RemindersConfig,
    #[serde(default)]
    pub audit: AuditConfig,
    /// Holds / physical item queue. Accepts legacy TOML section `[reservations]`.
    #[serde(default, alias = "reservations")]
    pub holds: HoldsConfig,
    /// Due-date calendar policy. Defaults to skipping closed days (public library).
    #[serde(default)]
    pub circulation: CirculationConfig,
    #[serde(default)]
    pub meilisearch: Option<MeilisearchConfig>,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

impl AppConfig {
    /// Load configuration from the given file path.
    pub fn load(path: Option<impl AsRef<Path>>) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();
        if let Some(path) = path {
            builder = builder
                .add_source(File::from(path.as_ref().to_path_buf().as_path()).required(false));
        }

        let config = builder
            .add_source(
                Environment::with_prefix("ELIDUNE")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?;

        config.try_deserialize()
    }

    /// Reject insecure defaults before serving traffic.
    pub fn validate_security(&self) -> Result<(), String> {
        const DEFAULT_SECRET: &str = "change-this-secret-in-production";
        const MIN_SECRET_LEN: usize = 32;

        if self.users.jwt_secret == DEFAULT_SECRET {
            return Err("users.jwt_secret is the sample default — set a strong secret via config or ELIDUNE_USERS__JWT_SECRET".into());
        }
        if self.users.jwt_secret.len() < MIN_SECRET_LEN {
            return Err(format!(
                "users.jwt_secret must be at least {MIN_SECRET_LEN} characters"
            ));
        }

        let prod_mode = std::env::var("ELIDUNE_PRODUCTION").ok().as_deref() == Some("true");
        if prod_mode {
            let origins = self.server.cors_origins.as_deref().unwrap_or(&[]);
            if origins.is_empty() {
                return Err("server.cors_origins must be set when ELIDUNE_PRODUCTION=true".into());
            }
        }
        Ok(())
    }

    /// Warning when outbound mail is configured but links would point at this machine.
    ///
    /// Email is treated as enabled when `smtp_host` is non-empty.
    #[must_use]
    pub fn email_public_base_url_warning(&self) -> Option<&'static str> {
        if self.email.smtp_host.trim().is_empty() {
            return None;
        }
        let configured = self
            .server
            .public_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        match configured {
            None => Some(
                "ELIDUNE_SERVER__PUBLIC_BASE_URL is unset while email is enabled; outgoing links fall back to localhost. Set the public UI origin before sending mail.",
            ),
            Some(url) if public_base_url_is_local(url) => Some(
                "ELIDUNE_SERVER__PUBLIC_BASE_URL points at localhost while email is enabled; mail links will not work for patrons. Set the public UI origin.",
            ),
            Some(_) => None,
        }
    }

    /// Meilisearch production mode rejects master keys shorter than 16 bytes.
    pub fn validate_meilisearch_key(&self) -> Result<(), String> {
        let Some(meili) = &self.meilisearch else {
            return Ok(());
        };
        let Some(key) = meili.api_key.as_deref() else {
            return Ok(());
        };
        let key = key.trim();
        if key.is_empty() || key.len() >= 16 {
            return Ok(());
        }
        Err(format!(
            "meilisearch.api_key is {} bytes; Meilisearch production mode requires at least 16. Set ELIDUNE_MEILISEARCH__API_KEY to a longer key.",
            key.len()
        ))
    }

    /// Public UI origin used in announcement emails, without a trailing slash.
    #[must_use]
    pub fn public_base_url(&self) -> String {
        if let Some(configured) = self.server.public_base_url.as_deref() {
            let trimmed = configured.trim().trim_end_matches('/');
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        let host = match self.server.host.as_str() {
            "0.0.0.0" | "::" | "" => "localhost",
            other => other,
        };
        format!("http://{host}:{}", self.server.port)
    }

    /// Minimal configuration for integration tests (uses `DATABASE_URL` / `REDIS_URL` env vars).
    pub fn for_test() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://elidune:elidune@localhost:5432/elidune_test".into());
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig {
                url: database_url,
                max_connections: 5,
                min_connections: 1,
            },
            users: UsersConfig {
                jwt_secret: "integration-test-jwt-secret-min-32-chars".into(),
                jwt_expiration_hours: 24,
                public_base_url: None,
                password_reset_url_template: None,
            },
            logging: LoggingConfig::default(),
            email: EmailConfig {
                smtp_host: String::new(),
                smtp_port: 587,
                smtp_username: None,
                smtp_password: None,
                smtp_from: "test@elidune.local".into(),
                smtp_from_name: Some("Elidune Test".into()),
                smtp_use_tls: false,
                templates_dir: "data/email_templates".into(),
                overridable: true,
            },
            redis: RedisConfig {
                url: redis_url,
                z3950_cache_ttl_seconds: 3600,
            },
            reminders: RemindersConfig {
                enabled: false,
                overridable: true,
                ..RemindersConfig::default()
            },
            audit: AuditConfig::default(),
            holds: HoldsConfig {
                overridable: true,
                ..HoldsConfig::default()
            },
            circulation: CirculationConfig {
                overridable: true,
                ..CirculationConfig::default()
            },
            meilisearch: None,
            privacy: PrivacyConfig::default(),
        }
    }
}

fn public_base_url_is_local(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("://localhost")
        || lower.contains("://127.0.0.1")
        || lower.contains("://[::1]")
        || lower.contains("://0.0.0.0")
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            cors_origins: None,
            trusted_proxies: None,
            auth_ip_per_minute: default_auth_ip_per_minute(),
            auth_ip_burst: default_auth_ip_burst(),
            auth_identifier_max_failures: default_auth_identifier_max_failures(),
            auth_identifier_window_secs: default_auth_identifier_window_secs(),
            auth_password_reset_max: default_auth_password_reset_max(),
            auth_password_reset_window_secs: default_auth_password_reset_window_secs(),
            auth_session_per_minute: default_auth_session_per_minute(),
            auth_session_burst: default_auth_session_burst(),
            public_per_minute: default_public_per_minute(),
            public_burst: default_public_burst(),
            public_base_url: None,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            output: "stdout".to_string(),
            file_path: None,
            file_rotation: None,
            overridable: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, DatabaseConfig, MeilisearchConfig};

    #[test]
    fn database_pool_sizes_default_when_omitted() {
        let db: DatabaseConfig =
            serde_json::from_str(r#"{"url":"postgres://localhost/elidune"}"#).expect("db config");
        assert_eq!(db.max_connections, 10);
        assert_eq!(db.min_connections, 2);
    }

    #[test]
    fn warns_when_mail_is_enabled_without_a_public_origin() {
        let mut cfg = AppConfig::for_test();
        assert!(cfg.email_public_base_url_warning().is_none());
        cfg.email.smtp_host = "smtp.example.com".into();
        let unset = cfg.email_public_base_url_warning().expect("unset warning");
        assert!(unset.contains("unset"));
        cfg.server.public_base_url = Some("http://localhost:8181".into());
        let local = cfg
            .email_public_base_url_warning()
            .expect("localhost warning");
        assert!(local.contains("localhost"));
        cfg.server.public_base_url = Some("https://library.example.org".into());
        assert!(cfg.email_public_base_url_warning().is_none());
    }

    #[test]
    fn short_meilisearch_key_is_rejected() {
        let mut cfg = AppConfig::for_test();
        assert!(cfg.validate_meilisearch_key().is_ok());
        cfg.meilisearch = Some(MeilisearchConfig {
            url: "http://127.0.0.1:7700".into(),
            api_key: Some("changeme".into()),
            index_name: "items".into(),
        });
        let err = cfg.validate_meilisearch_key().expect_err("short key");
        assert!(err.contains("16"), "{err}");
    }
}
