//! Loans domain methods on [`Repository`].

mod mutations;
mod queries;
mod reminders;
mod settings;

use async_trait::async_trait;

use super::Repository;
use crate::{
    error::AppResult,
    models::loan::{
        CreateLoan, Loan, LoanCreateOutcome, LoanDetails, LoanMarcExportRow, LoanReturnOutcome,
        LoanSettings, LoanSettingsRenewAt,
    },
};

pub use crate::renewal::renewal_opens_at;
pub use reminders::{OverdueLoanRow, ReminderTierDelays};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait LoansRepository: Send + Sync {
    async fn loans_get_by_id(&self, id: i64) -> AppResult<Loan>;
    async fn loans_get_borrower_for_loan(
        &self,
        loan_id: i64,
    ) -> AppResult<crate::models::user::UserShort>;
    async fn loans_active_borrowers_by_item_ids(
        &self,
        item_ids: &[i64],
    ) -> AppResult<std::collections::HashMap<i64, crate::models::user::UserShort>>;
    async fn loans_get_by_item_identification(&self, item_identification: &str) -> AppResult<Loan>;
    async fn loans_get_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<LoanDetails>, i64)>;
    async fn loans_archives_get_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<LoanDetails>, i64)>;
    /// All loans for MARC export (no pagination). Active or archived only.
    async fn loans_get_for_marc_export(
        &self,
        user_id: i64,
        archived: bool,
    ) -> AppResult<Vec<LoanMarcExportRow>>;
    async fn loans_create(
        &self,
        loan: &CreateLoan,
        prior_refusals: Vec<crate::checkout_refusal::CheckoutRefusal>,
        caller_rights: &crate::models::user::UserRights,
    ) -> AppResult<LoanCreateOutcome>;
    async fn loans_return(&self, loan_id: i64) -> AppResult<LoanReturnOutcome>;
    /// Closed loan whose copy is still lost.
    async fn loans_lost_notice_for_closed_loan(
        &self,
        loan_id: i64,
    ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>>;
    /// Lost copy with no active loan, matched by barcode.
    async fn loans_lost_scan_by_barcode(
        &self,
        barcode: &str,
    ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>>;
    /// Barcode is not on loan. Completes a `to_pick` hold when this copy is assigned to one.
    async fn loans_complete_to_pick_by_identification(
        &self,
        identification: &str,
    ) -> AppResult<Option<crate::models::dto::holds::ToPickCompletion>>;
    /// Barcode of a `to_search` copy. Clears the missing state and runs the return queue.
    async fn loans_recover_to_search_by_identification(
        &self,
        identification: &str,
    ) -> AppResult<Option<crate::models::dto::holds::ShelfFound>>;
    /// Non-archived copy with this barcode. Used to tell a shelf copy from an unknown barcode.
    async fn loans_item_exists_by_barcode(&self, barcode: &str) -> AppResult<bool>;
    /// Circulation and weeding status of a live copy, read before a return scan writes.
    async fn loans_return_scan_copy(
        &self,
        barcode: &str,
    ) -> AppResult<Option<crate::models::item::ReturnScanCopy>>;
    /// Known copy that is not on loan. Serves the next pending hold (`ready` or transit).
    async fn loans_check_in_shelf_copy(
        &self,
        identification: &str,
    ) -> AppResult<Option<crate::models::dto::holds::ShelfCheckIn>>;
    /// Loan facts for [`crate::renewal::renewal_eligibility`] before the locked write.
    async fn loans_renewal_snapshot(
        &self,
        loan_id: i64,
    ) -> AppResult<crate::renewal::LoanRenewalSnapshot>;
    /// Skips the gates named on `renewal_override`. A lost card, a withdrawn copy,
    /// a claims-returned copy, and a waiting hold stay refused.
    async fn loans_renew(
        &self,
        loan_id: i64,
        renewal_override: crate::renewal::RenewalOverride,
    ) -> AppResult<crate::renewal::LoanRenewalResult>;
    async fn loans_get_settings(&self) -> AppResult<Vec<LoanSettings>>;
    async fn loans_count_active(&self) -> AppResult<i64>;
    async fn loans_count_overdue(&self) -> AppResult<i64>;
    async fn loans_count_active_for_item(&self, item_id: i64) -> AppResult<i64>;
    async fn loans_get_active_ids_for_item(&self, item_id: i64) -> AppResult<Vec<i64>>;
    async fn loans_get_active_ids_for_biblio(&self, biblio_id: i64) -> AppResult<Vec<i64>>;
    async fn loans_get_active_ids_for_user(&self, user_id: i64) -> AppResult<Vec<i64>>;
    async fn loans_count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64>;
    async fn loans_count_active_for_user(&self, user_id: i64) -> AppResult<i64>;
    async fn loans_get_overdue_for_reminders(
        &self,
        delays: ReminderTierDelays,
    ) -> AppResult<Vec<OverdueLoanRow>>;
    async fn loans_get_overdue(
        &self,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<OverdueLoanRow>, i64)>;
    async fn loans_update_reminder_sent(&self, loan_ids: &[i64]) -> AppResult<()>;
    /// Upsert global loan rules (`loans_settings`). `media_type == None` updates the default row (`media_type` IS NULL).
    async fn loans_settings_upsert_row(
        &self,
        media_type: Option<String>,
        nb_max: i16,
        nb_renews: i16,
        duration: i16,
        renew_at: LoanSettingsRenewAt,
        renew_window_days: i16,
    ) -> AppResult<()>;
    async fn loans_settings_delete_rows(&self) -> AppResult<()>;
    /// IANA timezone name from library settings (default `Europe/Paris`).
    async fn library_timezone_name(&self) -> AppResult<String>;
}

/// Combined repository trait used by [`crate::services::loans::LoansService`].
///
/// Implemented by the concrete [`Repository`] via blanket impl below.
pub trait LoansServiceRepository:
    LoansRepository + crate::repository::UsersRepository + Send + Sync
{
}

impl<T: LoansRepository + crate::repository::UsersRepository + Send + Sync> LoansServiceRepository
    for T
{
}

#[async_trait::async_trait]
impl LoansRepository for Repository {
    async fn loans_get_by_id(&self, id: i64) -> crate::error::AppResult<Loan> {
        Repository::loans_get_by_id(self, id).await
    }
    async fn loans_get_borrower_for_loan(
        &self,
        loan_id: i64,
    ) -> crate::error::AppResult<crate::models::user::UserShort> {
        Repository::loans_get_borrower_for_loan(self, loan_id).await
    }
    async fn loans_active_borrowers_by_item_ids(
        &self,
        item_ids: &[i64],
    ) -> crate::error::AppResult<std::collections::HashMap<i64, crate::models::user::UserShort>>
    {
        Repository::loans_active_borrowers_by_item_ids(self, item_ids).await
    }
    async fn loans_get_by_item_identification(
        &self,
        identification: &str,
    ) -> crate::error::AppResult<Loan> {
        Repository::loans_get_by_item_identification(self, identification).await
    }
    async fn loans_get_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> crate::error::AppResult<(Vec<LoanDetails>, i64)> {
        Repository::loans_get_for_user(self, user_id, page, per_page).await
    }
    async fn loans_archives_get_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> crate::error::AppResult<(Vec<LoanDetails>, i64)> {
        Repository::loans_archives_get_for_user(self, user_id, page, per_page).await
    }
    async fn loans_get_for_marc_export(
        &self,
        user_id: i64,
        archived: bool,
    ) -> crate::error::AppResult<Vec<LoanMarcExportRow>> {
        Repository::loans_get_for_marc_export(self, user_id, archived).await
    }
    async fn loans_create(
        &self,
        loan: &CreateLoan,
        prior_refusals: Vec<crate::checkout_refusal::CheckoutRefusal>,
        caller_rights: &crate::models::user::UserRights,
    ) -> crate::error::AppResult<LoanCreateOutcome> {
        Repository::loans_create_with_prior(self, loan, prior_refusals, caller_rights).await
    }
    async fn loans_return(&self, loan_id: i64) -> crate::error::AppResult<LoanReturnOutcome> {
        Repository::loans_return(self, loan_id).await
    }
    async fn loans_lost_notice_for_closed_loan(
        &self,
        loan_id: i64,
    ) -> crate::error::AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
        Repository::loans_lost_notice_for_closed_loan(self, loan_id).await
    }
    async fn loans_lost_scan_by_barcode(
        &self,
        barcode: &str,
    ) -> crate::error::AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
        Repository::loans_lost_scan_by_barcode(self, barcode).await
    }
    async fn loans_complete_to_pick_by_identification(
        &self,
        identification: &str,
    ) -> crate::error::AppResult<Option<crate::models::dto::holds::ToPickCompletion>> {
        Repository::holds_complete_to_pick_by_identification(self, identification).await
    }
    async fn loans_recover_to_search_by_identification(
        &self,
        identification: &str,
    ) -> crate::error::AppResult<Option<crate::models::dto::holds::ShelfFound>> {
        Repository::holds_recover_to_search_by_identification(self, identification).await
    }
    async fn loans_item_exists_by_barcode(&self, barcode: &str) -> crate::error::AppResult<bool> {
        Repository::loans_item_exists_by_barcode(self, barcode).await
    }
    async fn loans_return_scan_copy(
        &self,
        barcode: &str,
    ) -> crate::error::AppResult<Option<crate::models::item::ReturnScanCopy>> {
        Repository::loans_return_scan_copy(self, barcode).await
    }
    async fn loans_check_in_shelf_copy(
        &self,
        identification: &str,
    ) -> crate::error::AppResult<Option<crate::models::dto::holds::ShelfCheckIn>> {
        Repository::holds_check_in_shelf_copy(self, identification).await
    }
    async fn loans_renewal_snapshot(
        &self,
        loan_id: i64,
    ) -> crate::error::AppResult<crate::renewal::LoanRenewalSnapshot> {
        Repository::loans_renewal_snapshot(self, loan_id).await
    }
    async fn loans_renew(
        &self,
        loan_id: i64,
        renewal_override: crate::renewal::RenewalOverride,
    ) -> crate::error::AppResult<crate::renewal::LoanRenewalResult> {
        Repository::loans_renew(self, loan_id, renewal_override).await
    }
    async fn loans_get_settings(
        &self,
    ) -> crate::error::AppResult<Vec<crate::models::loan::LoanSettings>> {
        Repository::loans_get_settings(self).await
    }
    async fn loans_count_active(&self) -> crate::error::AppResult<i64> {
        Repository::loans_count_active(self).await
    }
    async fn loans_count_overdue(&self) -> crate::error::AppResult<i64> {
        Repository::loans_count_overdue(self).await
    }
    async fn loans_count_active_for_item(&self, item_id: i64) -> crate::error::AppResult<i64> {
        Repository::loans_count_active_for_item(self, item_id).await
    }
    async fn loans_get_active_ids_for_item(
        &self,
        item_id: i64,
    ) -> crate::error::AppResult<Vec<i64>> {
        Repository::loans_get_active_ids_for_item(self, item_id).await
    }
    async fn loans_get_active_ids_for_biblio(
        &self,
        biblio_id: i64,
    ) -> crate::error::AppResult<Vec<i64>> {
        Repository::loans_get_active_ids_for_biblio(self, biblio_id).await
    }
    async fn loans_get_active_ids_for_user(
        &self,
        user_id: i64,
    ) -> crate::error::AppResult<Vec<i64>> {
        Repository::loans_get_active_ids_for_user(self, user_id).await
    }
    async fn loans_count_active_for_biblio(&self, biblio_id: i64) -> crate::error::AppResult<i64> {
        Repository::loans_count_active_for_biblio(self, biblio_id).await
    }
    async fn loans_count_active_for_user(&self, user_id: i64) -> crate::error::AppResult<i64> {
        Repository::loans_count_active_for_user(self, user_id).await
    }
    async fn loans_get_overdue_for_reminders(
        &self,
        delays: ReminderTierDelays,
    ) -> crate::error::AppResult<Vec<OverdueLoanRow>> {
        Repository::loans_get_overdue_for_reminders(self, delays).await
    }
    async fn loans_get_overdue(
        &self,
        page: i64,
        per_page: i64,
    ) -> crate::error::AppResult<(Vec<OverdueLoanRow>, i64)> {
        Repository::loans_get_overdue(self, page, per_page).await
    }
    async fn loans_update_reminder_sent(&self, loan_ids: &[i64]) -> crate::error::AppResult<()> {
        Repository::loans_update_reminder_sent(self, loan_ids).await
    }
    async fn loans_settings_upsert_row(
        &self,
        media_type: Option<String>,
        nb_max: i16,
        nb_renews: i16,
        duration: i16,
        renew_at: LoanSettingsRenewAt,
        renew_window_days: i16,
    ) -> crate::error::AppResult<()> {
        Repository::loans_settings_upsert_row(
            self,
            media_type,
            nb_max,
            nb_renews,
            duration,
            renew_at,
            renew_window_days,
        )
        .await
    }
    async fn loans_settings_delete_rows(&self) -> crate::error::AppResult<()> {
        Repository::loans_settings_delete_rows(self).await
    }
    async fn library_timezone_name(&self) -> crate::error::AppResult<String> {
        Repository::library_timezone_name(self).await
    }
}

/// Scalar subquery (column alias `author`): first author on biblio `b` as JSON for [`BiblioShort`].
pub(crate) const LOAN_DETAILS_FIRST_AUTHOR_SQL: &str = r#"(SELECT jsonb_build_object(
                'id', a.id::text, 'lastname', a.lastname, 'firstname', a.firstname,
                'bio', a.bio, 'notes', a.notes, 'function', ba.function
            ) FROM biblio_authors ba JOIN authors a ON a.id = ba.author_id
            WHERE ba.biblio_id = b.id ORDER BY ba.position LIMIT 1) as author"#;
