//! Lenders and incoming deposit batches.
//!
//! A deposit batch is a set of copies lent to us. `items.deposit_batch_id`
//! NULL means own stock. `items.site_id` stays the site where the copy sits.

use crate::models::fine::Cents;
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
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown deposit copy state: {s}").into())
    }
}

/// Library that lends copies to us. Not a [`crate::models::site::Site`].
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
    /// Replacement bill in integer cents when a deposit copy is lost or damaged.
    /// Null means staff must enter the amount. Never zero.
    pub replacement_rate_cents: Option<Cents>,
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
    /// Integer cents. Omit to leave the lender without a rate. Zero is rejected.
    #[serde(default)]
    pub replacement_rate_cents: Option<Cents>,
}

#[derive(Debug, Clone, Default, Deserialize, Validate, ToSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateLender {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[validate(length(max = 500))]
    pub contact: Option<String>,
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
    /// Integer cents. Omit the field to leave the stored rate unchanged.
    /// JSON `null` clears it. Zero and negative values are rejected.
    #[serde(default, with = "serde_with::rust::double_option")]
    #[schema(value_type = Option<i64>)]
    pub replacement_rate_cents: Option<Option<Cents>>,
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
    /// Null means the batch has no fixed return date yet (permanent deposit).
    pub return_due_on: Option<NaiveDate>,
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
    /// Omit for a permanent deposit; set later via `PATCH /deposit-batches/{id}`.
    pub return_due_on: Option<NaiveDate>,
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
    pub return_due_on: Option<NaiveDate>,
    pub batch_status: DepositBatchStatus,
    /// When staff recorded that the lender was told this copy was lost or damaged.
    /// Null until `POST /items/{id}/deposit-loss-notified`.
    #[serde(default)]
    pub deposit_loss_notified_at: Option<DateTime<Utc>>,
}

/// Deposit block on reader and OPAC item JSON. Batch return date is staff-only.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemDepositPublic {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub lender_id: i64,
    pub lender_name: String,
    pub reference: String,
    pub batch_status: DepositBatchStatus,
}

impl From<ItemDeposit> for ItemDepositPublic {
    fn from(deposit: ItemDeposit) -> Self {
        Self {
            batch_id: deposit.batch_id,
            lender_id: deposit.lender_id,
            lender_name: deposit.lender_name,
            reference: deposit.reference,
            batch_status: deposit.batch_status,
        }
    }
}

/// `items.deposit_batch_id IS NULL`. Own stock, and the predicate exclusion
/// rules share. Callers pass the items table alias.
#[must_use]
pub fn own_stock_sql(item_alias: &str) -> String {
    format!("{item_alias}.deposit_batch_id IS NULL")
}

/// True when copy alias `i` was scanned out or checked in for lender return (`deposit_returned_at`).
#[must_use]
pub fn deposit_copy_returned_to_lender_sql(item_alias: &str) -> String {
    format!("{item_alias}.deposit_returned_at IS NOT NULL")
}

/// True when copy alias `i` must not be assigned to a hold.
///
/// The batch is `returning` or `closed`, or `return_due_on` is before today
/// in the library timezone. Batches without a return date are holdable while active.
/// Own stock (`deposit_batch_id` NULL) is holdable.
pub const DEPOSIT_COPY_NOT_HOLDABLE_SQL: &str = r#"
EXISTS (
    SELECT 1 FROM deposit_batches deposit_hold_batch
    WHERE deposit_hold_batch.id = i.deposit_batch_id
      AND (
          deposit_hold_batch.status IN ('returning', 'closed')
          OR (
              deposit_hold_batch.return_due_on IS NOT NULL
              AND deposit_hold_batch.return_due_on < (
                  CURRENT_TIMESTAMP AT TIME ZONE COALESCE(
                      (SELECT li.timezone FROM library_info li WHERE li.id = 1),
                      'Europe/Paris'
                  )
              )::date
          )
      )
)
"#;

/// One copy on the batch close report. Barcode and title are for the lender.
#[serde_as]
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositCloseCopy {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: Option<String>,
    pub title: Option<String>,
}

/// `POST /deposit-batches/{id}/close`. `{ returned, missing }`.
///
/// Close is refused while any copy is on loan, so those copies are not in this report.
#[derive(Debug, Clone, Serialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositCloseReport {
    pub returned: Vec<DepositCloseCopy>,
    pub missing: Vec<DepositCloseCopy>,
}

/// One copy still on loan. Listed on `422 deposit_batch_has_active_loans`.
#[serde_as]
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositActiveLoanCopy {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: Option<String>,
}

/// `POST /deposit-batches/{id}/scan-out`.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositScanOutRequest {
    #[validate(length(min = 1, max = 64))]
    pub barcode: String,
}

/// Copy marked back to the lender. A second scan keeps the first timestamp.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositScanOut {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: String,
    pub deposit_returned_at: DateTime<Utc>,
    pub already_scanned: bool,
}

/// `POST /deposit-batches/scan-out` — same as [`DepositScanOut`] plus resolved batch context.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositGlobalScanOut {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: String,
    pub deposit_returned_at: DateTime<Utc>,
    pub already_scanned: bool,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub deposit_batch_id: i64,
    pub batch_reference: String,
    pub lender_name: String,
}

