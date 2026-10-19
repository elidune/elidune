//! Lenders and incoming deposit batches.
//!
//! A deposit batch is a set of copies lent to us. `items.deposit_batch_id`
//! NULL means own stock. `items.source_id` stays the site where the copy sits.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// Lifecycle of one incoming batch. Stored as these exact strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum DepositBatchStatus {
    #[default]
    Active,
    Returning,
    Closed,
}

impl DepositBatchStatus {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Returning => "returning",
            Self::Closed => "closed",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "returning" => Some(Self::Returning),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for DepositBatchStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for DepositBatchStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown deposit batch status: {s}").into())
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for DepositBatchStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str().to_string(), buf)
    }
}

/// Where a deposit copy sits inside its batch. Used by the batch item list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum DepositCopyState {
    OnShelf,
    OnLoan,
    OnHold,
    InTransit,
    Returned,
    /// Archived without a scan-out, or declared missing. Empty until a batch is closed.
    Missing,
    /// Still in the batch, but not on the shelf (lost, damaged, not borrowable, …).
    Unavailable,
}

impl DepositCopyState {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::OnShelf => "onShelf",
            Self::OnLoan => "onLoan",
            Self::OnHold => "onHold",
            Self::InTransit => "inTransit",
            Self::Returned => "returned",
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "onShelf" => Some(Self::OnShelf),
            "onLoan" => Some(Self::OnLoan),
            "onHold" => Some(Self::OnHold),
            "inTransit" => Some(Self::InTransit),
            "returned" => Some(Self::Returned),
            "missing" => Some(Self::Missing),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }

    /// States accepted by `GET /deposit-batches/{id}/items?state=`.
    #[must_use]
    pub fn from_filter(value: &str) -> Option<Self> {
        match Self::from_db_str(value) {
            Some(Self::Unavailable) => None,
            other => other,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for DepositCopyState {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for DepositCopyState {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown deposit copy state: {s}").into())
    }
}

/// Library that lends copies to us. Not a [`crate::models::source::Source`].
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Lender {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub name: String,
    pub contact: Option<String>,
    pub notes: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateLender {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(max = 500))]
    pub contact: Option<String>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLender {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[validate(length(max = 500))]
    pub contact: Option<String>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

/// One incoming batch of copies from a lender.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositBatch {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub lender_id: i64,
    pub reference: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub site_id: i64,
    pub arrived_on: NaiveDate,
    pub return_due_on: NaiveDate,
    pub status: DepositBatchStatus,
    pub closed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Copy counts for `GET /deposit-batches/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositBatchCounts {
    pub total: i64,
    pub on_shelf: i64,
    pub on_loan: i64,
    pub on_hold: i64,
    pub in_transit: i64,
    pub returned: i64,
}

/// Batch plus counts. List rows do not include counts.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositBatchDetail {
    #[serde(flatten)]
    pub batch: DepositBatch,
    pub counts: DepositBatchCounts,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDepositBatch {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub lender_id: i64,
    #[validate(length(min = 1, max = 200))]
    pub reference: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub site_id: i64,
    pub arrived_on: NaiveDate,
    pub return_due_on: NaiveDate,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

/// Fields that can change while the batch is `active`.
/// Changing `siteId` does not move copies.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDepositBatch {
    pub return_due_on: Option<NaiveDate>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct DepositBatchListQuery {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub lender_id: Option<i64>,
    pub status: Option<DepositBatchStatus>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct DepositBatchItemsQuery {
    /// `onLoan`, `onHold`, `inTransit`, `onShelf`, `returned`, or `missing`.
    pub state: Option<String>,
}

/// One copy on `GET /deposit-batches/{id}/items`.
#[serde_as]
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositBatchItem {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    pub barcode: Option<String>,
    pub title: Option<String>,
    pub state: DepositCopyState,
}

/// Deposit block embedded on item JSON. `null` means own stock.
///
/// Loaded with the item query (one join), never a query per copy.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemDeposit {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub lender_id: i64,
    pub lender_name: String,
    pub reference: String,
    pub return_due_on: NaiveDate,
    pub batch_status: DepositBatchStatus,
}

/// `items.deposit_batch_id IS NULL`. Own stock, and the predicate later
/// exclusion rules share. Callers pass the items table alias.
#[must_use]
pub fn own_stock_sql(item_alias: &str) -> String {
    format!("{item_alias}.deposit_batch_id IS NULL")
}

/// JSON object for [`ItemDeposit`], or JSON null for own stock.
///
/// The value is jsonb, never SQL NULL, so `#[sqlx(json)] Option<ItemDeposit>`
/// decodes own stock as `None`.
pub const ITEM_DEPOSIT_JSON: &str = r#"
CASE WHEN deposit_batch.id IS NULL THEN 'null'::jsonb
ELSE jsonb_build_object(
    'batchId', deposit_batch.id::text,
    'lenderId', deposit_batch.lender_id::text,
    'lenderName', deposit_lender.name,
    'reference', deposit_batch.reference,
    'returnDueOn', to_char(deposit_batch.return_due_on, 'YYYY-MM-DD'),
    'batchStatus', deposit_batch.status
) END
"#;

/// Joins that back [`ITEM_DEPOSIT_JSON`]. One join pair per item query.
#[must_use]
pub fn item_deposit_join(item_alias: &str) -> String {
    format!(
        "LEFT JOIN deposit_batches deposit_batch ON deposit_batch.id = {item_alias}.deposit_batch_id \
         LEFT JOIN lenders deposit_lender ON deposit_lender.id = deposit_batch.lender_id"
    )
}

/// Decode a `deposit` jsonb column. NULL and a decode failure are own-stock `None`
/// only for NULL; a present object that does not match [`ItemDeposit`] is an error.
pub fn item_deposit_from_value(
    value: Option<serde_json::Value>,
) -> Result<Option<ItemDeposit>, serde_json::Error> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value).map(Some),
    }
}
