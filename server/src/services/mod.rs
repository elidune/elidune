//! Business logic services

pub mod auth;
pub mod catalog;
pub mod email;
pub mod loans;
pub mod redis;
pub mod settings;
pub mod stats;
pub mod z3950;

use crate::{config::{AuthConfig, EmailConfig}, error::AppResult, repository::Repository};

/// Container for all services
#[derive(Clone)]
pub struct Services {
    pub auth: auth::AuthService,
    pub catalog: catalog::CatalogService,
    pub users: auth::AuthService, // Users operations are part of auth service
    pub loans: loans::LoansService,
    pub z3950: z3950::Z3950Service,
    pub stats: stats::StatsService,
    pub settings: settings::SettingsService,
    pub email: email::EmailService,
    pub redis: redis::RedisService,
}

impl Services {
    /// Create all services with the given repository
    pub async fn new(
        repository: Repository,
        auth_config: AuthConfig,
        email_config: EmailConfig,
        redis_service: redis::RedisService,
    ) -> AppResult<Self> {
        Ok(Self {
            auth: auth::AuthService::new(repository.clone(), auth_config.clone(), redis_service.clone()),
            catalog: catalog::CatalogService::new(repository.clone()),
            users: auth::AuthService::new(repository.clone(), auth_config, redis_service.clone()),
            loans: loans::LoansService::new(repository.clone()),
            z3950: z3950::Z3950Service::new(repository.clone()),
            stats: stats::StatsService::new(repository.clone()),
            settings: settings::SettingsService::new(repository),
            email: email::EmailService::new(email_config),
            redis: redis_service,
        })
    }
}