/// Why a deposit copy was billed back to the lender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum DepositLossKind {
    Lost,
    Damaged,
}

impl DepositLossKind {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Lost => "lost",
            Self::Damaged => "damaged",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "lost" => Some(Self::Lost),
            "damaged" => Some(Self::Damaged),
            _ => None,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for DepositLossKind {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for DepositLossKind {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown deposit loss kind: {s}").into())
    }
}

/// One deposit copy declared lost or damaged. `GET /deposit-losses`.
#[serde_as]
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepositLoss {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub barcode: Option<String>,
    pub title: Option<String>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub lender_id: i64,
    pub lender_name: String,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub batch_id: i64,
    /// Batch agreement reference.
    pub reference: String,
    pub kind: DepositLossKind,
    /// Frozen fine amount in integer cents.
    pub amount_cents: Cents,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub fine_id: i64,
    pub declared_at: DateTime<Utc>,
    pub deposit_loss_notified_at: Option<DateTime<Utc>>,
}

/// `GET /deposit-losses`. `notified` defaults to false (only copies not yet told to the lender).
#[serde_as]
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct DepositLossListQuery {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub lender_id: Option<i64>,
    /// `true` lists copies already notified. `false` or omitted lists the rest.
    pub notified: Option<bool>,
}

/// Calendar date or timestamp for `POST /items/{id}/deposit-loss-notified`.
#[derive(Debug, Clone)]
pub enum NotifiedOn {
    /// Start of this day in the library timezone.
    Date(NaiveDate),
    Timestamp(DateTime<Utc>),
}

/// Optional body. An empty body, or JSON `null` for `notifiedOn`, means now.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkDepositLossNotified {
    /// `YYYY-MM-DD` (start of that day in the library timezone) or an RFC 3339 timestamp.
    #[serde(default, deserialize_with = "deserialize_notified_on")]
    #[schema(value_type = Option<String>, example = "2026-10-19")]
    pub notified_on: Option<NotifiedOn>,
}

fn deserialize_notified_on<'de, D>(deserializer: D) -> Result<Option<NotifiedOn>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(serde::de::Error::custom("notifiedOn is empty"));
    }
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(Some(NotifiedOn::Timestamp(timestamp.with_timezone(&Utc))));
    }
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(Some(NotifiedOn::Date(date)));
    }
    Err(serde::de::Error::custom("notifiedOn must be a YYYY-MM-DD date or an RFC 3339 timestamp"))
}

/// JSON object for [`ItemDeposit`], or JSON null for own stock.
///
/// `item_alias` is the items table alias in the surrounding query. The value is
/// jsonb, never SQL NULL, so `#[sqlx(json)] Option<ItemDeposit>` decodes own
/// stock as `None`.
#[must_use]
pub fn item_deposit_json(item_alias: &str) -> String {
    format!(
        r#"CASE WHEN deposit_batch.id IS NULL THEN 'null'::jsonb
ELSE jsonb_build_object(
    'batchId', deposit_batch.id::text,
    'lenderId', deposit_batch.lender_id::text,
    'lenderName', deposit_lender.name,
    'reference', deposit_batch.reference,
    'returnDueOn', CASE
        WHEN deposit_batch.return_due_on IS NULL THEN NULL
        ELSE to_char(deposit_batch.return_due_on, 'YYYY-MM-DD')
    END,
    'batchStatus', deposit_batch.status,
    'depositLossNotifiedAt', CASE
        WHEN {alias}.deposit_loss_notified_at IS NULL THEN NULL
        ELSE to_char(
            {alias}.deposit_loss_notified_at AT TIME ZONE 'UTC',
            'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'
        )
    END
) END"#,
        alias = item_alias
    )
}

/// Joins that back [`item_deposit_json`]. One join pair per item query.
#[must_use]
pub fn item_deposit_join(item_alias: &str) -> String {
    format!(
        "LEFT JOIN deposit_batches deposit_batch ON deposit_batch.id = {item_alias}.deposit_batch_id \
         LEFT JOIN lenders deposit_lender ON deposit_lender.id = deposit_batch.lender_id"
    )
}

/// Decode a `deposit` jsonb column. NULL and a decode failure are own-stock `None`
/// only for NULL; a present object that does not match [`ItemDeposit`] is an error.
pub fn item_deposit_from_value(value: Option<serde_json::Value>) -> Result<Option<ItemDeposit>, serde_json::Error> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value).map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateLender;
    use crate::models::fine::Cents;

    #[test]
    fn replacement_rate_patch_distinguishes_absent_null_and_value() {
        let absent: UpdateLender = serde_json::from_str(r#"{"name":"A"}"#).unwrap();
        assert!(absent.replacement_rate_cents.is_none());

        let cleared: UpdateLender = serde_json::from_str(r#"{"replacementRateCents":null}"#).unwrap();
        assert_eq!(cleared.replacement_rate_cents, Some(None));

        let set: UpdateLender = serde_json::from_str(r#"{"replacementRateCents":1500}"#).unwrap();
        assert_eq!(set.replacement_rate_cents, Some(Some(Cents(1500))));
    }
}
