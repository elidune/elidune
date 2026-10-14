//! Fine / penalty model

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::user::UserStatus;

/// Kind of charge stored on a fine row (overdue accrual vs exception bills).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum FineChargeType {
    #[default]
    Overdue,
    Replacement,
    Damage,
}

impl FineChargeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Overdue => "overdue",
            Self::Replacement => "replacement",
            Self::Damage => "damage",
        }
    }
}

impl From<String> for FineChargeType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "replacement" => Self::Replacement,
            "damage" => Self::Damage,
            _ => Self::Overdue,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for FineChargeType {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("varchar")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for FineChargeType {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for FineChargeType {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Fine status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FineStatus {
    Pending,
    Partial,
    Paid,
    Waived,
    /// Uncollected charge voided (lost copy found, nothing was paid).
    Cancelled,
    /// Collected amount returned to the patron (lost copy found).
    Refunded,
}

impl FineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Waived => "waived",
            Self::Cancelled => "cancelled",
            Self::Refunded => "refunded",
        }
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Pending | Self::Partial)
    }
}

impl From<String> for FineStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "partial" => Self::Partial,
            "paid" => Self::Paid,
            "waived" => Self::Waived,
            "cancelled" => Self::Cancelled,
            "refunded" => Self::Refunded,
            _ => Self::Pending,
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for FineStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("varchar")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for FineStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for FineStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Fine record
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Fine {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub loan_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub user_id: Option<i64>,
    pub amount: Decimal,
    pub paid_amount: Decimal,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub status: FineStatus,
    pub notes: Option<String>,
    #[serde(default)]
    #[sqlx(default)]
    pub charge_type: FineChargeType,
    /// Copy this charge belongs to. Replacement bills set this so a lost copy
    /// can be resolved after the loan row is archived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    /// Amount given back when a collected charge is reversed.
    #[serde(default, skip_serializing_if = "is_zero_decimal")]
    #[sqlx(default)]
    pub refunded_amount: Decimal,
}

fn is_zero_decimal(amount: &Decimal) -> bool {
    amount.is_zero()
}

/// Currency amount as integer cents. `NUMERIC(12,2)` values convert exactly.
pub fn money_to_cents(amount: Decimal) -> Result<i64, &'static str> {
    let cents = (amount * Decimal::from(100u32)).round_dp(0);
    cents
        .to_i64()
        .ok_or("fine amount is outside the cents range")
}

/// Integer cents as a `NUMERIC(12,2)` currency amount.
#[must_use]
pub fn cents_to_money(cents: i64) -> Decimal {
    Decimal::from(cents) / Decimal::from(100)
}

/// `NUMERIC(12,2)` column read and written as integer cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoneyCents(pub i64);

impl serde::Serialize for MoneyCents {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for MoneyCents {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i64::deserialize(deserializer)?))
    }
}

impl sqlx::Type<sqlx::Postgres> for MoneyCents {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Decimal as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <Decimal as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for MoneyCents {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let amount = <Decimal as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        money_to_cents(amount).map(Self).map_err(Into::into)
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for MoneyCents {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <Decimal as sqlx::Encode<sqlx::Postgres>>::encode(cents_to_money(self.0), buf)
    }
}

macro_rules! impl_pg_text_enum {
    ($ty:ty) => {
        impl sqlx::Type<sqlx::Postgres> for $ty {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                <String as sqlx::Type<sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
                <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
            }
        }

        impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $ty {
            fn decode(
                value: sqlx::postgres::PgValueRef<'r>,
            ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                let text: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
                Self::from_db(&text).map_err(Into::into)
            }
        }

        impl sqlx::Encode<'_, sqlx::Postgres> for $ty {
            fn encode_by_ref(
                &self,
                buf: &mut sqlx::postgres::PgArgumentBuffer,
            ) -> sqlx::encode::IsNull {
                <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
            }
        }
    };
}

/// Global unpaid-fine gate. `none` never blocks on amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GlobalUnpaidThresholdMode {
    None,
    Amount,
}

impl GlobalUnpaidThresholdMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Amount => "amount",
        }
    }

    fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "none" => Ok(Self::None),
            "amount" => Ok(Self::Amount),
            other => Err(format!("unknown unpaid threshold mode {other}")),
        }
    }
}

impl_pg_text_enum!(GlobalUnpaidThresholdMode);

/// Audience unpaid-fine gate. `inherit` uses the global mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudienceUnpaidThresholdMode {
    Inherit,
    None,
    Amount,
}

impl AudienceUnpaidThresholdMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::None => "none",
            Self::Amount => "amount",
        }
    }

    fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "inherit" => Ok(Self::Inherit),
            "none" => Ok(Self::None),
            "amount" => Ok(Self::Amount),
            other => Err(format!("unknown unpaid threshold mode {other}")),
        }
    }
}

impl_pg_text_enum!(AudienceUnpaidThresholdMode);

/// Why a threshold write was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdInputError {
    /// `unpaidThresholdMode` is `amount` and `unpaidThresholdCents` was omitted.
    CentsRequired,
    Negative,
}

impl ThresholdInputError {
    pub fn status(self) -> u16 {
        match self {
            Self::CentsRequired => 422,
            Self::Negative => 400,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::CentsRequired => {
                "unpaidThresholdCents is required when unpaidThresholdMode is amount"
            }
            Self::Negative => "Unpaid fine threshold cannot be negative",
        }
    }

    pub fn into_app_error(self) -> crate::error::AppError {
        if self.status() == 422 {
            crate::error::AppError::coded(422, crate::error::error_code::VALIDATION, self.message())
        } else {
            crate::error::AppError::Validation(self.message().to_string())
        }
    }
}

/// Global row. The numeric column is always stored; mode `none` ignores it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StoredGlobalUnpaidThreshold {
    #[sqlx(rename = "unpaid_threshold_mode")]
    pub mode: GlobalUnpaidThresholdMode,
    #[sqlx(rename = "unpaid_fine_threshold")]
    pub amount: Decimal,
}

/// Amount stored for global mode `amount`. `None` means mode `none` (keep the numeric column).
pub fn resolve_global_amount(
    mode: GlobalUnpaidThresholdMode,
    cents: Option<i64>,
) -> Result<Option<Decimal>, ThresholdInputError> {
    match mode {
        GlobalUnpaidThresholdMode::None => Ok(None),
        GlobalUnpaidThresholdMode::Amount => cents_amount(cents),
    }
}

pub fn resolve_audience_threshold_create(
    mode: Option<AudienceUnpaidThresholdMode>,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Decimal>), ThresholdInputError> {
    resolve_audience_mode(mode.unwrap_or(AudienceUnpaidThresholdMode::Inherit), cents)
}

/// Absent mode keeps the stored mode and amount. Cents sent without a mode are ignored.
pub fn resolve_audience_threshold_update(
    existing_mode: AudienceUnpaidThresholdMode,
    existing_cents: Option<i64>,
    mode: Option<AudienceUnpaidThresholdMode>,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Decimal>), ThresholdInputError> {
    let Some(mode) = mode else {
        return Ok((existing_mode, existing_cents.map(cents_to_money)));
    };
    resolve_audience_mode(mode, cents)
}

fn resolve_audience_mode(
    mode: AudienceUnpaidThresholdMode,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Decimal>), ThresholdInputError> {
    match mode {
        AudienceUnpaidThresholdMode::Amount => Ok((mode, cents_amount(cents)?)),
        AudienceUnpaidThresholdMode::Inherit | AudienceUnpaidThresholdMode::None => {
            Ok((mode, None))
        }
    }
}

fn cents_amount(cents: Option<i64>) -> Result<Option<Decimal>, ThresholdInputError> {
    let cents = cents.ok_or(ThresholdInputError::CentsRequired)?;
    if cents < 0 {
        return Err(ThresholdInputError::Negative);
    }
    Ok(Some(cents_to_money(cents)))
}

/// Fine rule per media type
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FineRule {
    pub id: i32,
    pub media_type: Option<String>,
    pub daily_rate: rust_decimal::Decimal,
    pub max_amount: Option<rust_decimal::Decimal>,
    pub grace_days: i32,
    pub notes: Option<String>,
}

/// Pay fine request
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
pub struct PayFineRequest {
    pub amount: rust_decimal::Decimal,
    pub notes: Option<String>,
}

/// Waive fine request
#[derive(Debug, Deserialize, ToSchema)]
pub struct WaiveFineRequest {
    pub notes: Option<String>,
}

/// Loan context needed to accrue an overdue fine.
#[derive(Debug, Clone)]
pub struct FineAccrualLoan {
    pub loan_id: i64,
    pub user_id: i64,
    pub expiry_at: Option<DateTime<Utc>>,
    pub returned_at: Option<DateTime<Utc>>,
    pub media_type: Option<String>,
    pub user_status: UserStatus,
    /// Loan site: `loans.site_id`, or the library default site when the loan has none.
    /// Network-wide closures still apply when this is null.
    pub site_id: Option<i64>,
}
