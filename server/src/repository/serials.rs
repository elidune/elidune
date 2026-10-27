//! Frequencies and numbering patterns for serial subscriptions.

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use snowflaked::Generator;
use sqlx::FromRow;

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::{
        item::Item,
        serial::{
            CounterFormat, CounterRule, FrequencyUnit, FrequencyWrite, NewSerialIssue,
            NumberingPatternWrite, PredictionState, SerialFrequency, SerialIssue,
            SerialIssueStatus, SerialListQuery, SerialLocale, SerialNumberingPattern,
            SerialSubscription, SerialSubscriptionQuery, SerialSubscriptionWrite,
        },
    },
};

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(13)));

pub(super) fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .generate::<i64>()
}

fn page_window(query: &SerialListQuery) -> (i64, i64, Option<String>) {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let q = query
        .q
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    (per_page, (page - 1) * per_page, q)
}

fn map_serial_sql(err: sqlx::Error) -> AppError {
    let sql_state = match &err {
        sqlx::Error::Database(db) => db.code().map(|code| code.into_owned()),
        _ => None,
    };
    match sql_state.as_deref() {
        Some("23503") => AppError::Conflict("This serials record is still referenced".to_string()),
        Some("23514") => AppError::Validation(
            "Frequency or numbering pattern values are outside the allowed range".to_string(),
        ),
        _ => AppError::from(err),
    }
}

const FREQUENCY_COLUMNS: &str = "id, description, unit, issues_per_unit, units_per_issue, \
     display_order, created_at, updated_at";

const PATTERN_COLUMNS: &str = "id, label, description, display_order, formula, locale, \
     name_x, add_x, every_x, when_more_than_x, set_back_to_x, format_x, zero_pad_x, \
     name_y, add_y, every_y, when_more_than_y, set_back_to_y, format_y, zero_pad_y, \
     name_z, add_z, every_z, when_more_than_z, set_back_to_z, format_z, zero_pad_z, \
     created_at, updated_at";

