//! Persistence for serial subscriptions and their predicted issue rows.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::{Postgres, Transaction};

use super::serials::next_id;
use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::serial::{
        CombinedIssue, FrequencySpec, FrequencyUnit, LengthKind, NewSerialIssue, PredictionState,
        RecurringSkip, SerialIssue, SerialIssueStatus, SerialSubscription, SerialSubscriptionQuery,
        SerialSubscriptionStatus, SerialSubscriptionWrite, SubscriptionLength,
    },
    serials::grace,
};

const SUBSCRIPTION_FROM: &str = "\
FROM serial_subscriptions s \
JOIN biblios b ON b.id = s.biblio_id \
JOIN serial_frequencies f ON f.id = s.frequency_id";

fn subscription_select() -> String {
    format!(
        "s.id, s.biblio_id, b.title AS biblio_title, {ISSNS} AS issns, \
         s.site_id, s.vendor_id, s.fund_id, s.cost, s.issue_price_cents, \
         s.frequency_id, s.numbering_pattern_id, \
         f.unit AS frequency_unit, f.issues_per_unit, f.units_per_issue, \
         s.first_issue_date, s.start_date, s.end_date, \
         s.value_x, s.value_y, s.value_z, s.inner_x, s.inner_y, s.inner_z, s.next_slot, \
         s.length_kind, s.length_count, s.irregularities, s.recurring_skips, s.combined_issues, \
         s.grace_days, s.status, s.create_items, s.latest_issue_library_use_only, \
         s.holdings_statement, s.call_number, s.location, s.public_note, s.internal_note, s.notes, \
         s.created_at, s.updated_at, \
         (SELECT COUNT(*)::bigint FROM serial_issues i \
           WHERE i.serial_subscription_id = s.id AND i.status <> 'deleted') AS issue_count"
    )
}

const ISSNS: &str = "\
COALESCE(( \
  SELECT array_agg(DISTINCT issn ORDER BY issn) FROM ( \
    SELECT se.issn::text AS issn FROM biblio_series bs \
    JOIN series se ON se.id = bs.series_id \
    WHERE bs.biblio_id = s.biblio_id AND se.issn IS NOT NULL AND btrim(se.issn) <> '' \
    UNION \
    SELECT c.issn::text FROM biblio_collections bc \
    JOIN collections c ON c.id = bc.collection_id \
    WHERE bc.biblio_id = s.biblio_id AND c.issn IS NOT NULL AND btrim(c.issn) <> '' \
  ) raw \
), ARRAY[]::text[])";

const SEARCH_WHERE: &str = "\
($1::text IS NULL OR strpos(lower(coalesce(b.title, '')), lower($1)) > 0 \
 OR EXISTS ( \
    SELECT 1 FROM biblio_series bs \
    JOIN series se ON se.id = bs.series_id \
    WHERE bs.biblio_id = s.biblio_id \
      AND strpos(lower(coalesce(se.issn, '')), lower($1)) > 0) \
 OR EXISTS ( \
    SELECT 1 FROM biblio_collections bc \
    JOIN collections c ON c.id = bc.collection_id \
    WHERE bc.biblio_id = s.biblio_id \
      AND strpos(lower(coalesce(c.issn, '')), lower($1)) > 0)) \
 AND ($2::bigint IS NULL OR s.site_id = $2) \
 AND ($3::bigint IS NULL OR s.vendor_id = $3) \
 AND ($4::text IS NULL OR s.status = $4)";

