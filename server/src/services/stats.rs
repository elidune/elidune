//! Statistics service

use sqlx::Row;

use crate::{
    api::stats::{ItemStats, LoanStats, StatEntry, StatsResponse, UserStats},
    error::AppResult,
    repository::Repository,
};

#[derive(Clone)]
pub struct StatsService {
    repository: Repository,
}

impl StatsService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    /// Get library statistics
    pub async fn get_stats(&self) -> AppResult<StatsResponse> {
        let pool = &self.repository.pool;

        // Item stats
        let total_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE is_archive = 0 OR is_archive IS NULL")
            .fetch_one(pool)
            .await?;

        let items_by_media_type = sqlx::query(
            r#"
            SELECT COALESCE(media_type, 'unknown') as label, COUNT(*) as value
            FROM items
            WHERE is_archive = 0 OR is_archive IS NULL
            GROUP BY media_type
            ORDER BY value DESC
            "#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| StatEntry {
            label: row.get("label"),
            value: row.get("value"),
        })
        .collect();

        let items_by_public_type = sqlx::query(
            r#"
            SELECT 
                CASE public_type
                    WHEN 97 THEN 'adult'
                    WHEN 106 THEN 'children'
                    ELSE 'unknown'
                END as label,
                COUNT(*) as value
            FROM items
            WHERE is_archive = 0 OR is_archive IS NULL
            GROUP BY public_type
            ORDER BY value DESC
            "#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| StatEntry {
            label: row.get("label"),
            value: row.get("value"),
        })
        .collect();

        // User stats (exclude deleted users: status != 2)
        let total_users: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE (status IS NULL OR status != 2)"
        )
            .fetch_one(pool)
            .await?;

        let active_users: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT user_id) FROM loans WHERE returned_date IS NULL"
        )
        .fetch_one(pool)
        .await?;

        let users_by_account_type = sqlx::query(
            r#"
            SELECT COALESCE(at.name, 'unknown') as label, COUNT(*) as value
            FROM users u
            LEFT JOIN account_types at ON u.account_type_id = at.id
            WHERE (u.status IS NULL OR u.status != 2)
            GROUP BY at.name
            ORDER BY value DESC
            "#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| StatEntry {
            label: row.get("label"),
            value: row.get("value"),
        })
        .collect();

        // Loan stats
        let active_loans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM loans WHERE returned_date IS NULL")
            .fetch_one(pool)
            .await?;

        let overdue_loans: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans WHERE returned_date IS NULL AND issue_date < NOW()"
        )
        .fetch_one(pool)
        .await?;

        let returned_today: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM loans_archives WHERE returned_date >= DATE_TRUNC('day', NOW())"
        )
        .fetch_one(pool)
        .await?;

        let loans_by_media_type = sqlx::query(
            r#"
            SELECT COALESCE(i.media_type, 'unknown') as label, COUNT(*) as value
            FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            JOIN items i ON s.id_item = i.id
            WHERE l.returned_date IS NULL
            GROUP BY i.media_type
            ORDER BY value DESC
            "#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| StatEntry {
            label: row.get("label"),
            value: row.get("value"),
        })
        .collect();

        Ok(StatsResponse {
            items: ItemStats {
                total: total_items,
                by_media_type: items_by_media_type,
                by_public_type: items_by_public_type,
            },
            users: UserStats {
                total: total_users,
                active: active_users,
                by_account_type: users_by_account_type,
            },
            loans: LoanStats {
                active: active_loans,
                overdue: overdue_loans,
                returned_today,
                by_media_type: loans_by_media_type,
            },
        })
    }
}