#[derive(Debug, FromRow)]
struct FrequencyRow {
    id: i64,
    description: String,
    unit: Option<String>,
    issues_per_unit: i32,
    units_per_issue: i32,
    display_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PatternRow {
    id: i64,
    label: String,
    description: Option<String>,
    display_order: i32,
    formula: String,
    locale: String,
    name_x: Option<String>,
    add_x: i32,
    every_x: i32,
    when_more_than_x: Option<i32>,
    set_back_to_x: Option<i32>,
    format_x: String,
    zero_pad_x: Option<i32>,
    name_y: Option<String>,
    add_y: i32,
    every_y: i32,
    when_more_than_y: Option<i32>,
    set_back_to_y: Option<i32>,
    format_y: String,
    zero_pad_y: Option<i32>,
    name_z: Option<String>,
    add_z: i32,
    every_z: i32,
    when_more_than_z: Option<i32>,
    set_back_to_z: Option<i32>,
    format_z: String,
    zero_pad_z: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn frequency_from_row(row: FrequencyRow) -> AppResult<SerialFrequency> {
    let unit = match row.unit.as_deref() {
        None => None,
        Some(value) => Some(
            FrequencyUnit::from_db_str(value)
                .ok_or_else(|| AppError::Internal(format!("Unknown frequency unit '{value}'")))?,
        ),
    };
    Ok(SerialFrequency {
        id: row.id,
        description: row.description,
        unit,
        issues_per_unit: row.issues_per_unit,
        units_per_issue: row.units_per_issue,
        display_order: row.display_order,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn counter_from_row(
    name: Option<String>,
    add: i32,
    every: i32,
    when_more_than: Option<i32>,
    set_back_to: Option<i32>,
    format: &str,
    zero_pad: Option<i32>,
) -> AppResult<CounterRule> {
    let every = u32::try_from(every).map_err(|_| {
        AppError::Internal("Numbering pattern every-value is out of range".to_string())
    })?;
    let zero_pad = match zero_pad {
        None => None,
        Some(width) => Some(u8::try_from(width).map_err(|_| {
            AppError::Internal("Numbering pattern zero-pad is out of range".to_string())
        })?),
    };
    Ok(CounterRule {
        name,
        add,
        every,
        when_more_than,
        set_back_to,
        format: CounterFormat::from_db_str(format),
        zero_pad,
    })
}

fn pattern_from_row(row: PatternRow) -> AppResult<SerialNumberingPattern> {
    let locale = SerialLocale::from_db_str(&row.locale).ok_or_else(|| {
        AppError::Internal(format!("Unknown numbering pattern locale '{}'", row.locale))
    })?;
    Ok(SerialNumberingPattern {
        id: row.id,
        label: row.label,
        description: row.description,
        display_order: row.display_order,
        formula: row.formula,
        locale,
        x: counter_from_row(
            row.name_x,
            row.add_x,
            row.every_x,
            row.when_more_than_x,
            row.set_back_to_x,
            &row.format_x,
            row.zero_pad_x,
        )?,
        y: counter_from_row(
            row.name_y,
            row.add_y,
            row.every_y,
            row.when_more_than_y,
            row.set_back_to_y,
            &row.format_y,
            row.zero_pad_y,
        )?,
        z: counter_from_row(
            row.name_z,
            row.add_z,
            row.every_z,
            row.when_more_than_z,
            row.set_back_to_z,
            &row.format_z,
            row.zero_pad_z,
        )?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn every_i32(every: u32) -> AppResult<i32> {
    i32::try_from(every).map_err(|_| {
        AppError::Validation("Counter every-value does not fit a 32-bit integer".to_string())
    })
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait SerialsRepository: Send + Sync {
    async fn frequencies_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialFrequency>, i64)>;
    async fn frequencies_get(&self, id: i64) -> AppResult<SerialFrequency>;
    async fn frequencies_create(&self, data: &FrequencyWrite) -> AppResult<SerialFrequency>;
    async fn frequencies_update(
        &self,
        id: i64,
        data: &FrequencyWrite,
    ) -> AppResult<SerialFrequency>;
    async fn frequencies_delete(&self, id: i64) -> AppResult<SerialFrequency>;

    async fn patterns_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialNumberingPattern>, i64)>;
    async fn patterns_get(&self, id: i64) -> AppResult<SerialNumberingPattern>;
    async fn patterns_create(
        &self,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern>;
    async fn patterns_update(
        &self,
        id: i64,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern>;
    async fn patterns_delete(&self, id: i64) -> AppResult<SerialNumberingPattern>;

    async fn periodical_biblio(&self, biblio_id: i64) -> AppResult<()>;
    async fn subscriptions_list(
        &self,
        query: &SerialSubscriptionQuery,
    ) -> AppResult<(Vec<SerialSubscription>, i64)>;
    async fn subscriptions_get(&self, id: i64) -> AppResult<SerialSubscription>;
    async fn subscriptions_issues(&self, subscription_id: i64) -> AppResult<Vec<SerialIssue>>;
    /// Non-supplement issues. Horizon generation uses this so a hors-série does not advance `next_slot`.
    async fn subscriptions_regular_issue_count(&self, subscription_id: i64) -> AppResult<i64>;
    async fn subscriptions_insert(
        &self,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<i64>;
    async fn subscriptions_update(
        &self,
        id: i64,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        new_issues: &[NewSerialIssue],
    ) -> AppResult<()>;
    async fn subscriptions_append(
        &self,
        id: i64,
        expected_next_slot: i32,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<()>;
    async fn subscriptions_delete(&self, id: i64) -> AppResult<()>;

    async fn allocate_item_barcode(&self) -> AppResult<String>;
    async fn subscription_issue(
        &self,
        subscription_id: i64,
        issue_id: i64,
    ) -> AppResult<SerialIssue>;
    async fn issues_mark_arrived(
        &self,
        subscription_id: i64,
        issue_id: i64,
        received_at: DateTime<Utc>,
        item_id: Option<i64>,
    ) -> AppResult<Option<SerialIssue>>;
    /// Lock the issue, then insert its copy and mark it arrived in that same transaction.
    async fn receive_serial_issue(
        &self,
        subscription_id: i64,
        issue_id: i64,
        received_at: DateTime<Utc>,
        biblio_id: i64,
        item: Option<Item>,
        hold_latest_in_library: bool,
    ) -> AppResult<SerialIssue>;
    async fn library_zone(&self) -> AppResult<chrono_tz::Tz>;
    async fn issues_set_status(
        &self,
        subscription_id: i64,
        issue_id: i64,
        status: SerialIssueStatus,
        from: &[SerialIssueStatus],
    ) -> AppResult<Option<SerialIssue>>;
    async fn issues_insert_supplement(
        &self,
        subscription_id: i64,
        planned_date: NaiveDate,
        display_label: &str,
        notes: Option<String>,
    ) -> AppResult<SerialIssue>;
    async fn issues_mark_late(
        &self,
        subscription_id: i64,
        grace_days: i32,
        today: NaiveDate,
    ) -> AppResult<Vec<SerialIssue>>;
    async fn apply_latest_issue_borrowable(&self, subscription_id: i64) -> AppResult<()>;
}

#[async_trait]
impl SerialsRepository for Repository {
    async fn frequencies_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialFrequency>, i64)> {
        Repository::frequencies_list(self, query).await
    }
    async fn frequencies_get(&self, id: i64) -> AppResult<SerialFrequency> {
        Repository::frequencies_get(self, id).await
    }
    async fn frequencies_create(&self, data: &FrequencyWrite) -> AppResult<SerialFrequency> {
        Repository::frequencies_create(self, data).await
    }
    async fn frequencies_update(
        &self,
        id: i64,
        data: &FrequencyWrite,
    ) -> AppResult<SerialFrequency> {
        Repository::frequencies_update(self, id, data).await
    }
    async fn frequencies_delete(&self, id: i64) -> AppResult<SerialFrequency> {
        Repository::frequencies_delete(self, id).await
    }
    async fn patterns_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialNumberingPattern>, i64)> {
        Repository::patterns_list(self, query).await
    }
    async fn patterns_get(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        Repository::patterns_get(self, id).await
    }
    async fn patterns_create(
        &self,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        Repository::patterns_create(self, data).await
    }
    async fn patterns_update(
        &self,
        id: i64,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        Repository::patterns_update(self, id, data).await
    }
    async fn patterns_delete(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        Repository::patterns_delete(self, id).await
    }
    async fn periodical_biblio(&self, biblio_id: i64) -> AppResult<()> {
        Repository::periodical_biblio(self, biblio_id).await
    }
    async fn subscriptions_list(
        &self,
        query: &SerialSubscriptionQuery,
    ) -> AppResult<(Vec<SerialSubscription>, i64)> {
        Repository::subscriptions_list(self, query).await
    }
    async fn subscriptions_get(&self, id: i64) -> AppResult<SerialSubscription> {
        Repository::subscriptions_get(self, id).await
    }
    async fn subscriptions_issues(&self, subscription_id: i64) -> AppResult<Vec<SerialIssue>> {
        Repository::subscriptions_issues(self, subscription_id).await
    }
    async fn subscriptions_regular_issue_count(&self, subscription_id: i64) -> AppResult<i64> {
        Repository::subscriptions_regular_issue_count(self, subscription_id).await
    }
    async fn subscriptions_insert(
        &self,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<i64> {
        Repository::subscriptions_insert(self, data, cursor, issues).await
    }
    async fn subscriptions_update(
        &self,
        id: i64,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        new_issues: &[NewSerialIssue],
    ) -> AppResult<()> {
        Repository::subscriptions_update(self, id, data, cursor, new_issues).await
    }
    async fn subscriptions_append(
        &self,
        id: i64,
        expected_next_slot: i32,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<()> {
        Repository::subscriptions_append(self, id, expected_next_slot, cursor, issues).await
    }
    async fn subscriptions_delete(&self, id: i64) -> AppResult<()> {
        Repository::subscriptions_delete(self, id).await
    }
    async fn allocate_item_barcode(&self) -> AppResult<String> {
        Repository::allocate_item_barcode(self).await
    }
    async fn subscription_issue(
        &self,
        subscription_id: i64,
        issue_id: i64,
    ) -> AppResult<SerialIssue> {
        Repository::subscription_issue(self, subscription_id, issue_id).await
    }
    async fn issues_mark_arrived(
        &self,
        subscription_id: i64,
        issue_id: i64,
        received_at: DateTime<Utc>,
        item_id: Option<i64>,
    ) -> AppResult<Option<SerialIssue>> {
        Repository::issues_mark_arrived(self, subscription_id, issue_id, received_at, item_id).await
    }
    async fn receive_serial_issue(
        &self,
        subscription_id: i64,
        issue_id: i64,
        received_at: DateTime<Utc>,
        biblio_id: i64,
        item: Option<Item>,
        hold_latest_in_library: bool,
    ) -> AppResult<SerialIssue> {
        Repository::receive_serial_issue(
            self,
            subscription_id,
            issue_id,
            received_at,
            biblio_id,
            item,
            hold_latest_in_library,
        )
        .await
    }
    async fn library_zone(&self) -> AppResult<chrono_tz::Tz> {
        Repository::library_zone(self).await
    }
    async fn issues_set_status(
        &self,
        subscription_id: i64,
        issue_id: i64,
        status: SerialIssueStatus,
        from: &[SerialIssueStatus],
    ) -> AppResult<Option<SerialIssue>> {
        Repository::issues_set_status(self, subscription_id, issue_id, status, from).await
    }
    async fn issues_insert_supplement(
        &self,
        subscription_id: i64,
        planned_date: NaiveDate,
        display_label: &str,
        notes: Option<String>,
    ) -> AppResult<SerialIssue> {
        Repository::issues_insert_supplement(
            self,
            subscription_id,
            planned_date,
            display_label,
            notes.as_deref(),
        )
        .await
    }
    async fn issues_mark_late(
        &self,
        subscription_id: i64,
        grace_days: i32,
        today: NaiveDate,
    ) -> AppResult<Vec<SerialIssue>> {
        Repository::issues_mark_late(self, subscription_id, grace_days, today).await
    }
    async fn apply_latest_issue_borrowable(&self, subscription_id: i64) -> AppResult<()> {
        Repository::apply_latest_issue_borrowable(self, subscription_id).await
    }
}

impl Repository {
    pub async fn frequencies_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialFrequency>, i64)> {
        let (limit, offset, q) = page_window(query);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM serial_frequencies \
             WHERE ($1::text IS NULL OR strpos(lower(description), lower($1)) > 0)",
        )
        .bind(q.as_deref())
        .fetch_one(&self.pool)
        .await?;
        let sql = format!(
            "SELECT {FREQUENCY_COLUMNS} FROM serial_frequencies \
             WHERE ($1::text IS NULL OR strpos(lower(description), lower($1)) > 0) \
             ORDER BY display_order, description, id \
             LIMIT $2 OFFSET $3"
        );
        let rows = sqlx::query_as::<_, FrequencyRow>(&sql)
            .bind(q.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let frequencies = rows
            .into_iter()
            .map(frequency_from_row)
            .collect::<AppResult<Vec<_>>>()?;
        Ok((frequencies, total))
    }

    pub async fn frequencies_get(&self, id: i64) -> AppResult<SerialFrequency> {
        let sql = format!("SELECT {FREQUENCY_COLUMNS} FROM serial_frequencies WHERE id = $1");
        let row = sqlx::query_as::<_, FrequencyRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Frequency {id} not found")))?;
        frequency_from_row(row)
    }

    pub async fn frequencies_create(&self, data: &FrequencyWrite) -> AppResult<SerialFrequency> {
        let sql = format!(
            "INSERT INTO serial_frequencies ({FREQUENCY_COLUMNS}) \
             VALUES ($1, $2, $3, $4, $5, $6, now(), now()) \
             RETURNING {FREQUENCY_COLUMNS}"
        );
        let row = sqlx::query_as::<_, FrequencyRow>(&sql)
            .bind(next_id())
            .bind(data.description.trim())
            .bind(data.unit.map(FrequencyUnit::as_db_str))
            .bind(data.issues_per_unit)
            .bind(data.units_per_issue)
            .bind(data.display_order)
            .fetch_one(&self.pool)
            .await
            .map_err(map_serial_sql)?;
        frequency_from_row(row)
    }

    pub async fn frequencies_update(
        &self,
        id: i64,
        data: &FrequencyWrite,
    ) -> AppResult<SerialFrequency> {
        let sql = format!(
            "UPDATE serial_frequencies \
             SET description = $2, unit = $3, issues_per_unit = $4, units_per_issue = $5, \
                 display_order = $6, updated_at = now() \
             WHERE id = $1 \
             RETURNING {FREQUENCY_COLUMNS}"
        );
        let row = sqlx::query_as::<_, FrequencyRow>(&sql)
            .bind(id)
            .bind(data.description.trim())
            .bind(data.unit.map(FrequencyUnit::as_db_str))
            .bind(data.issues_per_unit)
            .bind(data.units_per_issue)
            .bind(data.display_order)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_serial_sql)?
            .ok_or_else(|| AppError::NotFound(format!("Frequency {id} not found")))?;
        frequency_from_row(row)
    }

    pub async fn frequencies_delete(&self, id: i64) -> AppResult<SerialFrequency> {
        let sql =
            format!("DELETE FROM serial_frequencies WHERE id = $1 RETURNING {FREQUENCY_COLUMNS}");
        let row = sqlx::query_as::<_, FrequencyRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_serial_sql)?
            .ok_or_else(|| AppError::NotFound(format!("Frequency {id} not found")))?;
        frequency_from_row(row)
    }

    pub async fn patterns_list(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<(Vec<SerialNumberingPattern>, i64)> {
        let (limit, offset, q) = page_window(query);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM serial_numbering_patterns \
             WHERE ($1::text IS NULL OR strpos(lower(label), lower($1)) > 0 \
                    OR strpos(lower(coalesce(description, '')), lower($1)) > 0)",
        )
        .bind(q.as_deref())
        .fetch_one(&self.pool)
        .await?;
        let sql = format!(
            "SELECT {PATTERN_COLUMNS} FROM serial_numbering_patterns \
             WHERE ($1::text IS NULL OR strpos(lower(label), lower($1)) > 0 \
                    OR strpos(lower(coalesce(description, '')), lower($1)) > 0) \
             ORDER BY display_order, label, id \
             LIMIT $2 OFFSET $3"
        );
        let rows = sqlx::query_as::<_, PatternRow>(&sql)
            .bind(q.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let patterns = rows
            .into_iter()
            .map(pattern_from_row)
            .collect::<AppResult<Vec<_>>>()?;
        Ok((patterns, total))
    }

    pub async fn patterns_get(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        let sql = format!("SELECT {PATTERN_COLUMNS} FROM serial_numbering_patterns WHERE id = $1");
        let row = sqlx::query_as::<_, PatternRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Numbering pattern {id} not found")))?;
        pattern_from_row(row)
    }

    pub async fn patterns_create(
        &self,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        let sql = format!(
            "INSERT INTO serial_numbering_patterns ({PATTERN_COLUMNS}) VALUES (\
             $1, $2, $3, $4, $5, $6, \
             $7, $8, $9, $10, $11, $12, $13, \
             $14, $15, $16, $17, $18, $19, $20, \
             $21, $22, $23, $24, $25, $26, $27, \
             now(), now()) RETURNING {PATTERN_COLUMNS}"
        );
        bind_pattern(sqlx::query_as::<_, PatternRow>(&sql).bind(next_id()), data)?
            .fetch_one(&self.pool)
            .await
            .map_err(map_serial_sql)
            .and_then(pattern_from_row)
    }

    pub async fn patterns_update(
        &self,
        id: i64,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        let sql = format!(
            "UPDATE serial_numbering_patterns SET \
             label = $2, description = $3, display_order = $4, formula = $5, locale = $6, \
             name_x = $7, add_x = $8, every_x = $9, when_more_than_x = $10, set_back_to_x = $11, \
             format_x = $12, zero_pad_x = $13, \
             name_y = $14, add_y = $15, every_y = $16, when_more_than_y = $17, set_back_to_y = $18, \
             format_y = $19, zero_pad_y = $20, \
             name_z = $21, add_z = $22, every_z = $23, when_more_than_z = $24, set_back_to_z = $25, \
             format_z = $26, zero_pad_z = $27, updated_at = now() \
             WHERE id = $1 RETURNING {PATTERN_COLUMNS}"
        );
        bind_pattern(sqlx::query_as::<_, PatternRow>(&sql).bind(id), data)?
            .fetch_optional(&self.pool)
            .await
            .map_err(map_serial_sql)?
            .ok_or_else(|| AppError::NotFound(format!("Numbering pattern {id} not found")))
            .and_then(pattern_from_row)
    }

    pub async fn patterns_delete(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        let sql = format!(
            "DELETE FROM serial_numbering_patterns WHERE id = $1 RETURNING {PATTERN_COLUMNS}"
        );
        let row = sqlx::query_as::<_, PatternRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_serial_sql)?
            .ok_or_else(|| AppError::NotFound(format!("Numbering pattern {id} not found")))?;
        pattern_from_row(row)
    }
}

fn bind_pattern<'q>(
    query: sqlx::query::QueryAs<'q, sqlx::Postgres, PatternRow, sqlx::postgres::PgArguments>,
    data: &'q NumberingPatternWrite,
) -> AppResult<sqlx::query::QueryAs<'q, sqlx::Postgres, PatternRow, sqlx::postgres::PgArguments>> {
    let description = data
        .description
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(query
        .bind(data.label.trim().to_string())
        .bind(description)
        .bind(data.display_order)
        .bind(data.formula.trim().to_string())
        .bind(data.locale.as_db_str())
        .bind(counter_name(&data.x))
        .bind(data.x.add)
        .bind(every_i32(data.x.every)?)
        .bind(data.x.when_more_than)
        .bind(data.x.set_back_to)
        .bind(data.x.format.as_db_str())
        .bind(data.x.zero_pad.map(i32::from))
        .bind(counter_name(&data.y))
        .bind(data.y.add)
        .bind(every_i32(data.y.every)?)
        .bind(data.y.when_more_than)
        .bind(data.y.set_back_to)
        .bind(data.y.format.as_db_str())
        .bind(data.y.zero_pad.map(i32::from))
        .bind(counter_name(&data.z))
        .bind(data.z.add)
        .bind(every_i32(data.z.every)?)
        .bind(data.z.when_more_than)
        .bind(data.z.set_back_to)
        .bind(data.z.format.as_db_str())
        .bind(data.z.zero_pad.map(i32::from)))
}

fn counter_name(rule: &CounterRule) -> Option<String> {
    rule.name
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
