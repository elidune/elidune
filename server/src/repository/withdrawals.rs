//! `item_withdrawals` — weeding events (#139).

use chrono::{DateTime, Utc};
use snowflaked::Generator;
use sqlx::{postgres::PgArguments, Arguments, FromRow, Postgres, Transaction};

use crate::{
    error::{error_code, AppError, AppResult},
    models::withdrawal::{
        DestinationTotal, ItemWithdrawal, ReasonTotal, WithdrawalCsvSource, WithdrawalDestination,
        WithdrawalDisposeFailure, WithdrawalList, WithdrawalQuery, WithdrawalReason,
        WithdrawalWrite,
    },
};

use super::Repository;

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(11)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate::<i64>()
}

const SELECT_WITHDRAWAL: &str = r#"
    SELECT
        w.id,
        w.item_id,
        w.withdrawn_at,
        w.reason,
        w.reason_detail,
        w.destination,
        w.agent_id,
        NULLIF(btrim(concat_ws(' ', ag.firstname, ag.lastname)), '') AS agent_name,
        w.cancelled_at,
        w.cancelled_by,
        w.disposed_at,
        w.disposed_by,
        NULLIF(btrim(concat_ws(' ', dp.firstname, dp.lastname)), '') AS disposed_by_name,
        w.legacy,
        i.barcode,
        i.call_number,
        i.biblio_id,
        b.title AS biblio_title,
        i.source_id AS site_id,
        so.name AS site_name
    FROM item_withdrawals w
    JOIN items i ON i.id = w.item_id
    LEFT JOIN biblios b ON b.id = i.biblio_id
    LEFT JOIN sources so ON so.id = i.source_id
    LEFT JOIN users ag ON ag.id = w.agent_id
    LEFT JOIN users dp ON dp.id = w.disposed_by
"#;

const SELECT_WITHDRAWAL_CSV: &str = r#"
    SELECT
        w.withdrawn_at,
        w.reason,
        w.reason_detail,
        w.destination,
        NULLIF(btrim(concat_ws(' ', ag.firstname, ag.lastname)), '') AS agent_name,
        w.disposed_at,
        NULLIF(btrim(concat_ws(' ', dp.firstname, dp.lastname)), '') AS disposed_by_name,
        w.legacy,
        i.barcode,
        i.call_number,
        b.title AS biblio_title,
        NULLIF(btrim((
            SELECT string_agg(
                btrim(concat_ws(' ', a.lastname, NULLIF(btrim(a.firstname), ''))),
                ', ' ORDER BY ba.position, a.id
            )
            FROM biblio_authors ba
            JOIN authors a ON a.id = ba.author_id
            WHERE ba.biblio_id = i.biblio_id
        )), '') AS authors,
        so.name AS site_name,
        i.created_at AS acquired_at,
        i.price AS purchase_price
    FROM item_withdrawals w
    JOIN items i ON i.id = w.item_id
    LEFT JOIN biblios b ON b.id = i.biblio_id
    LEFT JOIN sources so ON so.id = i.source_id
    LEFT JOIN users ag ON ag.id = w.agent_id
    LEFT JOIN users dp ON dp.id = w.disposed_by
"#;

#[derive(FromRow)]
struct CancelLock {
    cancelled_at: Option<DateTime<Utc>>,
    disposed_at: Option<DateTime<Utc>>,
    item_id: i64,
}

