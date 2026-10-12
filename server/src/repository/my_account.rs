//! Owner-scoped loan and hold lists. SQL always filters `user_id` to the caller.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use super::Repository;
use crate::{
    error::AppResult,
    models::{
        biblio::MediaType,
        circulation::CirculationStatus,
        item::WeedingStatus,
        my_account::{MyHoldStatus, MyHoldView, MyLoanView},
    },
    renewal::{renewal_eligibility, RenewalContext, RenewalLoan, RenewalUser},
    services::fines::UnpaidThresholdCheck,
};

#[derive(Debug, FromRow)]
struct OwnerLoanRow {
    id: i64,
    barcode: Option<String>,
    title: Option<String>,
    media_type: String,
    start_date: DateTime<Utc>,
    due_date: Option<DateTime<Utc>>,
    renewal_count: i16,
    weeding_status: String,
    circulation_status: Option<i16>,
    hold_waiting: bool,
    is_overdue: bool,
}

#[derive(Debug, FromRow)]
struct OwnerHoldRow {
    id: i64,
    biblio_id: i64,
    title: Option<String>,
    status: String,
    expires_at: Option<DateTime<Utc>>,
    pickup_site_id: Option<i64>,
    pickup_site_name: Option<String>,
    position: i32,
    created_at: DateTime<Utc>,
    in_transit: bool,
}