#[derive(Debug, sqlx::FromRow)]
struct SubscriptionRow {
    id: i64,
    biblio_id: i64,
    biblio_title: Option<String>,
    issns: Vec<String>,
    site_id: i64,
    vendor_id: Option<i64>,
    fund_id: Option<i64>,
    cost: Option<Decimal>,
    issue_price_cents: Option<i32>,
    frequency_id: i64,
    numbering_pattern_id: i64,
    frequency_unit: Option<String>,
    issues_per_unit: i32,
    units_per_issue: i32,
    first_issue_date: NaiveDate,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    value_x: i32,
    value_y: i32,
    value_z: i32,
    inner_x: i32,
    inner_y: i32,
    inner_z: i32,
    next_slot: i32,
    length_kind: String,
    length_count: i32,
    irregularities: Vec<i32>,
    recurring_skips: sqlx::types::Json<Vec<RecurringSkip>>,
    combined_issues: sqlx::types::Json<Vec<CombinedIssue>>,
    grace_days: Option<i32>,
    status: String,
    create_items: bool,
    latest_issue_library_use_only: bool,
    holdings_statement: Option<String>,
    call_number: Option<String>,
    location: Option<String>,
    public_note: Option<String>,
    internal_note: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    issue_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct IssueRow {
    id: i64,
    serial_subscription_id: i64,
    planned_date: NaiveDate,
    display_label: String,
    sequence: i32,
    slot: i32,
    value_x: i32,
    value_y: i32,
    value_z: i32,
    status: String,
    supplement: bool,
    item_id: Option<i64>,
    received_at: Option<DateTime<Utc>>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn map_subscription_sql(err: sqlx::Error) -> AppError {
    let (code, constraint) = match &err {
        sqlx::Error::Database(db) => (
            db.code().map(|code| code.into_owned()),
            db.constraint().map(str::to_string),
        ),
        _ => (None, None),
    };
    match code.as_deref() {
        Some("23505") if constraint.as_deref() == Some("serial_subscriptions_one_active_uidx") => {
            AppError::Conflict(
                "An open serial subscription already exists for this periodical at this site"
                    .to_string(),
            )
        }
        Some("23505") if constraint.as_deref() == Some("serial_issues_subscription_slot_key") => {
            AppError::Conflict("This horizon was already generated".to_string())
        }
        Some("23505")
            if constraint.as_deref() == Some("serial_issues_subscription_sequence_key") =>
        {
            AppError::Conflict("This horizon was already generated".to_string())
        }
        Some("23503") => {
            AppError::Validation("A referenced serial subscription record does not exist".into())
        }
        Some("23514") => AppError::Validation(
            "Serial subscription values are outside the allowed range".to_string(),
        ),
        _ => AppError::from(err),
    }
}

fn non_negative_i32(value: i32, label: &str) -> AppResult<u32> {
    u32::try_from(value).map_err(|_| AppError::Internal(format!("Stored {label} is out of range")))
}

fn fit_i32(value: u32, label: &str) -> AppResult<i32> {
    i32::try_from(value).map_err(|_| AppError::Validation(format!("{label} is out of range")))
}

fn blank(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn subscription_from_row(row: SubscriptionRow) -> AppResult<SerialSubscription> {
    let length_kind = LengthKind::from_db_str(&row.length_kind).ok_or_else(|| {
        AppError::Internal(format!("Unknown subscription length '{}'", row.length_kind))
    })?;
    let status = SerialSubscriptionStatus::from_db_str(&row.status).ok_or_else(|| {
        AppError::Internal(format!("Unknown subscription status '{}'", row.status))
    })?;
    let unit = match row.frequency_unit.as_deref() {
        None => None,
        Some(value) => Some(
            FrequencyUnit::from_db_str(value)
                .ok_or_else(|| AppError::Internal(format!("Unknown frequency unit '{value}'")))?,
        ),
    };
    let spec = FrequencySpec {
        unit,
        issues_per_unit: row.issues_per_unit,
        units_per_issue: row.units_per_issue,
    };
    let effective_grace_days = row
        .grace_days
        .or_else(|| grace::default_grace_days(&spec).and_then(|days| i32::try_from(days).ok()));
    let mut irregularities = Vec::with_capacity(row.irregularities.len());
    for slot in row.irregularities {
        irregularities.push(non_negative_i32(slot, "irregularity")?);
    }
    Ok(SerialSubscription {
        id: row.id,
        biblio_id: row.biblio_id,
        biblio_title: row.biblio_title,
        issns: row.issns,
        site_id: row.site_id,
        vendor_id: row.vendor_id,
        fund_id: row.fund_id,
        cost: row.cost,
        issue_price_cents: row.issue_price_cents,
        frequency_id: row.frequency_id,
        numbering_pattern_id: row.numbering_pattern_id,
        first_issue_date: row.first_issue_date,
        start_date: row.start_date,
        end_date: row.end_date,
        x: row.value_x,
        y: row.value_y,
        z: row.value_z,
        inner_x: non_negative_i32(row.inner_x, "inner loop")?,
        inner_y: non_negative_i32(row.inner_y, "inner loop")?,
        inner_z: non_negative_i32(row.inner_z, "inner loop")?,
        next_slot: non_negative_i32(row.next_slot, "next slot")?,
        length: SubscriptionLength {
            kind: length_kind,
            count: non_negative_i32(row.length_count, "length")?,
        },
        irregularities,
        recurring_skips: row.recurring_skips.0,
        combined_issues: row.combined_issues.0,
        grace_days: row.grace_days,
        effective_grace_days,
        status,
        create_items: row.create_items,
        latest_issue_library_use_only: row.latest_issue_library_use_only,
        holdings_statement: row.holdings_statement,
        call_number: row.call_number,
        location: row.location,
        public_note: row.public_note,
        internal_note: row.internal_note,
        notes: row.notes,
        issue_count: row.issue_count,
        issues: Vec::new(),
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn issue_from_row(row: IssueRow) -> AppResult<SerialIssue> {
    let status = SerialIssueStatus::from_db_str(&row.status)
        .ok_or_else(|| AppError::Internal(format!("Unknown issue status '{}'", row.status)))?;
    Ok(SerialIssue {
        id: row.id,
        serial_subscription_id: row.serial_subscription_id,
        planned_date: row.planned_date,
        display_label: row.display_label,
        sequence: non_negative_i32(row.sequence, "sequence")?,
        slot: non_negative_i32(row.slot, "slot")?,
        x: row.value_x,
        y: row.value_y,
        z: row.value_z,
        status,
        supplement: row.supplement,
        item_id: row.item_id,
        received_at: row.received_at,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn page_window(query: &SerialSubscriptionQuery) -> (i64, i64, Option<String>) {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).clamp(1, 100);
    let q = query
        .q
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    (per_page, (page - 1) * per_page, q)
}

async fn insert_issues(
    tx: &mut Transaction<'_, Postgres>,
    subscription_id: i64,
    issues: &[NewSerialIssue],
) -> AppResult<()> {
    for issue in issues {
        sqlx::query(
            "INSERT INTO serial_issues ( \
                id, serial_subscription_id, planned_date, display_label, sequence, slot, \
                value_x, value_y, value_z, status, supplement, created_at, updated_at \
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'expected', false, now(), now())",
        )
        .bind(next_id())
        .bind(subscription_id)
        .bind(issue.planned_date)
        .bind(&issue.display_label)
        .bind(issue.sequence)
        .bind(issue.slot)
        .bind(issue.x)
        .bind(issue.y)
        .bind(issue.z)
        .execute(&mut **tx)
        .await
        .map_err(map_subscription_sql)?;
    }
    Ok(())
}

fn cursor_i32(cursor: &PredictionState) -> AppResult<(i32, i32, i32, i32)> {
    Ok((
        fit_i32(cursor.inner_x, "Inner loop")?,
        fit_i32(cursor.inner_y, "Inner loop")?,
        fit_i32(cursor.inner_z, "Inner loop")?,
        fit_i32(cursor.next_slot, "Next slot")?,
    ))
}

impl Repository {
    pub async fn periodical_biblio(&self, biblio_id: i64) -> AppResult<()> {
        let media_type: Option<String> = sqlx::query_scalar(
            "SELECT media_type FROM biblios WHERE id = $1 AND archived_at IS NULL",
        )
        .bind(biblio_id)
        .fetch_optional(&self.pool)
        .await?;
        match media_type.as_deref() {
            None => Err(AppError::NotFound(format!(
                "Bibliographic record {biblio_id} not found"
            ))),
            Some("periodic") => Ok(()),
            Some(_) => Err(AppError::Validation(
                "Serial subscriptions require a periodical bibliographic record".into(),
            )),
        }
    }

    pub async fn subscriptions_list(
        &self,
        query: &SerialSubscriptionQuery,
    ) -> AppResult<(Vec<SerialSubscription>, i64)> {
        let (limit, offset, q) = page_window(query);
        let status = query.status.map(SerialSubscriptionStatus::as_db_str);
        let total: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*)::bigint {SUBSCRIPTION_FROM} WHERE {SEARCH_WHERE}"
        ))
        .bind(q.as_deref())
        .bind(query.site_id)
        .bind(query.vendor_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;
        let columns = subscription_select();
        let sql = format!(
            "SELECT {columns} {SUBSCRIPTION_FROM} WHERE {SEARCH_WHERE} \
             ORDER BY lower(coalesce(b.title, '')), s.id \
             LIMIT $5 OFFSET $6"
        );
        let rows = sqlx::query_as::<_, SubscriptionRow>(&sql)
            .bind(q.as_deref())
            .bind(query.site_id)
            .bind(query.vendor_id)
            .bind(status)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let subscriptions = rows
            .into_iter()
            .map(subscription_from_row)
            .collect::<AppResult<Vec<_>>>()?;
        Ok((subscriptions, total))
    }

    pub async fn subscriptions_get(&self, id: i64) -> AppResult<SerialSubscription> {
        let columns = subscription_select();
        let sql = format!("SELECT {columns} {SUBSCRIPTION_FROM} WHERE s.id = $1");
        let row = sqlx::query_as::<_, SubscriptionRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Serial subscription {id} not found")))?;
        subscription_from_row(row)
    }

    pub async fn subscriptions_issues(&self, subscription_id: i64) -> AppResult<Vec<SerialIssue>> {
        let rows = sqlx::query_as::<_, IssueRow>(
            "SELECT id, serial_subscription_id, planned_date, display_label, sequence, slot, \
                    value_x, value_y, value_z, status, supplement, item_id, received_at, notes, \
                    created_at, updated_at \
             FROM serial_issues \
             WHERE serial_subscription_id = $1 \
             ORDER BY slot, sequence, id",
        )
        .bind(subscription_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(issue_from_row).collect()
    }

    pub async fn subscriptions_insert(
        &self,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<i64> {
        let id = next_id();
        let (inner_x, inner_y, inner_z, next_slot) = cursor_i32(cursor)?;
        let irregularities = irregularities_of(&data.irregularities)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO serial_subscriptions ( \
                id, biblio_id, site_id, vendor_id, fund_id, cost, issue_price_cents, \
                frequency_id, numbering_pattern_id, first_issue_date, start_date, end_date, \
                value_x, value_y, value_z, inner_x, inner_y, inner_z, next_slot, \
                length_kind, length_count, irregularities, recurring_skips, combined_issues, \
                grace_days, status, create_items, latest_issue_library_use_only, \
                holdings_statement, call_number, location, public_note, internal_note, notes, \
                created_at, updated_at \
             ) VALUES ( \
                $1, $2, $3, $4, $5, $6, $7, \
                $8, $9, $10, $11, $12, \
                $13, $14, $15, $16, $17, $18, $19, \
                $20, $21, $22, $23, $24, \
                $25, $26, $27, $28, \
                $29, $30, $31, $32, $33, $34, \
                now(), now())",
        )
        .bind(id)
        .bind(data.biblio_id)
        .bind(data.site_id)
        .bind(data.vendor_id)
        .bind(data.fund_id)
        .bind(data.cost)
        .bind(data.issue_price_cents)
        .bind(data.frequency_id)
        .bind(data.numbering_pattern_id)
        .bind(data.first_issue_date)
        .bind(data.start_date)
        .bind(data.end_date)
        .bind(cursor.x)
        .bind(cursor.y)
        .bind(cursor.z)
        .bind(inner_x)
        .bind(inner_y)
        .bind(inner_z)
        .bind(next_slot)
        .bind(data.length.kind.as_db_str())
        .bind(fit_i32(data.length.count, "Length")?)
        .bind(&irregularities)
        .bind(sqlx::types::Json(&data.recurring_skips))
        .bind(sqlx::types::Json(&data.combined_issues))
        .bind(data.grace_days)
        .bind(data.status.as_db_str())
        .bind(data.create_items)
        .bind(data.latest_issue_library_use_only)
        .bind(blank(&data.holdings_statement))
        .bind(blank(&data.call_number))
        .bind(blank(&data.location))
        .bind(blank(&data.public_note))
        .bind(blank(&data.internal_note))
        .bind(blank(&data.notes))
        .execute(&mut *tx)
        .await
        .map_err(map_subscription_sql)?;
        insert_issues(&mut tx, id, issues).await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn subscriptions_update(
        &self,
        id: i64,
        data: &SerialSubscriptionWrite,
        cursor: &PredictionState,
        new_issues: &[NewSerialIssue],
    ) -> AppResult<()> {
        let (inner_x, inner_y, inner_z, next_slot) = cursor_i32(cursor)?;
        let irregularities = irregularities_of(&data.irregularities)?;
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE serial_subscriptions SET \
                site_id = $2, vendor_id = $3, fund_id = $4, cost = $5, issue_price_cents = $6, \
                frequency_id = $7, numbering_pattern_id = $8, first_issue_date = $9, \
                start_date = $10, end_date = $11, \
                value_x = $12, value_y = $13, value_z = $14, \
                inner_x = $15, inner_y = $16, inner_z = $17, next_slot = $18, \
                length_kind = $19, length_count = $20, irregularities = $21, \
                recurring_skips = $22, combined_issues = $23, grace_days = $24, status = $25, \
                create_items = $26, latest_issue_library_use_only = $27, \
                holdings_statement = $28, call_number = $29, location = $30, \
                public_note = $31, internal_note = $32, notes = $33, updated_at = now() \
             WHERE id = $1",
        )
        .bind(id)
        .bind(data.site_id)
        .bind(data.vendor_id)
        .bind(data.fund_id)
        .bind(data.cost)
        .bind(data.issue_price_cents)
        .bind(data.frequency_id)
        .bind(data.numbering_pattern_id)
        .bind(data.first_issue_date)
        .bind(data.start_date)
        .bind(data.end_date)
        .bind(cursor.x)
        .bind(cursor.y)
        .bind(cursor.z)
        .bind(inner_x)
        .bind(inner_y)
        .bind(inner_z)
        .bind(next_slot)
        .bind(data.length.kind.as_db_str())
        .bind(fit_i32(data.length.count, "Length")?)
        .bind(&irregularities)
        .bind(sqlx::types::Json(&data.recurring_skips))
        .bind(sqlx::types::Json(&data.combined_issues))
        .bind(data.grace_days)
        .bind(data.status.as_db_str())
        .bind(data.create_items)
        .bind(data.latest_issue_library_use_only)
        .bind(blank(&data.holdings_statement))
        .bind(blank(&data.call_number))
        .bind(blank(&data.location))
        .bind(blank(&data.public_note))
        .bind(blank(&data.internal_note))
        .bind(blank(&data.notes))
        .execute(&mut *tx)
        .await
        .map_err(map_subscription_sql)?;
        if updated.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "Serial subscription {id} not found"
            )));
        }
        insert_issues(&mut tx, id, new_issues).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn subscriptions_append(
        &self,
        id: i64,
        expected_next_slot: i32,
        cursor: &PredictionState,
        issues: &[NewSerialIssue],
    ) -> AppResult<()> {
        let (inner_x, inner_y, inner_z, next_slot) = cursor_i32(cursor)?;
        let mut tx = self.pool.begin().await?;
        let current: Option<i32> = sqlx::query_scalar(
            "SELECT next_slot FROM serial_subscriptions WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(current) = current else {
            return Err(AppError::NotFound(format!(
                "Serial subscription {id} not found"
            )));
        };
        if current != expected_next_slot {
            return Err(AppError::Conflict(
                "The serial subscription changed; generate the horizon again".into(),
            ));
        }
        insert_issues(&mut tx, id, issues).await?;
        sqlx::query(
            "UPDATE serial_subscriptions SET \
                value_x = $2, value_y = $3, value_z = $4, \
                inner_x = $5, inner_y = $6, inner_z = $7, next_slot = $8, updated_at = now() \
             WHERE id = $1",
        )
        .bind(id)
        .bind(cursor.x)
        .bind(cursor.y)
        .bind(cursor.z)
        .bind(inner_x)
        .bind(inner_y)
        .bind(inner_z)
        .bind(next_slot)
        .execute(&mut *tx)
        .await
        .map_err(map_subscription_sql)?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn subscriptions_delete(&self, id: i64) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM serial_subscriptions WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(AppError::NotFound(format!(
                "Serial subscription {id} not found"
            )));
        }
        let blocking: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM serial_issues \
             WHERE serial_subscription_id = $1 \
               AND (status <> 'expected' OR item_id IS NOT NULL OR supplement)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if blocking > 0 {
            return Err(AppError::Conflict(
                "This serial subscription has received or supplemental issues".into(),
            ));
        }
        sqlx::query(
            "DELETE FROM serial_issues \
             WHERE serial_subscription_id = $1 AND status = 'expected' AND item_id IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_subscription_sql)?;
        sqlx::query("DELETE FROM serial_subscriptions WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(map_subscription_sql)?;
        tx.commit().await?;
        Ok(())
    }
}

fn irregularities_of(slots: &[u32]) -> AppResult<Vec<i32>> {
    slots
        .iter()
        .map(|slot| fit_i32(*slot, "Irregularity slot"))
        .collect()
}