#[derive(FromRow)]
struct DisposalLock {
    cancelled_at: Option<DateTime<Utc>>,
    disposed_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct WithdrawalRow {
    id: i64,
    item_id: i64,
    withdrawn_at: DateTime<Utc>,
    reason: WithdrawalReason,
    reason_detail: Option<String>,
    destination: Option<WithdrawalDestination>,
    agent_id: Option<i64>,
    agent_name: Option<String>,
    cancelled_at: Option<DateTime<Utc>>,
    cancelled_by: Option<i64>,
    disposed_at: Option<DateTime<Utc>>,
    disposed_by: Option<i64>,
    disposed_by_name: Option<String>,
    legacy: bool,
    barcode: Option<String>,
    call_number: Option<String>,
    biblio_id: Option<i64>,
    biblio_title: Option<String>,
    site_id: Option<i64>,
    site_name: Option<String>,
}

impl From<WithdrawalRow> for ItemWithdrawal {
    fn from(row: WithdrawalRow) -> Self {
        Self {
            id: row.id,
            item_id: row.item_id,
            withdrawn_at: row.withdrawn_at,
            reason: row.reason,
            reason_detail: row.reason_detail,
            destination: row.destination,
            agent_id: row.agent_id,
            agent_name: row.agent_name,
            cancelled_at: row.cancelled_at,
            cancelled_by: row.cancelled_by,
            disposed_at: row.disposed_at,
            disposed_by: row.disposed_by,
            disposed_by_name: row.disposed_by_name,
            legacy: row.legacy,
            barcode: row.barcode,
            call_number: row.call_number,
            biblio_id: row.biblio_id,
            biblio_title: row.biblio_title,
            site_id: row.site_id,
            site_name: row.site_name,
        }
    }
}

enum Param {
    Text(String),
    I64(i64),
    Time(DateTime<Utc>),
}

struct PreparedFilter {
    where_sql: String,
    params: Vec<Param>,
}

fn like_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn prepare_filter(query: &WithdrawalQuery, export: bool) -> AppResult<PreparedFilter> {
    let mut parts = Vec::new();
    let mut params = Vec::new();

    parts.push(crate::models::deposit::own_stock_sql("i"));
    if !query.include_cancelled.unwrap_or(false) {
        parts.push("w.cancelled_at IS NULL".to_string());
    }
    if let Some(disposed) = query.disposed {
        parts.push(if disposed {
            "w.disposed_at IS NOT NULL".to_string()
        } else {
            "w.disposed_at IS NULL".to_string()
        });
    }
    if let Some(from) = parse_bound(query.from.as_deref(), "from")? {
        params.push(Param::Time(from));
        parts.push(format!("w.withdrawn_at >= ${}", params.len()));
    } else if export {
        return Err(AppError::Validation(
            "Export requires from and to (RFC3339)".into(),
        ));
    }
    if let Some(to) = parse_bound(query.to.as_deref(), "to")? {
        params.push(Param::Time(to));
        parts.push(format!("w.withdrawn_at <= ${}", params.len()));
    } else if export {
        return Err(AppError::Validation(
            "Export requires from and to (RFC3339)".into(),
        ));
    }
    if let Some(site_id) = query.site_id {
        params.push(Param::I64(site_id));
        parts.push(format!("i.source_id = ${}", params.len()));
    }
    if let Some(reason) = query.reason {
        params.push(Param::Text(reason.as_db_str().to_string()));
        parts.push(format!("w.reason = ${}", params.len()));
    }
    if let Some(destination) = query
        .destination
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if destination == "unset" {
            parts.push("w.destination IS NULL".to_string());
        } else {
            let Some(parsed) = WithdrawalDestination::from_db_str(destination) else {
                return Err(AppError::Validation(
                    "destination must be pulp, donation, sale, deposit, or unset".into(),
                ));
            };
            params.push(Param::Text(parsed.as_db_str().to_string()));
            parts.push(format!("w.destination = ${}", params.len()));
        }
    }
    if let Some(collection_id) = query.collection_id {
        params.push(Param::I64(collection_id));
        parts.push(format!(
            "EXISTS (SELECT 1 FROM biblio_collections bc WHERE bc.biblio_id = i.biblio_id AND bc.collection_id = ${})",
            params.len()
        ));
    }
    if let Some(call_number) = query
        .call_number
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        params.push(Param::Text(format!("%{}%", like_escape(call_number))));
        parts.push(format!(
            "unaccent(lower(i.call_number)) LIKE unaccent(lower(${}))",
            params.len()
        ));
    }

    let where_sql = if parts.is_empty() {
        "TRUE".to_string()
    } else {
        parts.join(" AND ")
    };
    Ok(PreparedFilter { where_sql, params })
}

fn parse_bound(raw: Option<&str>, name: &str) -> AppResult<Option<DateTime<Utc>>> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|dt| Some(dt.with_timezone(&Utc)))
        .map_err(|_| AppError::Validation(format!("{name} must be an RFC3339 timestamp")))
}

