//! OpenAPI documentation

use axum::Router;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

use crate::api::{
    account_types, acquisitions, admin_config, audit, auth, batch, biblios, collections, deposits, email_templates, equipment, events, fines, first_setup, genres, health, holds, inventory,
    items, library_info, loans, maintenance, metrics, opac, public_types, schedules, series, sites, stats, suggestions, tasks, transits, users, visitor_counts, withdrawals, z3950,
};

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
        health::version,
        metrics::metrics,
        first_setup::post_first_setup,
        // Auth
        auth::login,
        auth::me,
        auth::clear_auth_rate_limit,
        auth::verify_2fa,
        auth::verify_recovery,
        auth::request_password_reset,
        auth::reset_password,
        auth::setup_2fa,
        auth::disable_2fa,
        // Biblios and physical items
        biblios::list_biblios,
        biblios::get_biblio,
        biblios::create_biblio,
        biblios::load_marc,
        biblios::import_marc_batch,
        biblios::scan_deposit_marc_duplicates,
        biblios::list_marc_batches,
        biblios::load_marc_batch,
        biblios::update_biblio,
        biblios::delete_biblio,
        biblios::list_items,
        biblios::create_item,
        items::get_biblio_by_item,
        items::get_biblio_by_barcode,
        items::update_item,
        items::delete_item,
        items::set_item_weeding,
        withdrawals::withdraw_item,
        withdrawals::get_active_item_withdrawal,
        withdrawals::get_withdrawal,
        withdrawals::batch_withdraw,
        withdrawals::cancel_withdrawal,
        withdrawals::dispose_withdrawals,
        withdrawals::complete_legacy_withdrawal,
        withdrawals::list_withdrawals,
        withdrawals::list_weeding_candidates,
        withdrawals::export_withdrawals_csv,
        // Users
        users::list_users,
        users::get_user_by_card,
        users::get_user,
        users::create_user,
        users::update_user,
        users::delete_user,
        users::report_card_lost,
        users::replace_card,
        users::update_my_profile,
        users::update_account_type,
        // Loans
        loans::get_user_loans,
        loans::list_my_loans,
        loans::export_user_loans_marc,
        loans::create_loan,
        batch::batch_create_loans,
        loans::get_loan_borrower,
        loans::return_loan,
        loans::renew_loan,
        loans::return_loan_by_item,
        loans::resolve_lost_found,
        loans::renew_loan_by_item,
        loans::get_overdue_loans,
        loans::send_overdue_reminders,
        loans::get_loan_settings,
        loans::update_loan_settings,
        loans::mark_loan_lost,
        loans::mark_loan_damaged,
        loans::mark_loan_claimed_returned,
        loans::resolve_loan_claims_returned,
        loans::list_claims_returned,
        // Fines
        fines::list_user_fines,
        fines::pay_fine,
        fines::waive_fine,
        fines::list_fine_rules,
        fines::upsert_fine_rule,
        fines::get_fine_policy,
        fines::update_fine_policy,
        fines::accrue_loan_fine,
        fines::accrue_user_fines,
        fines::accrue_overdue_fines,
        // Holds
        holds::list_holds,
        holds::list_my_holds,
        holds::list_pick_list,
        holds::pick_not_found,
        holds::create_hold,
        holds::get_holds_policy,
        holds::update_holds_policy,
        holds::get_hold_quota,
        holds::list_holds_for_item,
        holds::list_holds_for_biblio,
        holds::list_holds_for_user,
        holds::cancel_hold,
        transits::list_transits,
        transits::get_transit,
        transits::get_hold_transit,
        transits::get_item_transit,
        transits::create_hold_transit,
        transits::ship_transit,
        transits::receive_transit,
        transits::cancel_transit,
        // Inventory (stocktaking)
        inventory::list_sessions,
        inventory::create_session,
        inventory::get_session,
        inventory::close_session,
        inventory::scan_barcode,
        inventory::batch_scan,
        inventory::list_scans,
        inventory::list_missing,
        inventory::get_report,
        inventory::consolidation_preview,
        inventory::consolidate_session,
        // Z39.50
        z3950::search,
        z3950::import_record,
        z3950::get_z3950_servers,
        z3950::update_z3950_servers,
        // Stats
        stats::get_stats,
        stats::get_loan_stats,
        stats::get_user_stats,
        stats::get_catalog_stats,
        stats::get_stats_schema,
        stats::post_stats_query,
        stats::list_saved_queries,
        stats::create_saved_query,
        stats::update_saved_query,
        stats::delete_saved_query,
        stats::run_saved_query,
        // Library info
        library_info::get_library_info,
        library_info::update_library_info,
        // Email templates (settings)
        email_templates::list_email_templates,
        email_templates::get_email_template,
        email_templates::update_email_template,
        // Visitor counts
        visitor_counts::list_visitor_counts,
        visitor_counts::create_visitor_count,
        visitor_counts::delete_visitor_count,
        // Schedules
        schedules::list_periods,
        schedules::create_period,
        schedules::update_period,
        schedules::delete_period,
        schedules::list_slots,
        schedules::create_slot,
        schedules::delete_slot,
        schedules::list_closures,
        schedules::create_closure,
        schedules::delete_closure,
        // Series
        series::list_series,
        series::get_serie,
        series::get_serie_biblios,
        series::create_serie,
        series::update_serie,
        series::delete_serie,
        // Collections
        collections::list_collections,
        collections::get_collection,
        collections::get_collection_biblios,
        collections::create_collection,
        collections::update_collection,
        collections::delete_collection,
        // Sources
        sites::list_sites,
        sites::create_site,
        sites::get_site,
        sites::update_site,
        sites::archive_site,
        sites::merge_sites,
        // Lenders and incoming deposit batches
        deposits::list_lenders,
        deposits::create_lender,
        deposits::update_lender,
        deposits::archive_lender,
        deposits::list_deposit_losses,
        deposits::mark_deposit_loss_notified,
        deposits::list_deposit_batches,
        deposits::get_deposit_batch,
        deposits::create_deposit_batch,
        deposits::update_deposit_batch,
        deposits::list_deposit_batch_items,
        deposits::start_deposit_return,
        deposits::cancel_deposit_return,
        deposits::scan_out_deposit_copy,
        deposits::scan_out_deposit_copy_global,
        deposits::close_deposit_batch,
        // Equipment
        equipment::list_equipment,
        equipment::get_equipment,
        equipment::create_equipment,
        equipment::update_equipment,
        equipment::delete_equipment,
        // Events
        events::list_events,
        events::get_event,
        events::create_event,
        events::update_event,
        events::delete_event,
        events::send_event_announcement,
        events::count_announcement_recipients,
        events::unsubscribe_events,
        // Acquisitions
        acquisitions::list_vendors,
        acquisitions::get_vendor,
        acquisitions::create_vendor,
        acquisitions::update_vendor,
        acquisitions::archive_vendor,
        acquisitions::list_funds,
        acquisitions::get_fund,
        acquisitions::create_fund,
        acquisitions::update_fund,
        acquisitions::list_orders,
        acquisitions::get_order,
        acquisitions::create_order,
        acquisitions::update_order,
        acquisitions::add_order_line,
        acquisitions::update_order_line,
        acquisitions::delete_order_line,
        acquisitions::submit_order,
        acquisitions::cancel_order,
        acquisitions::receive_order,
        acquisitions::order_audit,
        suggestions::create_suggestion,
        suggestions::list_suggestions,
        suggestions::get_suggestion,
        suggestions::accept_suggestion,
        suggestions::refuse_suggestion,
        // Library account types (roles / rights)
        account_types::list_account_types,
        account_types::get_account_type,
        account_types::create_account_type,
        account_types::update_account_type,
        account_types::delete_account_type,
        // Admin config
        admin_config::get_config,
        admin_config::update_config_section,
        admin_config::reset_config_section,
        admin_config::test_email,
        // Maintenance
        maintenance::run_maintenance,
        maintenance::dump_database,
        maintenance::restore_database,
        // Background tasks
        tasks::list_tasks,
        tasks::get_task,
        // Audit
        audit::get_audit_log,
        audit::export_audit_log,
        // Public types
        public_types::list_public_types,
        public_types::get_public_type,
        public_types::create_public_type,
        public_types::update_public_type,
        public_types::delete_public_type,
        public_types::update_loan_settings,
        // Genres
        genres::list_genres,
        genres::reload_genres,
        // Opac
        opac::opac_search,
        opac::opac_get_biblio,
        opac::opac_availability,
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
            auth::RequestPasswordResetRequest,
            auth::RequestPasswordResetResponse,
            auth::ResetPasswordRequest,
            auth::ResetPasswordResponse,
            auth::Setup2FARequest,
            auth::Setup2FAResponse,
            auth::Disable2FARequest,
            // Biblios (bibliographic records)
            crate::models::biblio::Biblio,
            crate::models::biblio::BiblioShort,
            crate::models::biblio::BiblioQuery,
            crate::models::biblio::Serie,
            crate::models::biblio::Collection,
            crate::models::biblio::Edition,
            crate::models::biblio::CreateSerie,
            crate::models::biblio::UpdateSerie,
            crate::models::biblio::SerieQuery,
            crate::models::biblio::CreateCollection,
            crate::models::biblio::UpdateCollection,
            crate::models::biblio::CollectionQuery,
            series::PaginatedSeries,
            collections::PaginatedCollections,
            // Items (physical copies)
            crate::models::item::Item,
            crate::models::item::ItemShort,
            crate::models::item::ItemAvailability,
            crate::models::item::WeedingStatus,
            crate::models::catalog_view::ItemPublicAvailability,
            crate::models::catalog_view::ItemStaffView,
            crate::models::catalog_view::ItemPublicView,
            crate::models::catalog_view::ItemShortStaff,
            crate::models::catalog_view::ItemShortPublic,
            crate::models::catalog_view::BiblioStaffView,
            crate::models::catalog_view::BiblioPublicView,
            crate::models::catalog_view::BiblioShortStaff,
            crate::models::catalog_view::BiblioShortPublic,
            crate::models::catalog_view::AuthorPublic,
            crate::models::catalog_view::UserShortStaff,
            crate::models::catalog_view::UserShortPublic,
            crate::models::catalog_view::HoldDetailsStaff,
            crate::models::catalog_view::HoldListStaff,
            crate::models::catalog_view::HoldDetailsPublic,
            crate::models::catalog_view::LoanDetailsStaff,
            crate::models::catalog_view::LoanDetailsPublic,
            crate::models::catalog_view::LoanDetailsOwn,
            biblios::BiblioReadResponse,
            biblios::BiblioSearchResponse,
            biblios::ItemListResponse,
            biblios::BiblioShortListResponse,
            holds::HoldListResponse,
            loans::UserLoanListResponse,
            loans::LoanBorrowerResponse,
            crate::models::dto::patron_card::CardLookupResponse,
            crate::models::dto::patron_card::DeskCardLookup,
            crate::models::dto::patron_card::StaffCardLookup,
            crate::models::dto::patron_card::PatronLoanStatus,
            crate::patron_eligibility::PatronBlockReason,
            loans::LoanBorrowerMinimized,
            loans::LoanDetailsView,
            loans::OverdueLoansView,
            loans::OverdueLoanView,
            loans::OverdueBorrower,
            loans::OverdueBorrowerFull,
            crate::models::my_account::MyLoanView,
            crate::renewal::RenewalRefusal,
            crate::models::my_account::MyHoldView,
            crate::models::my_account::MyHoldStatus,
            biblios::PaginatedResponse<crate::models::my_account::MyLoanView>,
            biblios::PaginatedResponse<crate::models::my_account::MyHoldView>,
            crate::models::dto::weeding::SetWeedingRequest,
            crate::models::dto::weeding::WeedingCandidate,
            crate::models::dto::weeding::WeedingCandidateList,
            crate::models::dto::weeding::WeedingCandidateQuery,
            crate::models::withdrawal::WithdrawalReason,
            crate::models::withdrawal::WithdrawalDestination,
            crate::models::withdrawal::WithdrawalQuery,
            crate::models::withdrawal::WithdrawalWrite,
            crate::models::withdrawal::BatchWithdrawalRequest,
            crate::models::withdrawal::BatchDisposeRequest,
            crate::models::withdrawal::WithdrawalDisposeFailure,
            crate::models::withdrawal::WithdrawalDisposeRejectedBody,
            crate::models::withdrawal::ItemWithdrawal,
            crate::models::withdrawal::ReasonTotal,
            crate::models::withdrawal::DestinationTotal,
            crate::models::withdrawal::WithdrawalList,
            crate::models::withdrawal::BatchWithdrawalResponse,
            // Pagination
            biblios::PaginatedResponse<crate::models::biblio::BiblioShort>,
            biblios::PaginatedResponse<crate::models::user::UserShort>,
            biblios::PaginatedResponse<crate::models::loan::LoanDetails>,
            // Users
            crate::models::user::User,
            crate::models::user::UserShort,
            crate::models::user::UserQuery,
            crate::models::user::UserPayload,
            crate::models::user::UserNoticeLanguage,
            crate::models::dto::card::CardLostReason,
            crate::models::dto::card::ReportCardLostRequest,
            crate::models::dto::card::ReportCardLostResponse,
            crate::models::dto::card::ReportCardLostStaffBody,
            crate::models::dto::card::ReportCardLostDeskBody,
            crate::models::dto::card::ReplaceCardRequest,
            crate::models::dto::card::ReplaceCardResponse,
            crate::models::user::UpdateProfile,
            crate::models::user::UpdateAccountType,
            crate::models::account_type::AccountTypeDefinition,
            crate::models::account_type::CreateAccountType,
            crate::models::account_type::UpdateAccountTypeDefinition,
            // Loans
            loans::CreateLoanRequest,
            loans::LoanResponse,
            loans::ReturnResponse,
            crate::models::dto::circulation::DeskNotice,
            crate::models::dto::circulation::DeskNoticeLevel,
            loans::OverdueLoansQuery,
            crate::models::fine::Fine,
            crate::models::fine::FineRule,
            crate::models::fine::FineStatus,
            crate::models::fine::FineChargeType,
            crate::models::circulation::CirculationStatus,
            crate::models::circulation::DamageDisposition,
            crate::models::circulation::ClaimsResolveOutcome,
            crate::models::dto::circulation::CirculationExceptionOutcome,
            crate::models::dto::circulation::CirculationExceptionResponse,
            crate::models::dto::circulation::ClaimsReturnedQueueItem,
            crate::models::dto::circulation::MarkLostRequest,
            crate::models::dto::circulation::MarkDamagedRequest,
            crate::models::dto::circulation::MarkClaimedReturnedRequest,
            crate::models::dto::circulation::ResolveClaimsReturnedRequest,
            crate::models::dto::circulation::LostFineDeskStatus,
            crate::models::dto::circulation::LostFoundDisposition,
            crate::models::dto::circulation::LostFineAction,
            crate::models::dto::circulation::LostItemOnReturn,
            crate::models::dto::circulation::ResolveLostFoundRequest,
            crate::models::dto::circulation::LostFoundResolution,
            biblios::PaginatedResponse<crate::models::dto::circulation::ClaimsReturnedQueueItem>,
            crate::models::fine::PayFineRequest,
            crate::models::fine::WaiveFineRequest,
            fines::UpsertFineRuleRequest,
            fines::UnpaidFinesSummary,
            crate::models::dto::fines::AccrueOutcome,
            crate::models::dto::fines::AccrualBreakdown,
            crate::models::dto::fines::AccrueLoanResult,
            crate::models::dto::fines::AccrueBatchReport,
            crate::models::dto::fines::AccrueBatchError,
            crate::models::dto::fines::AccrueFinesRequest,
            crate::models::dto::fines::CirculationFinePolicy,
            crate::models::dto::fines::UpdateCirculationFinePolicy,
            crate::models::fine::GlobalUnpaidThresholdMode,
            crate::models::fine::AudienceUnpaidThresholdMode,
            // Holds
            crate::models::hold::Hold,
            crate::models::hold::HoldPublic,
            crate::models::hold::HoldDetails,
            crate::models::dto::holds::HoldsPolicy,
            crate::models::dto::holds::HoldQuota,
            holds::CreateHoldRequest,
            holds::HoldWriteResponse,
            holds::ListHoldsQuery,
            holds::PickListQuery,
            crate::models::dto::holds::HoldPickListItem,
            holds::HoldQuotaQuery,
            crate::models::transit::ItemTransit,
            crate::models::transit::TransitPatron,
            crate::models::transit::TransitStatus,
            crate::models::transit::CreateTransit,
            crate::models::transit::TransitActionRequest,
            crate::models::transit::TransitWithHold,
            crate::models::dto::transits::ListTransitsQuery,
            biblios::PaginatedResponse<crate::models::transit::ItemTransit>,
            biblios::PaginatedResponse<crate::models::hold::HoldDetails>,
            biblios::PaginatedResponse<crate::models::inventory::InventorySession>,
            biblios::PaginatedResponse<crate::models::inventory::InventoryScan>,
            biblios::PaginatedResponse<crate::models::inventory::InventoryMissingRow>,
            crate::models::inventory::InventorySession,
            crate::models::inventory::InventoryScan,
            crate::models::inventory::InventoryScanResult,
            crate::models::inventory::InventoryStatus,
            crate::models::inventory::InventoryReport,
            crate::models::inventory::InventoryMissingRow,
            crate::models::inventory::CreateInventorySession,
            crate::models::inventory::CreateInventorySessionResponse,
            crate::models::inventory::ScanBarcode,
            crate::models::inventory::BatchScanBarcodes,
            crate::models::inventory::ConsolidateInventorySession,
            crate::models::inventory::InventoryConsolidationResult,
            crate::models::inventory::InventoryConsolidationSkipped,
            crate::models::inventory::InventoryConsolidationPreview,
            crate::models::inventory::InventoryConsolidationPreviewSummary,
            crate::models::inventory::InventoryConsolidationPreviewRow,
            crate::models::inventory::InventoryConsolidationPreviewLoan,
            crate::models::inventory::InventoryConsolidationEmailError,
            inventory::ListInventorySessionsQuery,
            inventory::ListInventoryPageQuery,
            loans::GetUserLoansQuery,
            loans::ExportUserLoansMarcQuery,
            crate::models::loan::LoanMarcExportFormat,
            crate::models::loan::LoanMarcExportEncoding,
            loans::SendRemindersQuery,
            crate::models::loan::LoanDetails,
            crate::models::loan::LoanClosureReason,
            crate::services::reminders::ReminderReport,
            crate::services::reminders::ReminderDetail,
            crate::services::reminders::ReminderError,
            crate::services::reminders::ReminderTier,
            // Z39.50
            z3950::Z3950SearchQuery,
            z3950::Z3950SearchResponse,
            z3950::Z3950ImportRequest,
            z3950::Z3950ImportResponse,
            z3950::ImportItem,
            // Import report
            crate::models::import_report::ImportReport,
            crate::models::import_report::ImportAction,
            crate::models::import_report::DuplicateConfirmationRequired,
            crate::models::import_report::DuplicateItemBarcodeRequired,
            // Biblios
            biblios::CreateBiblioResponse,
            // Stats
            stats::StatsResponse,
            stats::StatsQuery,
            stats::LoanStatsResponse,
            stats::Interval,
            stats::LoanStatsQuery,
            stats::TimeSeriesEntry,
            stats::UserStatsAggregate,
            stats::UserStatsQuery,
            stats::CatalogStatsQuery,
            stats::CatalogStatsResponse,
            stats::CatalogStatsTotals,
            stats::CatalogSourceStats,
            stats::CatalogBreakdownStats,
            crate::models::stats_builder::StatsBuilderBody,
            crate::models::stats_builder::SelectField,
            crate::models::stats_builder::GroupByField,
            crate::models::stats_builder::StatsFilter,
            crate::models::stats_builder::HavingFilter,
            crate::models::stats_builder::FilterOperator,
            crate::models::stats_builder::StatsAggregation,
            crate::models::stats_builder::AggregateFunction,
            crate::models::stats_builder::TimeBucket,
            crate::models::stats_builder::TimeGranularity,
            crate::models::stats_builder::StatsOrderBy,
            crate::models::stats_builder::SortDirection,
            crate::models::stats_builder::StatsTableResponse,
            crate::models::stats_builder::ColumnMeta,
            crate::models::stats_builder::SavedStatsQuery,
            crate::models::stats_builder::SavedStatsQueryWrite,
            // Library info
            library_info::LibraryInfo,
            library_info::LibraryUiLanguage,
            library_info::UpdateLibraryInfoRequest,
            crate::features::EnabledFeatures,
            crate::features::FeaturesConfig,
            // Email templates
            email_templates::EmailTemplate,
            email_templates::UpdateEmailTemplateRequest,
            loans::LoanSettings,
            loans::UpdateLoanSettingsRequest,
            z3950::Z3950ServerConfig,
            z3950::UpdateZ3950ServersRequest,
            // Visitor counts
            crate::models::visitor_count::VisitorCount,
            crate::models::visitor_count::CreateVisitorCount,
            crate::models::visitor_count::VisitorCountQuery,
            // Schedules
            crate::models::schedule::SchedulePeriod,
            crate::models::schedule::ScheduleSlot,
            crate::models::schedule::ScheduleClosure,
            crate::models::schedule::CreateSchedulePeriod,
            crate::models::schedule::UpdateSchedulePeriod,
            crate::models::schedule::CreateScheduleSlot,
            crate::models::schedule::CreateScheduleClosure,
            crate::models::schedule::ScheduleClosureQuery,
            // Sources
            crate::models::site::Site,
            crate::models::site::CreateSite,
            crate::models::site::UpdateSite,
            crate::models::site::MergeSites,
            sites::SitesQuery,
            // Incoming deposits
            deposits::LendersQuery,
            crate::models::deposit::Lender,
            crate::models::deposit::CreateLender,
            crate::models::deposit::UpdateLender,
            crate::models::deposit::DepositBatch,
            crate::models::deposit::DepositBatchDetail,
            crate::models::deposit::DepositBatchCounts,
            crate::models::deposit::CreateDepositBatch,
            crate::models::deposit::UpdateDepositBatch,
            crate::models::deposit::DepositBatchListQuery,
            crate::models::deposit::DepositBatchItemsQuery,
            crate::models::deposit::DepositBatchItem,
            crate::models::deposit::DepositBatchStatus,
            crate::models::deposit::DepositCopyState,
            crate::models::deposit::ItemDeposit,
            crate::models::deposit::DepositScanOutRequest,
            crate::models::deposit::DepositScanOut,
            crate::models::deposit::DepositGlobalScanOut,
            crate::models::deposit::DepositCloseReport,
            crate::models::deposit::DepositCloseCopy,
            crate::models::deposit::DepositActiveLoanCopy,
            crate::models::deposit::DepositLoss,
            crate::models::deposit::DepositLossKind,
            crate::models::deposit::DepositLossListQuery,
            crate::models::deposit::MarkDepositLossNotified,
            crate::models::dto::stats::DepositLenderCount,
            crate::services::marc::MarcBatchImportReport,
            crate::services::marc::MarcBatchImportError,
            crate::services::marc::MarcBatchAttached,
            crate::models::dto::deposit_import::DepositDuplicateScanReport,
            crate::models::dto::deposit_import::DepositDuplicateConflict,
            crate::models::dto::deposit_import::DepositDuplicateConflictKind,
            crate::models::dto::deposit_import::DepositDuplicateAction,
            // Equipment
            crate::models::equipment::Equipment,
            crate::models::equipment::CreateEquipment,
            crate::models::equipment::UpdateEquipment,
            // Events
            crate::models::event::Event,
            crate::models::event::EventAttachmentInput,
            crate::models::event::CreateEvent,
            crate::models::event::UpdateEvent,
            crate::models::event::EventQuery,
            events::EventsListResponse,
            crate::services::events::SendAnnouncementRequest,
            crate::services::events::AnnouncementReport,
            crate::api::events::AnnouncementRecipientCount,
            crate::api::events::UnsubscribeEventsRequest,
            crate::models::user::EventsConsentSource,
            crate::services::events::AnnouncementError,
            // Acquisitions
            crate::models::acquisition::Vendor,
            crate::models::acquisition::CreateVendor,
            crate::models::acquisition::UpdateVendor,
            crate::models::acquisition::VendorQuery,
            crate::models::acquisition::VendorListResponse,
            crate::models::acquisition::AcquisitionFund,
            crate::models::acquisition::CreateFund,
            crate::models::acquisition::UpdateFund,
            crate::models::acquisition::FundQuery,
            crate::models::acquisition::FundListResponse,
            crate::models::acquisition::PurchaseOrderStatus,
            crate::models::acquisition::PurchaseOrder,
            crate::models::acquisition::PurchaseOrderLine,
            crate::models::acquisition::PurchaseOrderDetail,
            crate::models::acquisition::CreatePurchaseOrder,
            crate::models::acquisition::UpdatePurchaseOrder,
            crate::models::acquisition::CreateOrderLine,
            crate::models::acquisition::UpdateOrderLine,
            crate::models::acquisition::PurchaseOrderQuery,
            crate::models::acquisition::PurchaseOrderListResponse,
            crate::models::acquisition::ReceivePurchaseOrder,
            crate::models::acquisition::ReceiveOrderLine,
            crate::models::acquisition::ReceiveItemSpec,
            crate::models::acquisition::ReceivePurchaseOrderResult,
            crate::models::acquisition::Receipt,
            crate::models::acquisition::ReceiptLineResult,
            crate::models::acquisition::PurchaseSuggestionStatus,
            crate::models::acquisition::PurchaseSuggestion,
            crate::models::acquisition::CreatePurchaseSuggestion,
            crate::models::acquisition::ReviewPurchaseSuggestion,
            crate::models::acquisition::PurchaseSuggestionQuery,
            crate::models::acquisition::PurchaseSuggestionListResponse,
            // Admin config
            admin_config::ConfigResponse,
            admin_config::ConfigSectionInfo,
            admin_config::UpdateConfigSectionRequest,
            admin_config::TestEmailRequest,
            // Maintenance
            maintenance::MaintenanceRequest,
            maintenance::MaintenanceAction,
            maintenance::MaintenanceActionReport,
            maintenance::MaintenanceResponse,
            maintenance::MaintenanceTaskProgress,
            maintenance::CatalogZ3950RefreshProgress,
            maintenance::CatalogZ3950RefreshProgressStatus,
            maintenance::CatalogZ3950RefreshResult,
            // Background tasks
            tasks::TaskAcceptedResponse,
            crate::models::task::BackgroundTask,
            crate::models::task::TaskKind,
            crate::models::task::TaskStatus,
            crate::models::task::TaskProgress,
            // Audit
            audit::AuditQueryRequest,
            audit::AuditExportRequest,
            crate::models::audit::AuditLogPage,
            crate::models::audit::AuditLogEntry,
            // Public types
            crate::services::genre_catalog::GenreApiRow,
            crate::models::Genre,
            crate::models::public_type::PublicType,
            crate::models::loan::LoanSettingsRenewAt,
            crate::models::public_type::PublicTypeLoanSettings,
            crate::models::public_type::CreatePublicType,
            crate::models::public_type::UpdatePublicType,
            crate::models::public_type::PublicTypeLoanSettingInput,
            crate::models::public_type::ReplacePublicTypeLoanSettingsRequest,
            crate::repository::events::EventAnnualStats,
            crate::repository::events::EventTypeStats,
            // Health
            health::HealthResponse,
            health::HealthDatabaseStatus,
            health::HealthSetupStatus,
            health::VersionResponse,
            first_setup::FirstSetupRequest,
            first_setup::FirstSetupAdminBody,
            first_setup::FirstSetupEmailBody,
            first_setup::FirstSetupResponse,

            // Errors
            crate::error::ErrorResponse,
            crate::error::PaymentExceedsBalanceBody,
            crate::error::DepositBatchHasActiveLoansBody,
            crate::error::DepositItemOnLoanBody,
            crate::checkout_refusal::CheckoutRefusalBody,
            crate::checkout_refusal::CheckoutRefusalDetails,
            crate::checkout_refusal::ItemHeldForOtherDetails,
            crate::error::ItemToPickDetails,
            crate::renewal::RenewalRefusalBody,
        )
    ),
    tags(
        (name = "health", description = "Health / readiness, server version, and one-time POST /first_setup when the database has no users and no settings overrides"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "biblios", description = "Bibliographic record management"),
        (name = "items", description = "Physical copies (items) — get biblio for a copy, update/delete by item id"),
        (name = "users", description = "User management"),
        (name = "loans", description = "Loan management"),
        (name = "fines", description = "Overdue fines: list, accrue, pay, waive, and rules"),
        (name = "holds", description = "Hold queue: copy-level (itemId) and title-level (biblioId)"),
        (name = "transits", description = "Inter-site item transit for hold fulfillment"),
        (name = "inventory", description = "Stocktaking (inventory) sessions and barcode scans"),
        (name = "z3950", description = "Z39.50 catalog search"),
        (name = "stats", description = "Statistics"),
        (name = "visitor_counts", description = "Visitor counting"),
        (name = "schedules", description = "Library schedules (hours, closures)"),
        (name = "sites", description = "Physical catalog site management"),
        (name = "deposits", description = "Lenders and incoming deposit batches (copies lent to this library). Own stock has a null deposit block."),
        (name = "equipment", description = "Library equipment management"),
        (name = "events", description = "Cultural events and school visits"),
        (name = "acquisitions", description = "Vendors, yearly funds, purchase orders, and receipt (creates catalog items only at receipt)"),
        (name = "suggestions", description = "Patron purchase suggestions: propose a title; staff accept opens a draft order intent (no biblio, no item)"),
        (name = "account_types", description = "Library account types (built-in and custom) and per-domain rights. Settings write is required."),
        (name = "library_info", description = "Library global information (name, address, phones, email, default UI language)"),
        (name = "email_templates", description = "Editable email templates exposed to the Settings UI"),
        (name = "series", description = "Series management"),
        (name = "collections", description = "Collections management"),
        (name = "public_types", description = "Borrower public types (child, adult, school, staff, senior)"),
        (name = "genres", description = "Bibliographic genre catalog (shelf codes, labels, MARC synonyms); edit via SQL then POST /genres/reload"),
        (name = "admin", description = "Admin runtime configuration"),
        (name = "audit", description = "Audit log"),
        (name = "maintenance", description = "Data-quality maintenance operations (admin only)"),
        (name = "tasks", description = "Background task status polling")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("Bearer authentication using JWT. Use 'Authorization: Bearer <token>'".to_string()))
                        .build(),
                ),
            );
        }
    }
}

