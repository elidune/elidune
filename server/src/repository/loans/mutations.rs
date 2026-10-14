//! Loans repository — create, return, and renew mutations.

use chrono::{DateTime, Datelike, Utc};
use sqlx::Row;

use super::super::Repository;
use super::LOAN_DETAILS_FIRST_AUTHOR_SQL;
use crate::{
    checkout_refusal::{
        finish_checkout_refusals, push_checkout_refusal, CheckoutRefusal, ToPickPatron,
    },
    error::{error_code, AppError, AppResult},
    models::{
        biblio::BiblioShort,
        circulation::CirculationStatus,
        item::{ItemShort, WeedingStatus},
        loan::{
            CreateLoan, Loan, LoanClosureReason, LoanCreateOutcome, LoanDetails, LoanReturnOutcome,
            LoanSettingsRenewAt,
        },
        user::{UserRights, UserShort, UserShortRow},
    },
    renewal::{
        renewal_eligibility, LoanRenewalResult, LoanRenewalSnapshot, RenewalContext, RenewalLoan,
        RenewalUser,
    },
    services::fines::UnpaidThresholdCheck,
};

impl Repository {
    /// Create a new loan.
    ///
    /// Serializes on the item row (`FOR UPDATE`) and relies on
    /// `idx_loans_one_active_per_item` so concurrent checkouts cannot create two
    /// active loans on the same copy. An open loan is never closed here: another
    /// borrower is `409 item_on_loan_to_other_borrower`, the same borrower is
    /// `409 item_already_on_loan_to_borrower`. `force` does not change that.
    /// Settings and due-date reads use this transaction so they do not take a
    /// second pool connection while the item lock is held.
    ///
    /// `prior_refusals` are forceable patron gates (expired subscription, fines)
    /// already collected by the service. A later non-forceable item gate wins.
    pub async fn loans_create(&self, loan: &CreateLoan) -> AppResult<LoanCreateOutcome> {
        self.loans_create_with_prior(loan, Vec::new(), &UserRights::default())
            .await
    }