fn bind_params(params: &[Param]) -> PgArguments {
    let mut args = PgArguments::default();
    for param in params {
        match param {
            Param::Text(value) => {
                args.add(value);
            }
            Param::I64(value) => {
                args.add(*value);
            }
            Param::Time(value) => {
                args.add(*value);
            }
        }
    }
    args
}

fn unique_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

impl Repository {
    async fn load_withdrawal(&self, id: i64) -> AppResult<ItemWithdrawal> {
        let sql = format!("{SELECT_WITHDRAWAL} WHERE w.id = $1");
        let row = sqlx::query_as::<_, WithdrawalRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Withdrawal {id} not found")))?;
        Ok(row.into())
    }

    pub async fn item_withdrawals_get(&self, id: i64) -> AppResult<ItemWithdrawal> {
        self.load_withdrawal(id).await
    }

    /// The non-cancelled withdrawal for a copy, if one exists.
    pub async fn item_withdrawals_active_for_item(
        &self,
        item_id: i64,
    ) -> AppResult<ItemWithdrawal> {
        let sql = format!(
            "{SELECT_WITHDRAWAL} WHERE w.item_id = $1 AND w.cancelled_at IS NULL ORDER BY w.withdrawn_at DESC, w.id DESC LIMIT 1"
        );
        let row = sqlx::query_as::<_, WithdrawalRow>(&sql)
            .bind(item_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Item {item_id} has no active withdrawal"))
            })?;
        Ok(row.into())
    }

    async fn load_withdrawals_ordered(&self, ids: &[i64]) -> AppResult<Vec<ItemWithdrawal>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "{SELECT_WITHDRAWAL} WHERE w.id = ANY($1) ORDER BY array_position($1::bigint[], w.id)"
        );
        let rows = sqlx::query_as::<_, WithdrawalRow>(&sql)
            .bind(ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ItemWithdrawal::from).collect())
    }

    async fn withdraw_one_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        item_id: i64,
        agent_id: i64,
        input: &WithdrawalWrite,
    ) -> AppResult<i64> {
        let locked: Option<(Option<i64>, String, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT biblio_id, weeding_status, deposit_batch_id
            FROM items
            WHERE id = $1 AND archived_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(item_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((biblio_id, weeding_status, deposit_batch_id)) = locked else {
            return Err(AppError::NotFound(format!("Item {item_id} not found")));
        };
        if deposit_batch_id.is_some() {
            return Err(AppError::coded(
                422,
                error_code::DEPOSIT_ITEM_NOT_OWNED,
                "A deposit copy is not part of the library's own stock",
            ));
        }

        let on_loan: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM loans WHERE item_id = $1 AND returned_at IS NULL)",
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        if on_loan {
            return Err(AppError::coded(
                422,
                error_code::ITEM_ON_LOAN,
                format!("Item {item_id} is on loan and cannot be withdrawn"),
            ));
        }

        let in_transit: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item_transits WHERE item_id = $1 AND status IN ('requested', 'in_transit'))",
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        if in_transit {
            return Err(AppError::coded(
                422,
                error_code::ITEM_IN_TRANSIT,
                format!("Item {item_id} is in transit and cannot be withdrawn"),
            ));
        }

        let on_hold: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM holds WHERE item_id = $1 AND status IN ('to_pick', 'ready'))",
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        if on_hold {
            return Err(AppError::coded(
                422,
                error_code::ITEM_ON_HOLD,
                format!("Item {item_id} is on hold for pickup and cannot be withdrawn"),
            ));
        }

        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM item_withdrawals WHERE item_id = $1 AND cancelled_at IS NULL)",
        )
        .bind(item_id)
        .fetch_one(&mut **tx)
        .await?;
        if active || weeding_status == "withdrawn" {
            return Err(AppError::coded(
                409,
                error_code::WITHDRAWAL_ALREADY_ACTIVE,
                format!("Item {item_id} already has an active withdrawal"),
            ));
        }

        let destination = input.destination.ok_or_else(|| {
            AppError::coded(
                400,
                error_code::WITHDRAWAL_DESTINATION_REQUIRED,
                "destination is required",
            )
        })?;
        let id = next_id();
        let inserted = sqlx::query(
            r#"
            INSERT INTO item_withdrawals (
                id, item_id, withdrawn_at, reason, reason_detail, destination, agent_id, legacy
            )
            VALUES ($1, $2, NOW(), $3, $4, $5, $6, FALSE)
            "#,
        )
        .bind(id)
        .bind(item_id)
        .bind(input.reason)
        .bind(&input.reason_detail)
        .bind(destination)
        .bind(agent_id)
        .execute(&mut **tx)
        .await;
        if let Err(err) = inserted {
            if unique_violation(&err) {
                return Err(AppError::coded(
                    409,
                    error_code::WITHDRAWAL_ALREADY_ACTIVE,
                    format!("Item {item_id} already has an active withdrawal"),
                ));
            }
            return Err(err.into());
        }

        sqlx::query(
            r#"
            UPDATE items
            SET weeding_status = 'withdrawn',
                weeding_reason = NULL,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .execute(&mut **tx)
        .await?;

        let cancelled = self.holds_cancel_active_for_item_tx(tx, item_id).await?;
        if cancelled > 0 {
            if let Some(biblio_id) = biblio_id {
                self.holds_notify_next_available_copy_tx(tx, biblio_id, Some(item_id))
                    .await?;
            }
        }

        Ok(id)
    }

    pub async fn item_withdrawals_create(
        &self,
        item_id: i64,
        agent_id: i64,
        input: &WithdrawalWrite,
    ) -> AppResult<ItemWithdrawal> {
        let mut tx = self.pool.begin().await?;
        let id = self
            .withdraw_one_tx(&mut tx, item_id, agent_id, input)
            .await?;
        tx.commit().await?;
        self.load_withdrawal(id).await
    }

    pub async fn item_withdrawals_create_many(
        &self,
        item_ids: &[i64],
        agent_id: i64,
        input: &WithdrawalWrite,
    ) -> AppResult<Vec<ItemWithdrawal>> {
        let mut tx = self.pool.begin().await?;
        let mut ids = Vec::with_capacity(item_ids.len());
        for item_id in item_ids {
            ids.push(
                self.withdraw_one_tx(&mut tx, *item_id, agent_id, input)
                    .await?,
            );
        }
        tx.commit().await?;
        self.load_withdrawals_ordered(&ids).await
    }

    pub async fn item_withdrawals_cancel(
        &self,
        id: i64,
        agent_id: i64,
    ) -> AppResult<ItemWithdrawal> {
        let mut tx = self.pool.begin().await?;
        let state = sqlx::query_as::<_, CancelLock>(
            "SELECT cancelled_at, disposed_at, item_id FROM item_withdrawals WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(CancelLock {
            cancelled_at,
            disposed_at,
            item_id,
        }) = state
        else {
            return Err(AppError::NotFound(format!("Withdrawal {id} not found")));
        };
        if disposed_at.is_some() {
            return Err(AppError::coded(
                409,
                error_code::WITHDRAWAL_ALREADY_DISPOSED,
                format!("Withdrawal {id} is already disposed and cannot be cancelled"),
            ));
        }
        if cancelled_at.is_some() {
            return Err(AppError::coded(
                409,
                error_code::WITHDRAWAL_NOT_ACTIVE,
                format!("Withdrawal {id} is already cancelled"),
            ));
        }
        sqlx::query(
            "UPDATE item_withdrawals SET cancelled_at = NOW(), cancelled_by = $2 WHERE id = $1",
        )
        .bind(id)
        .bind(agent_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE items
            SET weeding_status = 'on_shelf',
                weeding_reason = NULL,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.load_withdrawal(id).await
    }

    /// Mark every id disposed, or none. Locks rows in id order.
    pub async fn item_withdrawals_dispose(
        &self,
        ids: &[i64],
        agent_id: i64,
        disposed_at: DateTime<Utc>,
    ) -> AppResult<Vec<ItemWithdrawal>> {
        let mut tx = self.pool.begin().await?;
        let mut lock_order = ids.to_vec();
        lock_order.sort_unstable();
        let mut locked: std::collections::HashMap<i64, DisposalLock> =
            std::collections::HashMap::new();
        for id in lock_order {
            let row = sqlx::query_as::<_, DisposalLock>(
                "SELECT cancelled_at, disposed_at FROM item_withdrawals WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(state) = row {
                locked.insert(id, state);
            }
        }

        let mut failures = Vec::new();
        for id in ids {
            let Some(row) = locked.get(id) else {
                failures.push(WithdrawalDisposeFailure {
                    id: *id,
                    code: error_code::NOT_FOUND.to_string(),
                    message: format!("Withdrawal {id} not found"),
                });
                continue;
            };
            if row.cancelled_at.is_some() {
                failures.push(WithdrawalDisposeFailure {
                    id: *id,
                    code: error_code::WITHDRAWAL_NOT_ACTIVE.to_string(),
                    message: format!("Withdrawal {id} is cancelled"),
                });
                continue;
            }
            if row.disposed_at.is_some() {
                failures.push(WithdrawalDisposeFailure {
                    id: *id,
                    code: error_code::WITHDRAWAL_ALREADY_DISPOSED.to_string(),
                    message: format!("Withdrawal {id} is already disposed"),
                });
            }
        }
        if !failures.is_empty() {
            let status = if failures
                .iter()
                .any(|failure| failure.code == error_code::NOT_FOUND)
            {
                422
            } else {
                409
            };
            return Err(AppError::WithdrawalDisposeRejected {
                status,
                failures: Box::new(failures),
            });
        }

        for id in ids {
            sqlx::query(
                "UPDATE item_withdrawals SET disposed_at = $2, disposed_by = $3 WHERE id = $1",
            )
            .bind(id)
            .bind(disposed_at)
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.load_withdrawals_ordered(ids).await
    }

    pub async fn item_withdrawals_complete(
        &self,
        id: i64,
        input: &WithdrawalWrite,
    ) -> AppResult<ItemWithdrawal> {
        let destination = input.destination.ok_or_else(|| {
            AppError::coded(
                400,
                error_code::WITHDRAWAL_DESTINATION_REQUIRED,
                "destination is required",
            )
        })?;
        let mut tx = self.pool.begin().await?;
        let state: Option<(Option<DateTime<Utc>>, bool)> = sqlx::query_as(
            "SELECT cancelled_at, legacy FROM item_withdrawals WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((cancelled_at, legacy)) = state else {
            return Err(AppError::NotFound(format!("Withdrawal {id} not found")));
        };
        if cancelled_at.is_some() {
            return Err(AppError::coded(
                409,
                error_code::WITHDRAWAL_NOT_ACTIVE,
                format!("Withdrawal {id} is already cancelled"),
            ));
        }
        if !legacy {
            return Err(AppError::coded(
                422,
                error_code::WITHDRAWAL_NOT_LEGACY,
                "Only a legacy withdrawal can be completed later",
            ));
        }
        sqlx::query(
            r#"
            UPDATE item_withdrawals
            SET reason = $2,
                reason_detail = $3,
                destination = $4
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(input.reason)
        .bind(&input.reason_detail)
        .bind(destination)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.load_withdrawal(id).await
    }

    pub async fn item_withdrawals_list(
        &self,
        query: &WithdrawalQuery,
    ) -> AppResult<WithdrawalList> {
        let prepared = prepare_filter(query, false)?;
        let page = query.page.unwrap_or(1).max(1);
        let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * per_page;

        let count_sql = format!(
            r#"
            SELECT COUNT(*)::bigint
            FROM item_withdrawals w
            JOIN items i ON i.id = w.item_id
            WHERE {}
            "#,
            prepared.where_sql
        );
        let total: i64 = sqlx::query_scalar_with(&count_sql, bind_params(&prepared.params))
            .fetch_one(&self.pool)
            .await?;

        let mut page_params = Vec::new();
        page_params.extend(prepared.params.iter().map(clone_param));
        page_params.push(Param::I64(per_page));
        let limit_idx = page_params.len();
        page_params.push(Param::I64(offset));
        let offset_idx = page_params.len();
        let page_sql = format!(
            "{SELECT_WITHDRAWAL} WHERE {} ORDER BY w.withdrawn_at DESC, w.id DESC LIMIT ${limit_idx} OFFSET ${offset_idx}",
            prepared.where_sql
        );
        let rows = sqlx::query_as_with::<_, WithdrawalRow, _>(&page_sql, bind_params(&page_params))
            .fetch_all(&self.pool)
            .await?;
        let items = rows.into_iter().map(ItemWithdrawal::from).collect();

        let reason_sql = format!(
            r#"
            SELECT w.reason, COUNT(*)::bigint AS count
            FROM item_withdrawals w
            JOIN items i ON i.id = w.item_id
            WHERE {}
            GROUP BY w.reason
            "#,
            prepared.where_sql
        );
        let reason_rows: Vec<(WithdrawalReason, i64)> =
            sqlx::query_as_with(&reason_sql, bind_params(&prepared.params))
                .fetch_all(&self.pool)
                .await?;
        let mut totals_by_reason: Vec<ReasonTotal> = reason_rows
            .into_iter()
            .map(|(reason, count)| ReasonTotal { reason, count })
            .collect();
        totals_by_reason.sort_by_key(|row| reason_rank(row.reason));

        let destination_sql = format!(
            r#"
            SELECT w.destination, COUNT(*)::bigint AS count
            FROM item_withdrawals w
            JOIN items i ON i.id = w.item_id
            WHERE {}
            GROUP BY w.destination
            "#,
            prepared.where_sql
        );
        let destination_rows: Vec<(Option<WithdrawalDestination>, i64)> =
            sqlx::query_as_with(&destination_sql, bind_params(&prepared.params))
                .fetch_all(&self.pool)
                .await?;
        let mut totals_by_destination: Vec<DestinationTotal> = destination_rows
            .into_iter()
            .map(|(destination, count)| DestinationTotal { destination, count })
            .collect();
        totals_by_destination.sort_by_key(|row| destination_rank(row.destination));

        Ok(WithdrawalList {
            items,
            total,
            page,
            per_page,
            totals_by_reason,
            totals_by_destination,
        })
    }

    pub async fn item_withdrawals_export_rows(
        &self,
        query: &WithdrawalQuery,
    ) -> AppResult<Vec<WithdrawalCsvSource>> {
        let prepared = prepare_filter(query, true)?;
        let sql = format!(
            "{SELECT_WITHDRAWAL_CSV} WHERE {} ORDER BY w.withdrawn_at ASC, w.id ASC LIMIT 100000",
            prepared.where_sql
        );
        let rows =
            sqlx::query_as_with::<_, WithdrawalCsvSource, _>(&sql, bind_params(&prepared.params))
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn user_preferred_language(&self, user_id: i64) -> AppResult<Option<String>> {
        let language: Option<Option<String>> =
            sqlx::query_scalar("SELECT language FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(language.flatten())
    }
}

fn clone_param(param: &Param) -> Param {
    match param {
        Param::Text(value) => Param::Text(value.clone()),
        Param::I64(value) => Param::I64(*value),
        Param::Time(value) => Param::Time(*value),
    }
}

fn reason_rank(reason: WithdrawalReason) -> u8 {
    match reason {
        WithdrawalReason::Worn => 0,
        WithdrawalReason::Obsolete => 1,
        WithdrawalReason::NewEdition => 2,
        WithdrawalReason::Surplus => 3,
        WithdrawalReason::NotBorrowed => 4,
        WithdrawalReason::OutOfPolicy => 5,
        WithdrawalReason::LostNotReplaced => 6,
        WithdrawalReason::Other => 7,
    }
}

fn destination_rank(destination: Option<WithdrawalDestination>) -> u8 {
    match destination {
        Some(WithdrawalDestination::Pulp) => 0,
        Some(WithdrawalDestination::Donation) => 1,
        Some(WithdrawalDestination::Sale) => 2,
        Some(WithdrawalDestination::Deposit) => 3,
        None => 4,
    }
}