/// Create the OpenAPI documentation router
pub fn create_openapi_router() -> Router {
    Router::new().merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

#[cfg(test)]
mod tests {
    use super::ApiDoc;
    use utoipa::OpenApi;

    #[test]
    fn openapi_schema_compiles() {
        let doc = ApiDoc::openapi();
        assert!(!doc.paths.paths.is_empty());
    }

    #[test]
    fn public_item_schema_omits_due_date_while_staff_and_my_loans_keep_it() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let schemas = &value["components"]["schemas"];
        let public_props = &schemas["ItemPublicView"]["properties"];
        assert!(public_props.get("dueDate").is_none(), "ItemPublicView must not declare dueDate: {public_props}");
        assert!(schemas["ItemStaffView"]["properties"]["dueDate"].is_object(), "ItemStaffView keeps dueDate");
        assert!(schemas["MyLoanView"]["properties"]["dueDate"].is_object(), "MyLoanView keeps dueDate");
        let opac = value["paths"]["/opac/biblios/{id}"]["get"]["responses"]["200"]["description"].as_str().unwrap_or("");
        assert!(opac.contains("omits `dueDate`"), "OPAC docs must say the key is omitted: {opac}");
    }

    #[test]
    fn card_lost_and_replace_routes_are_documented() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        for path in ["/users/{id}/card-lost", "/users/{id}/replace-card"] {
            assert!(value["paths"][path]["post"].is_object(), "{path} must be documented");
        }
        let schemas = &value["components"]["schemas"];
        assert!(schemas["ReportCardLostRequest"].is_object());
        assert!(schemas["ReplaceCardRequest"].is_object());
        assert!(schemas["ReportCardLostResponse"].is_object());
        assert!(schemas["ReportCardLostStaffBody"].is_object());
        assert!(schemas["ReportCardLostDeskBody"].is_object());
        assert!(schemas["ReplaceCardResponse"].is_object());
        assert!(schemas["ErrorResponse"].is_object());
    }

    #[test]
    fn return_by_barcode_path_param_is_named_barcode() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let path = "/loans/items/{barcode}/return";
        let post = &value["paths"][path]["post"];
        assert!(post.is_object(), "return scan must stay POST {path} (one segment, named barcode)");
        assert!(value["paths"]["/loans/items/{item_id}/return"].is_null(), "return path param must not be documented as item_id");
        let params = post["parameters"].as_array().expect("parameters");
        assert!(
            params.iter().any(|param| param["in"] == "path" && param["name"] == "barcode"),
            "path param must be named barcode: {params:?}"
        );
        let ok = post["responses"]["200"]["description"].as_str().unwrap_or("");
        assert!(ok.contains("item_not_on_loan") && ok.contains("informational"), "200 must document the informational notice: {ok}");
        assert!(
            ok.contains("item_not_available_status") && ok.contains("warning") && ok.contains("circulationStatus") && ok.contains("weedingStatus") && ok.contains("unchanged"),
            "200 must document the blocked-status warning: {ok}"
        );
        let missing = post["responses"]["404"]["description"].as_str().unwrap_or("");
        assert!(!missing.contains("item_not_on_loan"), "unknown barcode stays not_found: {missing}");
        assert!(value["components"]["schemas"]["DeskNotice"].is_object());
        assert!(value["components"]["schemas"]["ReturnResponse"]["properties"]["notice"].is_object());
        assert!(value["components"]["schemas"]["ReturnResponse"]["properties"]["holdFor"].is_object());
        assert!(value["components"]["schemas"]["ReturnResponse"]["properties"]["circulationStatus"].is_object());
        assert!(value["components"]["schemas"]["ReturnResponse"]["properties"]["weedingStatus"].is_object());
        for field in ["holdStatus", "transitId", "pickupSiteId"] {
            assert!(
                value["components"]["schemas"]["ReturnResponse"]["properties"][field].is_object(),
                "ReturnResponse.{field} must be documented"
            );
        }
        assert!(
            ok.contains("holdStatus") && ok.contains("transitId") && ok.contains("pickupSiteId"),
            "200 must document the set-aside outcome: {ok}"
        );
        let refused = post["responses"]["422"]["description"].as_str().unwrap_or("");
        assert!(
            refused.contains("set_aside_item_mismatch") && refused.contains("set_aside_wrong_title") && refused.contains("set_aside_not_oldest"),
            "422 must document set-aside guards: {refused}"
        );
    }

    #[test]
    fn create_user_documents_optional_notice_language() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let post = &value["paths"]["/users"]["post"];
        assert!(post["responses"].get("422").is_some(), "POST /users must document 422: {}", post["responses"]);
        let schemas = value["components"]["schemas"].as_object().expect("schemas");
        let prop = &schemas["UserPayload"]["properties"]["language"];
        assert!(prop.is_object(), "UserPayload.language missing from OpenAPI: {prop}");
        let nullable = prop.get("nullable").and_then(|flag| flag.as_bool()) == Some(true)
            || prop["anyOf"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item.get("type").and_then(|kind| kind.as_str()) == Some("null")));
        assert!(nullable, "create language must be nullable: {prop}");
        let reference = prop["allOf"][0]["$ref"]
            .as_str()
            .or_else(|| prop["$ref"].as_str())
            .or_else(|| prop["anyOf"].as_array().and_then(|items| items.iter().find_map(|item| item["$ref"].as_str())));
        let enum_schema = match reference {
            Some(reference) => {
                let name = reference.rsplit('/').next().expect("schema name");
                schemas.get(name).unwrap_or_else(|| panic!("missing schema {name} for language property {prop}"))
            }
            None => prop,
        };
        let codes = enum_schema["enum"]
            .as_array()
            .map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(codes, vec!["english".to_string(), "french".to_string(), "german".to_string(), "spanish".to_string()], "{enum_schema}");
    }

    #[test]
    fn library_info_documents_nullable_ui_language() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let schemas = value["components"]["schemas"].as_object().expect("schemas");
        let prop = &schemas["LibraryInfo"]["properties"]["language"];
        assert!(prop.is_object(), "LibraryInfo.language missing from OpenAPI: {prop}");
        let nullable = prop.get("nullable").and_then(|flag| flag.as_bool()) == Some(true)
            || prop["anyOf"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item.get("type").and_then(|kind| kind.as_str()) == Some("null")));
        assert!(nullable, "language must be nullable: {prop}");

        let reference = prop["allOf"][0]["$ref"]
            .as_str()
            .or_else(|| prop["$ref"].as_str())
            .or_else(|| prop["anyOf"].as_array().and_then(|items| items.iter().find_map(|item| item["$ref"].as_str())));
        let enum_schema = match reference {
            Some(reference) => {
                let name = reference.rsplit('/').next().expect("schema name");
                schemas.get(name).unwrap_or_else(|| panic!("missing schema {name} for language property {prop}"))
            }
            None => prop,
        };
        let codes = enum_schema["enum"]
            .as_array()
            .map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(codes, vec!["fr".to_string(), "en".to_string(), "de".to_string(), "es".to_string()], "{enum_schema}");
    }

    #[test]
    fn lost_found_damaged_reports_unchanged_fine_action() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let action = &value["components"]["schemas"]["LostFineAction"];
        let codes = action["enum"]
            .as_array()
            .map(|items| items.iter().filter_map(|item| item.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert!(codes.contains(&"unchanged"), "LostFineAction must include unchanged: {action}");

        let path = &value["paths"]["/loans/items/{item_id}/lost-found"]["post"];
        let description = path["description"].as_str().unwrap_or("");
        let lower = description.to_ascii_lowercase();
        assert!(
            lower.contains("unchanged") && lower.contains("damaged"),
            "lost-found description must say damaged leaves the charge unchanged: {description}"
        );
        assert!(
            lower.contains("refundedamount") && lower.contains("zero"),
            "lost-found description must report zero refunded/cancelled amounts for damaged: {description}"
        );
    }

    #[test]
    fn password_reset_request_marks_reset_url_deprecated_and_ignored() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let prop = &value["components"]["schemas"]["RequestPasswordResetRequest"]["properties"]["resetUrl"];
        assert_eq!(prop["deprecated"], true, "{prop}");
        let description = prop["description"].as_str().unwrap_or("");
        assert!(description.to_ascii_lowercase().contains("ignored"), "{description}");
    }

    #[test]
    fn password_reset_request_documents_only_the_generic_response() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let paths = value["paths"].as_object().expect("paths");
        let (_path, item) = paths.iter().find(|(path, _)| path.ends_with("request-password-reset")).expect("request-password-reset");
        let responses = &item["post"]["responses"];
        assert!(responses.get("404").is_none(), "{responses}");
        assert!(responses.get("429").is_none(), "{responses}");
        let description = responses["200"]["description"].as_str().unwrap_or("");
        let lower = description.to_ascii_lowercase();
        assert!(lower.contains("generic"), "{description}");
        assert!(
            lower.contains("queued") && lower.contains("smtp"),
            "reset acceptance must document asynchronous delivery: {description}"
        );
    }

    #[test]
    fn checkout_documents_refusal_codes() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let description = value["paths"]["/loans"]["post"]["responses"]["422"]["description"].as_str().unwrap_or("");
        for code in [
            "subscription_expired",
            "fines_over_threshold",
            "max_loans",
            "max_media_loans",
            "item_not_loanable",
            "item_held_for_other",
            "item_status_action_required",
            "card_lost",
            "item_to_pick_for_other",
            "business_rule_violation",
        ] {
            assert!(description.contains(code), "POST /loans 422 must list {code}: {description}");
        }
        assert!(
            description.contains("forceable") && description.contains("circulation_override_rights"),
            "POST /loans 422 must document caller-specific forceable: {description}"
        );
        assert!(
            description.contains("loans_rights") && description.contains("loan.fine_threshold_overridden") && description.contains("patron_id"),
            "POST /loans 422 must document the shared fine override: {description}"
        );
        assert!(
            description.contains("reasonRequired") && description.contains("override_reason_required"),
            "POST /loans 422 must document the mandatory reason: {description}"
        );
        assert!(description.contains("overrideReason"), "POST /loans 422 must document overrideReason: {description}");
        assert!(
            description.contains("`item_held_for_other` is `{ holdId }` only")
                && description.contains("userId")
                && description.contains("cardNumber")
                && description.contains("initials")
                && description.contains("are absent"),
            "POST /loans 422 must document hold-id-only details: {description}"
        );
        let held = &value["components"]["schemas"]["ItemHeldForOtherDetails"];
        let mut held_keys: Vec<_> = held["properties"].as_object().expect("ItemHeldForOtherDetails properties").keys().cloned().collect();
        held_keys.sort();
        assert_eq!(held_keys, ["holdId"], "ItemHeldForOtherDetails must be holdId only: {held}");
        let held_text = held.to_string();
        for forbidden in ["userId", "cardNumber", "initials", "firstname", "lastname"] {
            assert!(!held_text.contains(forbidden), "{forbidden} must be absent from ItemHeldForOtherDetails: {held}");
        }
        let to_pick = &value["components"]["schemas"]["ItemToPickDetails"]["properties"];
        for key in ["holdId", "userId", "cardNumber", "initials"] {
            assert!(to_pick[key].is_object(), "{key} must stay on ItemToPickDetails: {to_pick}");
        }
        let batch = value["paths"]["/loans/batch-create"]["post"]["responses"]["200"]["description"].as_str().unwrap_or("");
        assert!(
            batch.contains("fines_over_threshold")
                && batch.contains("loans_rights")
                && batch.contains("loan.fine_threshold_overridden")
                && batch.contains("circulation_override_rights")
                && batch.contains("overrideReason"),
            "batch checkout must document both overrides: {batch}"
        );
        let schema = value["paths"]["/loans"]["post"]["responses"]["422"]["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .unwrap_or("");
        assert!(schema.contains("CheckoutRefusalBody"), "POST /loans 422 body must be CheckoutRefusalBody: {schema}");
        let properties = &value["components"]["schemas"]["CheckoutRefusalBody"]["properties"];
        assert!(properties["forceable"].is_object(), "{properties}");
        assert!(properties["reasonRequired"].is_object(), "{properties}");
        assert!(properties["itemStatus"].is_object(), "{properties}");
        let request = &value["components"]["schemas"]["CreateLoanRequest"]["properties"];
        assert!(request["overrideReason"].is_object(), "{request}");
        assert!(request["force"].is_object(), "{request}");
        assert!(properties["code"].is_object(), "{properties}");
        assert!(properties["message"].is_object(), "{properties}");
    }

    #[test]
    fn renewal_documents_subscription_expired() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        for path in ["/loans/{id}/renew", "/loans/items/{item_id}/renew"] {
            let description = value["paths"][path]["post"]["responses"]["422"]["description"].as_str().unwrap_or("");
            assert!(description.contains("subscription_expired"), "{path}: {description}");
            assert!(
                description.contains("forceable") && description.contains("loans_rights"),
                "{path} 422 must document caller-specific forceable: {description}"
            );
            assert!(
                description.contains("circulation_override_rights"),
                "{path} 422 must document circulation override write for subscription_expired: {description}"
            );
            assert!(
                description.contains("reasonRequired") && description.contains("override_reason_required"),
                "{path} 422 must document the mandatory reason: {description}"
            );
            assert!(
                description.contains("max_renewals") && description.contains("renewal_too_early"),
                "{path} 422 must document the limit codes force can lift: {description}"
            );
            let body_schema = value["paths"][path]["post"]["responses"]["422"]["content"]["application/json"]["schema"]["$ref"].as_str().unwrap_or("");
            assert!(body_schema.contains("RenewalRefusalBody"), "{path} 422 body must be RenewalRefusalBody: {body_schema}");
        }
        let schema = &value["components"]["schemas"]["RenewalRefusal"];
        let rendered = schema.to_string();
        assert!(rendered.contains("subscription_expired"), "RenewalRefusal must list subscription_expired: {schema}");
        let properties = &value["components"]["schemas"]["RenewalRefusalBody"]["properties"];
        assert!(properties["code"].is_object(), "{properties}");
        assert!(properties["error"].is_object(), "{properties}");
        assert!(properties["message"].is_object(), "{properties}");
        assert!(properties["forceable"].is_object(), "{properties}");
        assert!(properties["reasonRequired"].is_object(), "{properties}");
    }

    #[test]
    fn closed_loan_schemas_expose_closure_reason_and_found_at() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let schemas = &value["components"]["schemas"];
        for name in ["LoanDetails", "LoanDetailsStaff", "LoanDetailsPublic", "LoanDetailsOwn", "LoanDetailsView", "MyLoanView"] {
            let properties = &schemas[name]["properties"];
            assert!(properties["closureReason"].is_object(), "{name} must expose closureReason: {properties}");
            assert!(properties["foundAt"].is_object(), "{name} must expose foundAt: {properties}");
            let found_at = properties["foundAt"].to_string();
            assert!(found_at.contains("date-time") || found_at.contains("DateTime"), "{name} foundAt must be a timestamp: {found_at}");
        }
        let reason = &schemas["LoanClosureReason"];
        let codes = reason["enum"]
            .as_array()
            .map(|items| items.iter().filter_map(|item| item.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert_eq!(codes, vec!["returned", "lost"], "{reason}");

        let me = value["paths"]["/me/loans"]["get"]["description"].as_str().unwrap_or("");
        assert!(me.contains("closureReason") && me.contains("foundAt") && me.contains("archived=true"), "{me}");
    }

    #[test]
    fn pay_fine_documents_payment_exceeds_balance() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let pay = &value["paths"]["/fines/{id}/pay"]["post"]["responses"]["422"];
        let pay_description = pay["description"].as_str().unwrap_or("");
        assert!(pay_description.contains("payment_exceeds_balance") && pay_description.contains("remainingCents"), "{pay}");
        let pay_schema = pay["content"]["application/json"]["schema"]["$ref"].as_str().unwrap_or("");
        assert!(
            pay_schema.contains("PaymentExceedsBalanceBody"),
            "POST /fines/{{id}}/pay 422 body must be PaymentExceedsBalanceBody: {pay_schema}"
        );
        let pay_props = &value["components"]["schemas"]["PaymentExceedsBalanceBody"]["properties"];
        for key in ["code", "error", "message", "remainingCents"] {
            assert!(pay_props[key].is_object(), "{key} missing: {pay_props}");
        }
    }

    #[test]
    fn put_item_documents_read_only_circulation_status() {
        let doc = ApiDoc::openapi();
        let value = serde_json::to_value(&doc).expect("openapi json");
        let put = &value["paths"]["/items/{id}"]["put"];
        let description = put["description"].as_str().unwrap_or("");
        let lower = description.to_ascii_lowercase();
        assert!(lower.contains("circulation_status_read_only"), "{description}");
        assert!(description.contains("lost") && description.contains("missing") && description.contains("to-search"), "{description}");
        let rejected = &put["responses"]["422"];
        let rejected_description = rejected["description"].as_str().unwrap_or("");
        assert!(rejected_description.contains("circulation_status_read_only"), "{rejected}");
        let schema = rejected["content"]["application/json"]["schema"]["$ref"].as_str().unwrap_or("");
        assert!(schema.contains("ErrorResponse"), "422 body must be ErrorResponse: {rejected}");
        let field = &value["components"]["schemas"]["Item"]["properties"]["circulationStatus"];
        let field_description = field["description"].as_str().unwrap_or("");
        assert!(field_description.contains("circulation_status_read_only"), "{field}");
    }
}
