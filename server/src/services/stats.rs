//! Statistics service

use chrono::{DateTime, Utc};
use sqlx::Row;

use crate::{
    api::stats::{Interval, ItemStats, LoanStats, LoanStatsResponse, StatEntry, StatsResponse, TimeSeriesEntry, UserStats}, error::AppResult, models::item::MediaType, repository::Repository
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
            SELECT COALESCE(u.account_type, 'unknown') as label, COUNT(*) as value
            FROM users u
            WHERE (u.status IS NULL OR u.status != 2)
            GROUP BY u.account_type
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

    /// Get advanced loan statistics with time series
    pub async fn get_loan_stats(
        &self,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        interval: Interval,
        media_type: Option<&MediaType>,
        user_id: Option<i32>,
    ) -> AppResult<LoanStatsResponse> {
        let pool = &self.repository.pool;

        // Default date range: last 30 days if not specified
        let start = start_date.unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
        let end = end_date.unwrap_or_else(Utc::now);

        // Build date truncation expression based on interval
        let date_trunc = match interval {
            Interval::Day => "DATE_TRUNC('day', date)",
            Interval::Week => "DATE_TRUNC('week', date)",
            Interval::Month => "DATE_TRUNC('month', date)",
            Interval::Year => "DATE_TRUNC('year', date)",
        };

        let date_format = match interval {
            Interval::Day => "YYYY-MM-DD",
            Interval::Week => "IYYY-\"W\"IW",
            Interval::Month => "YYYY-MM",
            Interval::Year => "YYYY",
        };

        // Build WHERE clause
        let mut where_clauses = vec![
            format!("l.date >= '{}'", start.format("%Y-%m-%d %H:%M:%S")),
            format!("l.date <= '{}'", end.format("%Y-%m-%d %H:%M:%S")),
        ];

        if let Some(mt) = media_type {
            where_clauses.push(format!("i.media_type = '{}'", mt));
        }

        if let Some(uid) = user_id {
            where_clauses.push(format!("l.user_id = {}", uid));
        }

        let where_clause = where_clauses.join(" AND ");

        // Query for loans (from active loans table)
        let loans_query = format!(
            r#"
            SELECT 
                TO_CHAR({}, '{}') as period,
                COUNT(*) as count
            FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            JOIN items i ON s.id_item = i.id
            WHERE {}
            GROUP BY {}
            ORDER BY period
            "#,
            date_trunc, date_format, where_clause, date_trunc
        );

        let loans_data: Vec<(String, i64)> = sqlx::query(&loans_query)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| {
                let period: String = row.get("period");
                let count: i64 = row.get("count");
                (period, count)
            })
            .collect();

        // Query for loans from archives table (historical loans)
        let mut archived_loans_where = vec![
            format!("la.date >= '{}'", start.format("%Y-%m-%d %H:%M:%S")),
            format!("la.date <= '{}'", end.format("%Y-%m-%d %H:%M:%S")),
        ];

        if let Some(mt) = media_type {
            archived_loans_where.push(format!("i.media_type = '{}'", mt));
        }

        if let Some(uid) = user_id {
            archived_loans_where.push(format!("la.user_id = {}", uid));
        }

        let archived_loans_where_clause = archived_loans_where.join(" AND ");
        let archived_loans_date_trunc = date_trunc.replace("date", "la.date");

        let archived_loans_query = format!(
            r#"
            SELECT 
                TO_CHAR({}, '{}') as period,
                COUNT(*) as count
            FROM loans_archives la
            JOIN items i ON la.item_id = i.id
            WHERE {}
            GROUP BY {}
            ORDER BY period
            "#,
            archived_loans_date_trunc, date_format, archived_loans_where_clause, archived_loans_date_trunc
        );

        let archived_loans_data: Vec<(String, i64)> = sqlx::query(&archived_loans_query)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| {
                let period: String = row.get("period");
                let count: i64 = row.get("count");
                (period, count)
            })
            .collect();

        // Query for returns (from loans_archives table)
        let mut returns_where = vec![
            format!("la.returned_date >= '{}'", start.format("%Y-%m-%d %H:%M:%S")),
            format!("la.returned_date <= '{}'", end.format("%Y-%m-%d %H:%M:%S")),
            "la.returned_date IS NOT NULL".to_string(),
        ];

        if let Some(mt) = media_type {
            returns_where.push(format!("i.media_type = '{}'", mt));
        }

        if let Some(uid) = user_id {
            returns_where.push(format!("la.user_id = {}", uid));
        }

        let returns_where_clause = returns_where.join(" AND ");
        let returns_date_trunc = date_trunc.replace("date", "la.returned_date");

        let returns_query = format!(
            r#"
            SELECT 
                TO_CHAR({}, '{}') as period,
                COUNT(*) as count
            FROM loans_archives la
            JOIN items i ON la.item_id = i.id
            WHERE {}
            GROUP BY {}
            ORDER BY period
            "#,
            returns_date_trunc, date_format, returns_where_clause, returns_date_trunc
        );

        let returns_data: Vec<(String, i64)> = sqlx::query(&returns_query)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| {
                let period: String = row.get("period");
                let count: i64 = row.get("count");
                (period, count)
            })
            .collect();

        // Combine loans and returns by period
        use std::collections::HashMap;
        let mut period_map: HashMap<String, (i64, i64)> = HashMap::new();

        for (period, count) in loans_data {
            period_map.entry(period).or_insert((0, 0)).0 += count;
        }

        for (period, count) in archived_loans_data {
            period_map.entry(period).or_insert((0, 0)).0 += count;
        }

        for (period, count) in returns_data {
            period_map.entry(period).or_insert((0, 0)).1 += count;
        }

        let mut time_series: Vec<TimeSeriesEntry> = period_map
            .into_iter()
            .map(|(period, (loans, returns))| TimeSeriesEntry {
                period,
                loans,
                returns,
            })
            .collect();

        time_series.sort_by_key(|e| e.period.clone());

        // Get total counts
        let total_loans: i64 = time_series.iter().map(|e| e.loans).sum();
        let total_returns: i64 = time_series.iter().map(|e| e.returns).sum();

        // Get statistics by media type
        let by_media_type_query = format!(
            r#"
            SELECT 
                COALESCE(i.media_type, 'unknown') as label,
                COUNT(*) as value
            FROM loans l
            JOIN specimens s ON l.specimen_id = s.id
            JOIN items i ON s.id_item = i.id
            WHERE {}
            GROUP BY i.media_type
            ORDER BY value DESC
            "#,
            where_clause
        );

        let by_media_type = sqlx::query(&by_media_type_query)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| StatEntry {
                label: row.get("label"),
                value: row.get("value"),
            })
            .collect();

        Ok(LoanStatsResponse {
            total_loans,
            total_returns,
            time_series,
            by_media_type,
        })
    }
}


