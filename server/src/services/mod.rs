//! Business logic services

pub mod catalog;
pub mod email;
pub mod loans;
pub mod redis;
pub mod settings;
pub mod users;
pub mod stats;
pub mod z3950;

use crate::{config::{UsersConfig, EmailConfig, RedisConfig}, error::AppResult, repository::Repository};

/// Container for all services
#[derive(Clone)]
pub struct Services {
    pub users: users::UsersService,
    pub catalog: catalog::CatalogService,
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
        auth_config: UsersConfig,
        email_config: EmailConfig,
        redis_config: RedisConfig,
        redis_service: redis::RedisService,
    ) -> AppResult<Self> {
        Ok(Self {
            catalog: catalog::CatalogService::new(repository.clone()),
            users: users::UsersService::new(repository.clone(), auth_config.clone(), redis_service.clone()),
            loans: loans::LoansService::new(repository.clone()),
            z3950: z3950::Z3950Service::new(repository.clone(), redis_service.clone(), redis_config.z3950_cache_ttl_seconds),
            stats: stats::StatsService::new(repository.clone()),
            settings: settings::SettingsService::new(repository),
            email: email::EmailService::new(email_config.clone()),
            redis: redis_service,
        })
    }
}


