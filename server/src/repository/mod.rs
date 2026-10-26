//! Repository layer for database operations.
//!
//! Each domain module defines its own `*Repository` trait next to the [`Repository`] inherent
//! methods (SQL) and a forwarding `impl *Repository for Repository`. The same pattern applies to
//! `settings` ([`RuntimeSettingsRepository`]), `z3950` ([`Z3950Repository`]), `library_info`
//! ([`LibraryInfoRepository`]), and `audit_log` ([`AuditLogRepository`]), not only older domains
//! like loans or biblios.

pub mod account_types;
pub mod acquisitions;
pub mod audit_log;
pub mod biblios;
pub mod catalog_entities;
pub mod circulation;
pub mod deposits;
pub mod email_outbox;
pub mod email_templates;
pub mod equipment;
pub mod events;
pub mod fines;
pub mod holds;
pub mod idempotency;
pub mod inventory;
pub mod library_info;
pub mod loans;
pub mod maintenance;
pub mod my_account;
pub mod public_types;
pub mod schedules;
pub mod serials;
pub mod settings;
pub mod sites;
pub mod stats;
pub mod transits;
pub mod users;
pub mod visitor_counts;
pub mod weeding;
pub mod withdrawals;
pub mod z3950;

pub use account_types::AccountTypesCatalogRepository;
pub use acquisitions::AcquisitionsRepository;
pub use audit_log::AuditLogRepository;
pub use biblios::BibliosRepository;
pub use catalog_entities::CatalogEntitiesRepository;
pub use email_templates::{EmailTemplateRow, EmailTemplatesRepository};
pub use equipment::EquipmentRepository;
pub use events::{EventsRepository, EventsServiceRepository};
pub use fines::FinesRepository;
pub use holds::HoldsRepository;
pub use inventory::InventoryRepository;
pub use library_info::{LibraryInfoRepository, LibraryInfoSnapshot};
pub use loans::{LoansRepository, LoansServiceRepository};
pub use maintenance::MaintenanceRepository;
pub use public_types::PublicTypesRepository;
pub use schedules::SchedulesRepository;
pub use serials::SerialsRepository;
pub use settings::RuntimeSettingsRepository;
pub use sites::SitesRepository;
pub use transits::TransitsRepository;
pub use users::UsersRepository;
pub use visitor_counts::VisitorCountsRepository;
pub use z3950::{Z3950Repository, Z3950ServerRecord};

use std::sync::Arc;

use sqlx::{Pool, Postgres};

use crate::dynamic_config::DynamicConfig;

/// Main repository struct holding database connection pool.
/// Methods are split across domain modules (items, loans, users, etc.) via separate `impl Repository` blocks.
#[derive(Clone)]
pub struct Repository {
    pub(crate) pool: Pool<Postgres>,
    /// When set, loan return uses live `holds.ready_expiry_days` from dynamic config.
    pub(crate) dynamic_config: Option<Arc<DynamicConfig>>,
}

impl Repository {
    /// Create a new repository with the given database pool.
    /// Pass `dynamic_config` so hold pickup windows follow TOML / DB settings.
    pub fn new(pool: Pool<Postgres>, dynamic_config: Option<Arc<DynamicConfig>>) -> Self {
        Self {
            pool,
            dynamic_config,
        }
    }

    /// Days until a `ready` hold expires (`expires_at`), from config or default **7** when no dynamic config.
    pub(crate) fn hold_ready_expiry_days(&self) -> i32 {
        self.dynamic_config
            .as_ref()
            .map(|dc| dc.read_holds().ready_expiry_days as i32)
            .unwrap_or_else(|| crate::config::HoldsConfig::default().ready_expiry_days as i32)
    }

    /// Whether checkout/renew should push due dates off closed days (default **true**).
    pub(crate) fn skip_closed_days(&self) -> bool {
        self.dynamic_config
            .as_ref()
            .map(|dc| dc.read_circulation().skip_closed_days)
            .unwrap_or_else(|| crate::config::CirculationConfig::default().skip_closed_days)
    }

    /// `reminders.accrue_fines`. False skips overdue fine accrual blocking only.
    /// Stored global and public-type thresholds are left as written.
    /// Missing dynamic config uses the file default (`true`).
    pub(crate) fn accrue_fines_enabled(&self) -> bool {
        self.dynamic_config
            .as_ref()
            .map(|dc| dc.read_reminders().accrue_fines)
            .unwrap_or_else(|| crate::config::RemindersConfig::default().accrue_fines)
    }

    /// Library default site (`sites."default"`), or none when that flag is unset.
    ///
    /// A NULL `loans.site_id` resolves to this at query time. The subquery is
    /// [`crate::services::calendar::LIBRARY_DEFAULT_SITE_ID_SQL`].
    pub(crate) async fn library_default_site_id_conn(
        &self,
        conn: &mut sqlx::PgConnection,
    ) -> crate::error::AppResult<Option<i64>> {
        let id: Option<i64> = sqlx::query_scalar(&format!(
            "SELECT id FROM sites WHERE id = {}",
            crate::services::calendar::LIBRARY_DEFAULT_SITE_ID_SQL
        ))
        .fetch_optional(&mut *conn)
        .await?;
        Ok(id)
    }

    /// `anchor + duration_days`, then the next open day of `site_id` when
    /// [`Self::skip_closed_days`] is on. The civil date is the library timezone,
    /// not the container zone. `site_id` is [`crate::services::calendar::loan_calendar_site`]:
    /// `loans.site_id`, or the library default site when the loan has none.
    ///
    /// `conn` is the caller's connection. Checkout and renew pass the transaction
    /// that holds `FOR UPDATE` so calendar reads do not take a second pool connection.
    pub(crate) async fn loans_due_at(
        &self,
        conn: &mut sqlx::PgConnection,
        anchor: chrono::DateTime<chrono::Utc>,
        duration_days: i16,
        site_id: Option<i64>,
    ) -> crate::error::AppResult<chrono::DateTime<chrono::Utc>> {
        let candidate = crate::circulation_calendar::raw_due_date(anchor, duration_days);
        if !self.skip_closed_days() {
            return Ok(candidate);
        }
        let zone = self.library_zone_conn(conn).await?;
        let start = crate::services::calendar::local_date(zone, candidate);
        let end = start + chrono::Duration::days(crate::circulation_calendar::MAX_LOOKAHEAD_DAYS);
        let calendar = self
            .schedules_opening_calendar_conn(conn, start, end)
            .await?;
        Ok(crate::services::calendar::slide_due_date(
            candidate, zone, site_id, &calendar,
        ))
    }

    /// Expose the underlying pool for callers that need to begin transactions directly.
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}

#[async_trait::async_trait]
impl crate::services::calendar::OpeningDays for Repository {
    async fn zone(&self) -> crate::error::AppResult<chrono_tz::Tz> {
        self.library_zone().await
    }

    async fn opening_between(
        &self,
        from: chrono::NaiveDate,
        to: chrono::NaiveDate,
    ) -> crate::error::AppResult<crate::circulation_calendar::OpeningCalendar> {
        self.schedules_opening_calendar(from, to).await
    }
}