    pub async fn loans_create_with_prior(
        &self,
        loan: &CreateLoan,
        prior_refusals: Vec<CheckoutRefusal>,
        caller_rights: &UserRights,
    ) -> AppResult<LoanCreateOutcome> {
        let now = Utc::now();

        if loan.item_id.is_none() && loan.item_identification.is_none() {
            return Err(AppError::BadRequest(
                "item_id or item_identification required".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await?;

        // Lock the physical copy so concurrent checkouts (and hold checks) serialize.
        let item_row = if let Some(id) = loan.item_id {
            sqlx::query(
                r#"
                SELECT it.id, it.borrowable, it.circulation_status, it.weeding_status,
                       b.media_type
                FROM items it
                JOIN biblios b ON it.biblio_id = b.id
                WHERE it.id = $1
                FOR UPDATE OF it
                "#,
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT it.id, it.borrowable, it.circulation_status, it.weeding_status,
                       b.media_type
                FROM items it
                JOIN biblios b ON it.biblio_id = b.id
                WHERE it.barcode = $1
                FOR UPDATE OF it
                "#,
            )
            .bind(loan.item_identification.as_deref())
            .fetch_optional(&mut *tx)
            .await?
        };

        let item_row = item_row.ok_or_else(|| AppError::NotFound("Item not found".to_string()))?;
        let item_id: i64 = item_row.get("id");
        let borrowable: bool = item_row.get("borrowable");
        let circulation_status = crate::models::circulation::CirculationStatus::from_db(
            item_row.get("circulation_status"),
        );
        let weeding_status: crate::models::item::WeedingStatus = item_row.get("weeding_status");
        let media_type: Option<String> = item_row.get("media_type");
        let mut refusals = prior_refusals;

        if weeding_status.blocks_circulation() {
            push_checkout_refusal(
                &mut refusals,
                loan.force,
                CheckoutRefusal::business(
                    crate::models::item::WeedingStatus::CHECKOUT_BLOCKED,
                    caller_rights,
                ),
            )?;
        }

        let existing: Option<Loan> = sqlx::query_as::<_, Loan>(
            "SELECT * FROM loans WHERE item_id = $1 AND returned_at IS NULL FOR UPDATE",
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(ref existing_loan) = existing {
            return Err(open_loan_conflict(
                Some(existing_loan.user_id),
                loan.user_id,
            ));
        }

        if let Some(transit) = self.transits_active_for_item_tx(&mut tx, item_id).await? {
            // Requested and in-transit are `business_rule_violation`, which force does not skip.
            if transit.status.blocks_checkout(false) {
                push_checkout_refusal(
                    &mut refusals,
                    loan.force,
                    CheckoutRefusal::business(
                        "Item is in transit to another site and cannot be checked out",
                        caller_rights,
                    ),
                )?;
            }
        }

        match circulation_status {
            CirculationStatus::Lost
            | CirculationStatus::ClaimedReturned
            | CirculationStatus::ToSearch
            | CirculationStatus::Missing => {
                push_checkout_refusal(
                    &mut refusals,
                    loan.force,
                    CheckoutRefusal::status_action(circulation_status, caller_rights),
                )?;
            }
            CirculationStatus::Damaged => {
                push_checkout_refusal(
                    &mut refusals,
                    loan.force,
                    CheckoutRefusal::new(
                        error_code::ITEM_NOT_LOANABLE,
                        "Item is marked damaged and is not borrowable",
                        caller_rights,
                    ),
                )?;
            }
            CirculationStatus::Available if !borrowable => {
                push_checkout_refusal(
                    &mut refusals,
                    loan.force,
                    CheckoutRefusal::new(
                        error_code::ITEM_NOT_LOANABLE,
                        "Item is not borrowable",
                        caller_rights,
                    ),
                )?;
            }
            CirculationStatus::Available => {}
        }

        let user_public_type: Option<i64> =
            sqlx::query_scalar::<_, Option<i64>>("SELECT public_type FROM users WHERE id = $1")
                .bind(loan.user_id)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();

        let (duration_days, nb_max_media, nb_max_total, _, _) = self
            .resolve_loan_settings_conn(&mut tx, user_public_type, media_type.as_deref())
            .await?;

        if let Some(site_id) = loan.site_id {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id = $1)")
                    .bind(site_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if !exists {
                return Err(AppError::NotFound(format!("Site {site_id} not found")));
            }
        }
        let default_site_id = self.library_default_site_id_conn(&mut tx).await?;
        let calendar_site =
            crate::services::calendar::loan_calendar_site(loan.site_id, default_site_id);
        let expiry_at = self
            .loans_due_at(&mut tx, now, duration_days, calendar_site)
            .await?;

        let current_loans_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE user_id = $1 AND returned_at IS NULL",
        )
        .bind(loan.user_id)
        .fetch_one(&mut *tx)
        .await?;
        let current_loans_media: i64 = if let Some(ref mt) = media_type {
            sqlx::query_scalar(
                r#"
                SELECT COUNT(*) FROM loans l
                JOIN items it ON l.item_id = it.id
                JOIN biblios b ON it.biblio_id = b.id
                WHERE l.user_id = $1 AND l.returned_at IS NULL AND b.media_type = $2
                "#,
            )
            .bind(loan.user_id)
            .bind(mt)
            .fetch_one(&mut *tx)
            .await?
        } else {
            0
        };

        let total_limit_reached = current_loans_total >= nb_max_total as i64;
        let media_limit_reached = current_loans_media >= nb_max_media as i64;

        if total_limit_reached || media_limit_reached {
            let (code, msg) = match (total_limit_reached, media_limit_reached) {
                (true, true) => (
                    error_code::MAX_LOANS,
                    format!(
                        "Maximum loans reached: total ({}/{}), this media type ({}/{})",
                        current_loans_total, nb_max_total, current_loans_media, nb_max_media
                    ),
                ),
                (true, false) => (
                    error_code::MAX_LOANS,
                    format!(
                        "Maximum total loans reached ({}/{})",
                        current_loans_total, nb_max_total
                    ),
                ),
                (false, true) => (
                    error_code::MAX_MEDIA_LOANS,
                    format!(
                        "Maximum loans for this document type reached ({}/{})",
                        current_loans_media, nb_max_media
                    ),
                ),
                (false, false) => unreachable!(),
            };
            push_checkout_refusal(
                &mut refusals,
                loan.force,
                CheckoutRefusal::new(code, msg, caller_rights),
            )?;
        }

        // Only a hold assigned to this copy (`ready`, `to_pick`, or a pinned `pending`)
        // reserves it. An unassigned title hold does not. Another patron's assignment
        // is not forceable: staff set the copy aside or cancel the hold.
        if let Some((hold_id, user_id, status, card_number, initials)) = self
            .holds_assigned_hold_blocks_lending_tx(&mut tx, item_id)
            .await?
        {
            if user_id != loan.user_id {
                let refusal = if status == crate::models::hold::HoldStatus::ToPick {
                    CheckoutRefusal::to_pick(
                        ToPickPatron {
                            hold_id,
                            user_id,
                            card_number,
                            initials,
                        },
                        caller_rights,
                    )
                } else {
                    CheckoutRefusal::new(
                        error_code::ITEM_HELD_FOR_OTHER,
                        "This copy has an active hold for another patron — only the queued patron may borrow it",
                        caller_rights,
                    )
                };
                push_checkout_refusal(&mut refusals, loan.force, refusal)?;
            }
        }

        finish_checkout_refusals(refusals)?;

        // Savepoint so a unique-index conflict can be re-read on this connection.
        // PostgreSQL aborts the statement; without the savepoint the transaction
        // cannot look up which borrower holds the copy.
        sqlx::query("SAVEPOINT loan_insert")
            .execute(&mut *tx)
            .await?;
        let loan_id = match sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO loans (user_id, item_id, date, expiry_at, nb_renews, site_id)
            VALUES ($1, $2, $3, $4, 0, $5)
            RETURNING id
            "#,
        )
        .bind(loan.user_id)
        .bind(item_id)
        .bind(now)
        .bind(expiry_at)
        .bind(loan.site_id)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(id) => {
                sqlx::query("RELEASE SAVEPOINT loan_insert")
                    .execute(&mut *tx)
                    .await?;
                id
            }
            Err(e) if is_active_loan_unique_violation(&e) => {
                sqlx::query("ROLLBACK TO SAVEPOINT loan_insert")
                    .execute(&mut *tx)
                    .await?;
                let existing_user_id: Option<i64> = sqlx::query_scalar(
                    "SELECT user_id FROM loans WHERE item_id = $1 AND returned_at IS NULL",
                )
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await?;
                return Err(open_loan_conflict(existing_user_id, loan.user_id));
            }
            Err(e) => return Err(e.into()),
        };

        let fulfilled_hold_id = if loan.force {
            self.holds_cancel_active_for_item_tx(&mut tx, item_id)
                .await?;
            None
        } else {
            self.holds_fulfill_active_for_user_item_tx(&mut tx, loan.user_id, item_id)
                .await?
        };

        tx.commit().await?;

        Ok(LoanCreateOutcome {
            loan_id,
            expiry_at,
            site_id: loan.site_id,
            fulfilled_hold_id,
        })
    }

    /// Archive an active loan and delete it. Does not advance the hold queue.
    pub(crate) async fn loans_archive_and_delete_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        loan: &Loan,
        returned_at: DateTime<Utc>,
        closure_reason: LoanClosureReason,
    ) -> AppResult<()> {
        let user_row = sqlx::query(
            "SELECT addr_city, account_type, public_type, birthdate FROM users WHERE id = $1",
        )
        .bind(loan.user_id)
        .fetch_optional(&mut **tx)
        .await?;

        let account_type: Option<String> = user_row.as_ref().and_then(|r| r.get("account_type"));
        let birthdate: Option<chrono::NaiveDate> =
            user_row.as_ref().and_then(|r| r.get("birthdate"));
        let age_band = birthdate.and_then(|b| {
            crate::models::user::borrower_age_band(b, loan.date.date_naive()).map(str::to_string)
        });
        let loan_year = i16::try_from(loan.date.year()).ok();

        sqlx::query(
            r#"
            INSERT INTO loans_archives (
                user_id, item_id, date, nb_renews, expiry_at,
                returned_at, notes, borrower_public_type,
                addr_city, account_type, age_band, loan_year,
                closure_reason, site_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(loan.user_id)
        .bind(loan.item_id)
        .bind(loan.date)
        .bind(loan.nb_renews)
        .bind(loan.expiry_at)
        .bind(returned_at)
        .bind(&loan.notes)
        .bind(
            user_row
                .as_ref()
                .and_then(|r| r.get::<Option<i64>, _>("public_type")),
        )
        .bind(
            user_row
                .as_ref()
                .and_then(|r| r.get::<Option<String>, _>("addr_city")),
        )
        .bind(account_type)
        .bind(age_band)
        .bind(loan_year)
        .bind(closure_reason.as_str())
        .bind(loan.site_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query("DELETE FROM loans WHERE id = $1")
            .bind(loan.id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    /// Record that a copy closed as lost was accepted back.
    ///
    /// The loan stays `lost`. `found_at` is set on the latest unmatched loss
    /// archive for this copy. If that archive was not labeled `lost` (older
    /// data), it is corrected: the copy is lost now, and a lost copy cannot
    /// have been checked out again.
    pub(crate) async fn loans_mark_lost_copy_found_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        item_id: i64,
        found_at: DateTime<Utc>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE loans_archives
            SET closure_reason = 'lost',
                found_at = $2
            WHERE id = (
                SELECT id
                FROM loans_archives
                WHERE item_id = $1
                  AND found_at IS NULL
                ORDER BY
                    CASE WHEN closure_reason = 'lost' THEN 0 ELSE 1 END,
                    returned_at DESC NULLS LAST,
                    id DESC
                LIMIT 1
            )
            "#,
        )
        .bind(item_id)
        .bind(found_at)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Return a loan (moves it to loans_archives).
    pub async fn loans_return(&self, loan_id: i64) -> AppResult<LoanReturnOutcome> {
        let now = Utc::now();

        let loan = self.loans_get_by_id(loan_id).await?;

        if loan.returned_at.is_some() {
            return Err(AppError::BusinessRule("Loan already returned".to_string()));
        }

        let mut tx = self.pool.begin().await?;

        self.loans_archive_and_delete_tx(&mut tx, &loan, now, LoanClosureReason::Returned)
            .await?;

        let status_before: Option<i16> =
            sqlx::query_scalar("SELECT circulation_status FROM items WHERE id = $1")
                .bind(loan.item_id)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
        let was_lost = CirculationStatus::from_db(status_before) == CirculationStatus::Lost;

        // A physical return is an inventory check: clear claims-returned and
        // missing-shelf (`to_search`) so the copy re-enters the borrowable pool
        // and the hold / transit queue below can take it. A force loan leaves
        // those flags in place. Damaged / lost stay as set — lost is never
        // cleared here. Staff resolve that with POST /loans/items/{id}/lost-found.
        sqlx::query(
            r#"
            UPDATE items SET
                circulation_status = 0,
                borrowable = TRUE,
                updated_at = $2
            WHERE id = $1
              AND circulation_status IN ($3, $4)
            "#,
        )
        .bind(loan.item_id)
        .bind(now)
        .bind(crate::models::circulation::CirculationStatus::CLAIMED_RETURNED)
        .bind(crate::models::circulation::CirculationStatus::TO_SEARCH)
        .execute(&mut *tx)
        .await?;

        // A copy that is still lost must not satisfy a hold or start a transit.
        let readied_hold = if was_lost {
            None
        } else {
            self.holds_notify_next_tx(&mut tx, loan.item_id, self.hold_ready_expiry_days())
                .await?
        };
        let lost = if was_lost {
            Some(
                self.fines_lost_return_notice_tx(&mut tx, loan.id, loan.item_id)
                    .await?,
            )
        } else {
            None
        };

        if let Some(ref h) = readied_hold {
            tracing::debug!(
                target: "loans",
                hold_id = h.id,
                item_id = loan.item_id,
                "Marked next pending hold as ready after loan return"
            );
        }

        tx.commit().await?;

        let biblio_row = sqlx::query(&format!(
            r#"
            SELECT b.id as biblio_id, b.media_type, b.isbn, b.title, b.publication_date,
                   it.barcode as item_identification,
                   it.id as item_copy_id, it.barcode as item_barcode,
                   it.call_number as item_call_number, it.borrowable as item_borrowable,
                   so.name as item_source_name,
                   {}
            FROM biblios b
            JOIN items it ON it.biblio_id = b.id
            LEFT JOIN sources so ON it.source_id = so.id
            WHERE it.id = $1
            "#,
            LOAN_DETAILS_FIRST_AUTHOR_SQL
        ))
        .bind(loan.item_id)
        .fetch_one(&self.pool)
        .await?;

        let user_short_row = sqlx::query_as::<_, UserShortRow>(
            r#"
            SELECT u.id, u.login, u.barcode, u.firstname, u.lastname, u.account_type, u.public_type,
                   u.status, u.created_at, u.expiry_at,
                   0::bigint as nb_loans, 0::bigint as nb_late_loans
            FROM users u
            WHERE u.id = $1
            "#,
        )
        .bind(loan.user_id)
        .fetch_optional(&self.pool)
        .await?;

        let user: Option<UserShort> = user_short_row.map(|r| r.into());

        let borrowable: bool = biblio_row.get("item_borrowable");
        let weeding_status = crate::models::item::WeedingStatus::OnShelf;
        let (availability, can_hold_item) = crate::models::item::ItemAvailability::local_pair(
            true,
            borrowable,
            None,
            weeding_status,
        );
        let item_short = ItemShort {
            id: biblio_row.get("item_copy_id"),
            barcode: biblio_row.get("item_barcode"),
            call_number: biblio_row.get("item_call_number"),
            borrowable,
            circulation_status: None,
            weeding_status,
            source_name: biblio_row.get("item_source_name"),
            borrowed: true,
            availability,
            can_hold_item,
            hold_user_id: None,
            hold_user_barcode: None,
            hold_user_firstname: None,
            hold_user_lastname: None,
        };

        let details = LoanDetails {
            id: loan.id,
            item_id: loan.item_id,
            site_id: loan.site_id,
            start_date: loan.date,
            expiry_at: loan.expiry_at.unwrap_or(now),
            renewal_date: loan.renew_at,
            nb_renews: loan.nb_renews.unwrap_or(0),
            returned_at: Some(now),
            closure_reason: Some(LoanClosureReason::Returned),
            found_at: Some(None),
            biblio: BiblioShort {
                id: biblio_row.get("biblio_id"),
                media_type: biblio_row.get("media_type"),
                isbn: biblio_row.get("isbn"),
                title: biblio_row.get("title"),
                date: biblio_row.get("publication_date"),
                status: 0,
                is_valid: Some(true),
                archived_at: None,
                weeding_status: crate::models::item::WeedingStatus::OnShelf,
                author: biblio_row
                    .get::<Option<serde_json::Value>, _>("author")
                    .and_then(|v| serde_json::from_value(v).ok()),
                items: vec![item_short],
            },
            user,
            item_identification: biblio_row.get("item_identification"),
            is_overdue: false,
        };

        Ok(LoanReturnOutcome {
            details,
            readied_hold,
            hold_ready_email: None,
            lost,
        })
    }

    /// Loan already archived when the copy was declared lost. Returns the desk
    /// alert when that copy is still lost. `None` when this id is not that case.
    pub async fn loans_lost_notice_for_closed_loan(
        &self,
        loan_id: i64,
    ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
        let row = sqlx::query(
            r#"
            SELECT i.id AS item_id
            FROM fines f
            JOIN items i ON i.id = f.item_id
            WHERE f.loan_id = $1
              AND f.charge_type = 'replacement'
              AND i.circulation_status = $2
              AND i.archived_at IS NULL
            ORDER BY f.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(loan_id)
        .bind(CirculationStatus::LOST)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let item_id: i64 = row.get("item_id");
        let notice = self
            .fines_lost_return_notice(Some(loan_id), item_id)
            .await?;
        Ok(Some(crate::models::dto::circulation::ClosedLostLoan {
            item_id,
            notice,
        }))
    }

    /// Barcode of a lost copy that is not on an active loan.
    pub async fn loans_lost_scan_by_barcode(
        &self,
        barcode: &str,
    ) -> AppResult<Option<crate::models::dto::circulation::ClosedLostLoan>> {
        let item_id: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT id FROM items
            WHERE barcode = $1
              AND archived_at IS NULL
              AND circulation_status = $2
            LIMIT 1
            "#,
        )
        .bind(barcode)
        .bind(CirculationStatus::LOST)
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        let Some(item_id) = item_id else {
            return Ok(None);
        };
        let notice = self.fines_lost_return_notice(None, item_id).await?;
        Ok(Some(crate::models::dto::circulation::ClosedLostLoan {
            item_id,
            notice,
        }))
    }

    /// Loan-level facts plus the media settings [`crate::renewal::renewal_eligibility`] needs.
    ///
    /// Account facts (can borrow, unpaid fines) stay with the caller so `/me/loans`
    /// can compute them once per request.
    pub async fn loans_renewal_snapshot(&self, loan_id: i64) -> AppResult<LoanRenewalSnapshot> {
        let row = sqlx::query_as::<_, RenewalSnapshotRow>(
            r#"
            SELECT l.user_id,
                   (l.returned_at IS NOT NULL) AS returned,
                   COALESCE(l.nb_renews, 0)::int2 AS renewal_count,
                   COALESCE(l.expiry_at, l.date) AS due_at,
                   it.weeding_status,
                   it.circulation_status,
                   b.media_type,
                   EXISTS (
                       SELECT 1 FROM holds h
                       WHERE h.biblio_id = b.id
                         AND h.user_id <> l.user_id
                         AND (
                               (h.status = 'pending' AND h.item_id IS NULL)
                               OR (h.item_id = l.item_id AND h.status IN ('pending', 'to_pick', 'ready'))
                             )
                   ) AS hold_waiting
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            WHERE l.id = $1
            "#,
        )
        .bind(loan_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Loan with id {loan_id} not found")))?;

        let public_type: Option<i64> =
            sqlx::query_scalar("SELECT public_type FROM users WHERE id = $1")
                .bind(row.user_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        let (_, _, _, max_renewals, _) = self
            .resolve_loan_settings(public_type, row.media_type.as_deref())
            .await?;
        let renew_window_days = self
            .resolve_renew_window_days(row.media_type.as_deref())
            .await?;

        Ok(LoanRenewalSnapshot {
            user_id: row.user_id,
            returned: row.returned,
            loan: RenewalLoan {
                renewal_count: row.renewal_count,
                due_at: row.due_at,
                withdrawn: WeedingStatus::from_db_str(&row.weeding_status).blocks_circulation(),
                claimed_returned: CirculationStatus::from_db(row.circulation_status)
                    == CirculationStatus::ClaimedReturned,
                hold_waiting: row.hold_waiting,
            },
            max_renewals,
            renew_window_days,
        })
    }

    /// Renew a loan.
    ///
    /// Serializes on the item row then the loan row (`FOR UPDATE`, same lock order
    /// as [`Self::loans_create`]) so concurrent renews cannot both pass the max-renewals
    /// check. Refusals come from [`renewal_eligibility`] while the loan row is locked.
    /// `renewal_override` skips the gates it names: account status and unpaid fines,
    /// an expired subscription, and the renewal limits. A lost card, a withdrawn
    /// copy, a claims-returned copy, and a waiting hold are still refused.
    /// A returned loan is refused before that function.
    /// The due-date calendar slide stays after eligibility succeeds.
    /// `lifted` lists the refusals this override actually skipped.
    pub async fn loans_renew(
        &self,
        loan_id: i64,
        renewal_override: crate::renewal::RenewalOverride,
    ) -> AppResult<LoanRenewalResult> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await?;

        // Peek item_id without locking so the item row can be locked first
        // (same order as checkout: item → loan → holds).
        let item_id: i64 = sqlx::query_scalar("SELECT item_id FROM loans WHERE id = $1")
            .bind(loan_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Loan with id {loan_id} not found")))?;

        let item_row = sqlx::query(
            r#"
            SELECT it.id, it.circulation_status, it.weeding_status, b.media_type
            FROM items it
            JOIN biblios b ON it.biblio_id = b.id
            WHERE it.id = $1
            FOR UPDATE OF it
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Item not found".to_string()))?;
        let media_type: Option<String> = item_row.get("media_type");
        let circulation_status = CirculationStatus::from_db(item_row.get("circulation_status"));
        let weeding_status: WeedingStatus = item_row.get("weeding_status");

        let loan = sqlx::query_as::<_, Loan>("SELECT * FROM loans WHERE id = $1 FOR UPDATE")
            .bind(loan_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Loan with id {loan_id} not found")))?;

        if loan.returned_at.is_some() {
            return Err(AppError::BusinessRule(
                "Cannot renew a returned loan".to_string(),
            ));
        }

        // Account, fines, settings, and the due-date calendar stay on this
        // transaction. A second pool connection here deadlocks under load the
        // same way checkout used to.
        let account = sqlx::query(
            "SELECT status, archived_at, account_type, public_type, card_lost, expiry_at FROM users WHERE id = $1",
        )
        .bind(loan.user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User with id {} not found", loan.user_id)))?;
        let status: Option<crate::models::user::UserStatus> = account.get("status");
        let archived_at: Option<DateTime<Utc>> = account.get("archived_at");
        let account_type: String = account.get("account_type");
        let public_type: Option<i64> = account.get("public_type");
        let can_borrow = status == Some(crate::models::user::UserStatus::Active)
            && archived_at.is_none()
            && !account_type.eq_ignore_ascii_case("guest");
        let card_lost: bool = account.get("card_lost");
        let subscription_expiry: Option<DateTime<Utc>> = account.get("expiry_at");

        let unpaid = self.fines_total_unpaid_conn(&mut tx, loan.user_id).await?;
        let threshold = self
            .fines_get_unpaid_threshold_conn(&mut tx, public_type)
            .await?;
        let check = UnpaidThresholdCheck { unpaid, threshold };
        let fines_over_threshold = check.blocks_circulation();
        let fines_message = check.desk_block_message();
        let hold_waiting = self
            .holds_waiting_hold_blocks_renewal_tx(&mut tx, loan.item_id, loan.user_id)
            .await?;
        let (duration_days, _nb_max_media, _nb_max_total, max_renews, renew_at_policy) = self
            .resolve_loan_settings_conn(&mut tx, public_type, media_type.as_deref())
            .await?;
        let renew_window_days = self
            .resolve_renew_window_days_conn(&mut tx, media_type.as_deref())
            .await?;
        let renewal_loan = RenewalLoan {
            renewal_count: loan.nb_renews.unwrap_or(0),
            due_at: loan.expiry_at.unwrap_or(now),
            withdrawn: weeding_status.blocks_circulation(),
            claimed_returned: circulation_status == CirculationStatus::ClaimedReturned,
            hold_waiting,
        };
        let ctx = RenewalContext {
            bypass_account_gates: renewal_override.account_gates,
            bypass_subscription_expired: renewal_override.subscription_expired,
            bypass_renewal_limits: renewal_override.renewal_limits,
            fines_over_threshold,
            fines_message,
            max_renewals: max_renews,
            renew_window_days,
            now,
            zone: self.library_zone_conn(&mut tx).await?,
        };
        let renewal_user = RenewalUser {
            can_borrow,
            card_lost,
            subscription_expiry,
        };
        if let Err(refusal) = renewal_eligibility(&renewal_loan, &renewal_user, &ctx) {
            return Err(refusal.into_app_error(&renewal_loan, &renewal_user, &ctx));
        }
        let lifted = crate::renewal::lifted_renewal_refusals(&renewal_loan, &renewal_user, &ctx);
        let current_renews = renewal_loan.renewal_count;
        let current_expiry = renewal_loan.due_at;

        let anchor = match renew_at_policy {
            LoanSettingsRenewAt::Now => now,
            LoanSettingsRenewAt::AtDueDate => current_expiry,
        };
        let default_site_id = self.library_default_site_id_conn(&mut tx).await?;
        let calendar_site =
            crate::services::calendar::loan_calendar_site(loan.site_id, default_site_id);
        let new_expiry_date = self
            .loans_due_at(&mut tx, anchor, duration_days, calendar_site)
            .await?;
        if new_expiry_date <= current_expiry {
            return Err(AppError::BusinessRule(
                "Renewal would not move the due date, so it was not counted".to_string(),
            ));
        }
        let new_renews = current_renews + 1;

        sqlx::query("UPDATE loans SET expiry_at = $1, renew_at = $2, nb_renews = $3 WHERE id = $4")
            .bind(new_expiry_date)
            .bind(now)
            .bind(new_renews)
            .bind(loan_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(LoanRenewalResult {
            expiry_at: new_expiry_date,
            renew_count: new_renews,
            lifted,
        })
    }
}

/// 409 for a copy that already has an open loan. `None` means the conflicting
/// row disappeared between the unique-index error and the re-read; treat that
/// as another borrower so the response stays 409.
fn open_loan_conflict(existing_user_id: Option<i64>, requester_id: i64) -> AppError {
    if existing_user_id == Some(requester_id) {
        AppError::coded(
            409,
            error_code::ITEM_ALREADY_ON_LOAN_TO_BORROWER,
            "This item is already on loan to this borrower. Renew the existing loan instead of creating a new one.",
        )
    } else {
        AppError::coded(
            409,
            error_code::ITEM_ON_LOAN_TO_OTHER_BORROWER,
            "This item is on loan to another borrower.",
        )
    }
}

/// `idx_loans_one_active_per_item` — last line of defense if two inserts race.
fn is_active_loan_unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("23505")
                && db.constraint() == Some("idx_loans_one_active_per_item")
        }
        _ => false,
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RenewalSnapshotRow {
    user_id: i64,
    returned: bool,
    renewal_count: i16,
    due_at: DateTime<Utc>,
    weeding_status: String,
    circulation_status: Option<i16>,
    media_type: Option<String>,
    hold_waiting: bool,
}