impl Repository {
    /// Active loans for this patron. `user_id` is the caller; the statement filters on it.
    pub async fn loans_list_for_owner(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> AppResult<(Vec<MyLoanView>, i64)> {
        let offset = (page - 1) * per_page;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM loans l WHERE l.user_id = $1 AND l.returned_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query_as::<_, OwnerLoanRow>(
            r#"
            SELECT l.id,
                   it.barcode,
                   b.title,
                   b.media_type,
                   l.date AS start_date,
                   l.expiry_at AS due_date,
                   COALESCE(l.nb_renews, 0)::int2 AS renewal_count,
                   it.weeding_status,
                   it.circulation_status,
                   EXISTS (
                       SELECT 1 FROM holds h
                       WHERE h.biblio_id = b.id
                         AND h.user_id <> l.user_id
                         AND (
                               (h.status = 'pending' AND h.item_id IS NULL)
                               OR (h.item_id = l.item_id AND h.status IN ('pending', 'to_pick', 'ready'))
                             )
                   ) AS hold_waiting,
                   COALESCE(l.expiry_at < NOW(), false) AS is_overdue
            FROM loans l
            JOIN items it ON l.item_id = it.id
            JOIN biblios b ON it.biblio_id = b.id
            WHERE l.user_id = $1 AND l.returned_at IS NULL
            ORDER BY l.expiry_at
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut items = Vec::with_capacity(rows.len());
        if rows.is_empty() {
            return Ok((items, total));
        }

        // Account facts once per request. Media settings still vary per row.
        let owner = self.users_get_by_id(user_id).await?;
        let unpaid = self.fines_total_unpaid(user_id).await?;
        let threshold = self.fines_get_unpaid_threshold(owner.public_type).await?;
        let fines_over_threshold = UnpaidThresholdCheck { unpaid, threshold }.blocks_circulation();
        let renewal_user = RenewalUser {
            can_borrow: owner.can_borrow(),
            card_lost: self.users_card_lost(user_id).await?,
        };
        let zone = self.library_zone().await?;
        let now = Utc::now();
        let mut settings: HashMap<String, (i16, i16)> = HashMap::new();
        for row in rows {
            let (max_renews, window_days) = if let Some(cached) = settings.get(&row.media_type) {
                *cached
            } else {
                let (_, _, _, max_renews, _) = self
                    .resolve_loan_settings(owner.public_type, Some(row.media_type.as_str()))
                    .await?;
                let window_days = self
                    .resolve_renew_window_days(Some(row.media_type.as_str()))
                    .await?;
                settings.insert(row.media_type.clone(), (max_renews, window_days));
                (max_renews, window_days)
            };
            let due = row.due_date.unwrap_or(row.start_date);
            let renewal_loan = RenewalLoan {
                renewal_count: row.renewal_count,
                due_at: due,
                withdrawn: WeedingStatus::from_db_str(&row.weeding_status).blocks_circulation(),
                claimed_returned: CirculationStatus::from_db(row.circulation_status)
                    == CirculationStatus::ClaimedReturned,
                hold_waiting: row.hold_waiting,
            };
            let ctx = RenewalContext {
                bypass_account_gates: false,
                fines_over_threshold,
                fines_message: String::new(),
                max_renewals: max_renews,
                renew_window_days: window_days,
                now,
                zone,
            };
            let refusal = renewal_eligibility(&renewal_loan, &renewal_user, &ctx).err();
            items.push(MyLoanView {
                id: row.id,
                barcode: row.barcode,
                title: row.title,
                media_type: MediaType::from(row.media_type.as_str()),
                start_date: row.start_date,
                due_date: due,
                renewal_count: row.renewal_count,
                can_renew: refusal.is_none(),
                renewal_refusal_code: refusal,
                renewal_opens_at: refusal.and_then(|code| code.opens_at_iso(&renewal_loan, &ctx)),
                is_overdue: row.is_overdue,
            });
        }
        Ok((items, total))
    }

    /// Holds for this patron. `user_id` is the caller; the statement filters on it.
    pub async fn holds_list_for_owner(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<MyHoldView>, i64)> {
        let offset = (page - 1) * per_page;
        let (total, rows) = if active_only {
            let total: i64 = sqlx::query_scalar(
                "SELECT COUNT(*)::bigint FROM holds WHERE user_id = $1 AND status IN ('pending','to_pick','ready')",
            )
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
            let rows = sqlx::query_as::<_, OwnerHoldRow>(
                r#"
                SELECT h.id,
                       h.biblio_id,
                       b.title,
                       h.status,
                       h.expires_at,
                       h.pickup_site_id,
                       so.name AS pickup_site_name,
                       h.position,
                       h.created_at,
                       EXISTS (
                           SELECT 1 FROM item_transits t
                           WHERE t.hold_id = h.id
                             AND t.status IN ('requested', 'in_transit')
                       ) AS in_transit
                FROM holds h
                JOIN biblios b ON b.id = h.biblio_id
                LEFT JOIN sources so ON so.id = h.pickup_site_id
                WHERE h.user_id = $1
                  AND h.status IN ('pending','to_pick','ready')
                ORDER BY h.created_at ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        } else {
            let total: i64 =
                sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_one(&self.pool)
                    .await?;
            let rows = sqlx::query_as::<_, OwnerHoldRow>(
                r#"
                SELECT h.id,
                       h.biblio_id,
                       b.title,
                       h.status,
                       h.expires_at,
                       h.pickup_site_id,
                       so.name AS pickup_site_name,
                       h.position,
                       h.created_at,
                       EXISTS (
                           SELECT 1 FROM item_transits t
                           WHERE t.hold_id = h.id
                             AND t.status IN ('requested', 'in_transit')
                       ) AS in_transit
                FROM holds h
                JOIN biblios b ON b.id = h.biblio_id
                LEFT JOIN sources so ON so.id = h.pickup_site_id
                WHERE h.user_id = $1
                ORDER BY h.created_at ASC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(user_id)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
            (total, rows)
        };

        let items = rows.into_iter().map(owner_hold_view).collect();
        Ok((items, total))
    }
}

fn owner_hold_view(row: OwnerHoldRow) -> MyHoldView {
    let status = if row.in_transit {
        MyHoldStatus::InTransit
    } else {
        match row.status.as_str() {
            "to_pick" => MyHoldStatus::ToPick,
            "ready" => MyHoldStatus::Ready,
            "fulfilled" => MyHoldStatus::Fulfilled,
            "cancelled" => MyHoldStatus::Cancelled,
            "expired" => MyHoldStatus::Expired,
            _ => MyHoldStatus::Pending,
        }
    };
    let pickup_deadline = if status == MyHoldStatus::Ready {
        row.expires_at
    } else {
        None
    };
    MyHoldView {
        id: row.id,
        biblio_id: row.biblio_id,
        title: row.title,
        status,
        pickup_deadline,
        pickup_site_id: row.pickup_site_id,
        pickup_site_name: row.pickup_site_name,
        position: row.position,
        created_at: row.created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{owner_hold_view, OwnerHoldRow};
    use crate::models::my_account::MyHoldStatus;

    #[test]
    fn ready_hold_exposes_the_deadline_and_transit_hides_it() {
        let deadline = Utc.with_ymd_and_hms(2026, 11, 8, 16, 0, 0).unwrap();
        let ready = owner_hold_view(OwnerHoldRow {
            id: 1,
            biblio_id: 2,
            title: Some("Ready".into()),
            status: "ready".into(),
            expires_at: Some(deadline),
            pickup_site_id: Some(9),
            pickup_site_name: Some("Desk".into()),
            position: 1,
            created_at: deadline,
            in_transit: false,
        });
        assert_eq!(ready.status, MyHoldStatus::Ready);
        assert_eq!(ready.pickup_deadline, Some(deadline));
        assert_eq!(ready.pickup_site_name.as_deref(), Some("Desk"));

        let moving = owner_hold_view(OwnerHoldRow {
            id: 3,
            biblio_id: 2,
            title: Some("Moving".into()),
            status: "pending".into(),
            expires_at: Some(deadline),
            pickup_site_id: Some(9),
            pickup_site_name: Some("Desk".into()),
            position: 2,
            created_at: deadline,
            in_transit: true,
        });
        assert_eq!(moving.status, MyHoldStatus::InTransit);
        assert!(moving.pickup_deadline.is_none());
    }
}
