//! OpenAPI documentation

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::{auth, health, items, loans, settings, stats, users, z3950};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Elidune API",
        version = "1.0.0",
        description = "Library Management System REST API",
        license(name = "GPL-2.0", url = "https://www.gnu.org/licenses/gpl-2.0.html"),
        contact(name = "Elidune Team", email = "contact@elidune.org")
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    paths(
        // Health
        health::health_check,
        health::readiness_check,
        // Auth
        auth::login,
        auth::me,
        auth::verify_2fa,
        auth::verify_recovery,
        auth::setup_2fa,
        auth::disable_2fa,
        // Items
        items::list_items,
        items::get_item,
        items::create_item,
        items::update_item,
        items::delete_item,
        items::list_specimens,
        items::create_specimen,
        items::delete_specimen,
        // Users
        users::list_users,
        users::get_user,
        users::create_user,
        users::update_user,
        users::delete_user,
        users::update_my_profile,
        users::update_account_type,
        // Loans
        loans::get_user_loans,
        loans::create_loan,
        loans::return_loan,
        loans::renew_loan,
        // Z39.50
        z3950::search,
        z3950::import_record,
        // Stats
        stats::get_stats,
        stats::get_loan_stats,
        // Settings
        settings::get_settings,
        settings::update_settings,
    ),
    components(
        schemas(
            // Auth
            auth::LoginRequest,
            auth::LoginResponse,
            auth::UserInfo,
            auth::Verify2FARequest,
            auth::Verify2FAResponse,
            auth::VerifyRecoveryRequest,
            auth::Setup2FARequest,
            auth::Setup2FAResponse,
            // Items
            crate::models::item::Item,
            crate::models::item::ItemShort,
            crate::models::item::ItemQuery,
            crate::models::item::CreateItem,
            crate::models::item::UpdateItem,
            crate::models::item::Serie,
            crate::models::item::Collection,
            crate::models::item::Edition,
            crate::models::specimen::Specimen,
            crate::models::specimen::CreateSpecimen,
            crate::models::author::AuthorWithFunction,
            // Users
            crate::models::user::User,
            crate::models::user::UserShort,
            crate::models::user::UserQuery,
            crate::models::user::CreateUser,
            crate::models::user::UpdateUser,
            crate::models::user::UpdateProfile,
            crate::models::user::UpdateAccountType,
            // Loans
            loans::CreateLoanRequest,
            loans::LoanResponse,
            loans::ReturnResponse,
            crate::models::loan::LoanDetails,
            // Z39.50
            z3950::Z3950SearchQuery,
            z3950::Z3950SearchResponse,
            z3950::Z3950ImportRequest,
            // Stats
            stats::StatsResponse,
            stats::ItemStats,
            stats::UserStats,
            stats::LoanStats,
            stats::StatEntry,
            stats::Interval,
            stats::LoanStatsQuery,
            stats::LoanStatsResponse,
            stats::TimeSeriesEntry,
            // Settings
            settings::SettingsResponse,
            settings::LoanSettings,
            settings::Z3950ServerConfig,
            settings::UpdateSettingsRequest,
            // Health
            health::HealthResponse,
            // Errors
            crate::error::ErrorResponse,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "items", description = "Catalog item management"),
        (name = "users", description = "User management"),
        (name = "loans", description = "Loan management"),
        (name = "z3950", description = "Z39.50 catalog search"),
        (name = "stats", description = "Statistics"),
        (name = "settings", description = "System settings")
    )
)]
pub struct ApiDoc;

/// Create the OpenAPI documentation router
pub fn create_openapi_router() -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
}
