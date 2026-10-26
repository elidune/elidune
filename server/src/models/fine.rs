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
    /// Charge total as a decimal-euro string (`"12.50"`). Stored as integer cents.
    #[schema(value_type = String, example = "12.50")]
    #[serde(with = "euro_string")]
    pub amount: Cents,
    /// Amount collected as a decimal-euro string (`"12.50"`). Stored as integer cents.
    #[schema(value_type = String, example = "0.00")]
    #[serde(with = "euro_string")]
    pub paid_amount: Cents,
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
    /// Amount given back as a decimal-euro string. Omitted when zero. Stored as integer cents.
    #[serde(default, skip_serializing_if = "is_zero_cents", with = "euro_string")]
    #[schema(value_type = String, example = "0.00")]
    #[sqlx(default)]
    pub refunded_amount: Cents,
}

fn is_zero_cents(amount: &Cents) -> bool {
    amount.0 == 0
}

/// Integer euro cents. Stored in `BIGINT` columns. JSON is an integer unless a
/// field opts into [`euro_string`] for a decimal-euro string.
///
/// `0` is a real amount. Absence is `Option::None`, not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Cents(pub i64);

/// Former name of the integer-cents JSON/database adapter. Same type as [`Cents`].
pub type MoneyCents = Cents;

/// Why a euro amount could not be stored as cents. Nothing is rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EuroParseError {
    Negative,
    TooManyDecimals,
    Unparseable,
}

impl std::fmt::Display for EuroParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Negative => "amount cannot be negative",
            Self::TooManyDecimals => "amount has more than 2 decimal places",
            Self::Unparseable => "amount is not a euro value",
        })
    }
}

impl std::error::Error for EuroParseError {}

impl Cents {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub fn get(self) -> i64 {
        self.0
    }

    /// `"12.50"` / `"12,50"` / `"0.00"`. Two digits, dot separator.
    #[must_use]
    pub fn to_euro_string(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    }

    /// Disposal CSV: `"12,50"`.
    #[must_use]
    pub fn to_csv_euro(self) -> String {
        self.to_euro_string().replace('.', ",")
    }

    pub fn checked_mul_i64(self, factor: i64) -> Option<Self> {
        self.0.checked_mul(factor).map(Self)
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Parse a major-unit euro string. Blank is `Ok(None)`.
    ///
    /// Accepts surrounding spaces, `.` or `,` as the decimal mark, and a trailing
    /// `€` or `EUR`. More than two non-zero fractional digits and negatives are
    /// rejected. Trailing zeros do not count (`"12.500"` is 1250 cents).
    pub fn parse_euro_text(raw: &str) -> Result<Option<Self>, EuroParseError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let without_currency = strip_trailing_currency(trimmed);
        let mut s = without_currency.trim();
        if s.is_empty() {
            return Err(EuroParseError::Unparseable);
        }
        if s.starts_with('-') {
            return Err(EuroParseError::Negative);
        }
        if let Some(rest) = s.strip_prefix('+') {
            s = rest.trim();
        }
        let (whole, frac) = split_plain_amount(s)?;
        let frac = frac.trim_end_matches('0');
        if frac.len() > 2 {
            return Err(EuroParseError::TooManyDecimals);
        }
        let padded = format!("{frac:0<2}");
        let combined = format!("{whole}{padded}");
        let value = combined
            .parse::<i64>()
            .map_err(|_| EuroParseError::Unparseable)?;
        Ok(Some(Self(value)))
    }

    /// Exact scale-2 (or less) `Decimal`. Rejects negatives and extra fractional digits.
    pub fn try_from_decimal(amount: Decimal) -> Result<Self, EuroParseError> {
        if amount.is_sign_negative() {
            return Err(EuroParseError::Negative);
        }
        if amount.normalize().scale() > 2 {
            return Err(EuroParseError::TooManyDecimals);
        }
        let scaled = amount * Decimal::from(100);
        if scaled.fract() != Decimal::ZERO {
            return Err(EuroParseError::TooManyDecimals);
        }
        let value = scaled.trunc().to_i64().ok_or(EuroParseError::Unparseable)?;
        Ok(Self(value))
    }
}

impl std::fmt::Display for Cents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_euro_string())
    }
}

impl<'s> utoipa::ToSchema<'s> for Cents {
    fn schema() -> (
        &'s str,
        utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>,
    ) {
        (
            "Cents",
            utoipa::openapi::ObjectBuilder::new()
                .schema_type(utoipa::openapi::SchemaType::Integer)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(utoipa::openapi::KnownFormat::Int64)))
                .description(Some("Integer euro cents. 1500 is 15.00 EUR. Fields documented as decimal euros use a string instead."))
                .into(),
        )
    }
}

impl serde::Serialize for Cents {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Cents {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(i64::deserialize(deserializer)?))
    }
}

impl sqlx::Type<sqlx::Postgres> for Cents {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <i64 as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Cents {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let n = <i64 as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(Self(n))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Cents {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <i64 as sqlx::Encode<sqlx::Postgres>>::encode(self.0, buf)
    }
}

/// Decimal-euro JSON (`"12.50"`) for a [`Cents`] field.
pub mod euro_string {
    use super::{Cents, EuroParseError};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(cents: &Cents, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&cents.to_euro_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Cents, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match Cents::parse_euro_text(&raw).map_err(serde::de::Error::custom)? {
            Some(cents) => Ok(cents),
            None => Err(serde::de::Error::custom(EuroParseError::Unparseable)),
        }
    }

    pub mod opt {
        use super::super::Cents;
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S: Serializer>(
            value: &Option<Cents>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match value {
                Some(cents) => serializer.serialize_str(&cents.to_euro_string()),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Cents>, D::Error> {
            let raw = Option::<String>::deserialize(deserializer)?;
            match raw {
                None => Ok(None),
                Some(text) => Cents::parse_euro_text(&text).map_err(serde::de::Error::custom),
            }
        }
    }
}

fn strip_trailing_currency(raw: &str) -> &str {
    let end = raw.trim_end();
    if let Some(rest) = end.strip_suffix('€') {
        return rest.trim_end();
    }
    if end.len() >= 3 && end[end.len() - 3..].eq_ignore_ascii_case("eur") {
        let rest = &end[..end.len() - 3];
        if rest
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            return raw.trim();
        }
        return rest.trim_end();
    }
    raw.trim()
}

fn split_plain_amount(s: &str) -> Result<(&str, &str), EuroParseError> {
    if s.is_empty() || !s.as_bytes()[0].is_ascii_digit() {
        return Err(EuroParseError::Unparseable);
    }
    let mut separator = None;
    for (index, byte) in s.bytes().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' | b',' if separator.is_none() => separator = Some(index),
            _ => return Err(EuroParseError::Unparseable),
        }
    }
    match separator {
        None => Ok((s, "")),
        Some(index) => {
            let frac = &s[index + 1..];
            if frac.is_empty() {
                return Err(EuroParseError::Unparseable);
            }
            Ok((&s[..index], frac))
        }
    }
}

/// Exact non-negative cent conversion. Does not round extra fractional digits.
pub fn money_to_cents(amount: Decimal) -> Result<i64, &'static str> {
    Cents::try_from_decimal(amount)
        .map(Cents::get)
        .map_err(|_| "amount is not an exact non-negative cent value")
}

/// Integer cents as a major-unit decimal (two places).
#[must_use]
pub fn cents_to_money(cents: i64) -> Decimal {
    Decimal::from(cents) / Decimal::from(100)
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
    /// Stored threshold in integer euro cents.
    #[sqlx(rename = "unpaid_fine_threshold")]
    pub amount: Cents,
}

/// Amount stored for global mode `amount`. `None` means mode `none` (keep the numeric column).
pub fn resolve_global_amount(
    mode: GlobalUnpaidThresholdMode,
    cents: Option<i64>,
) -> Result<Option<Cents>, ThresholdInputError> {
    match mode {
        GlobalUnpaidThresholdMode::None => Ok(None),
        GlobalUnpaidThresholdMode::Amount => cents_amount(cents),
    }
}

pub fn resolve_audience_threshold_create(
    mode: Option<AudienceUnpaidThresholdMode>,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Cents>), ThresholdInputError> {
    resolve_audience_mode(mode.unwrap_or(AudienceUnpaidThresholdMode::Inherit), cents)
}

/// Absent mode keeps the stored mode and amount. Cents sent without a mode are ignored.
pub fn resolve_audience_threshold_update(
    existing_mode: AudienceUnpaidThresholdMode,
    existing_cents: Option<i64>,
    mode: Option<AudienceUnpaidThresholdMode>,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Cents>), ThresholdInputError> {
    let Some(mode) = mode else {
        return Ok((existing_mode, existing_cents.map(Cents)));
    };
    resolve_audience_mode(mode, cents)
}

fn resolve_audience_mode(
    mode: AudienceUnpaidThresholdMode,
    cents: Option<i64>,
) -> Result<(AudienceUnpaidThresholdMode, Option<Cents>), ThresholdInputError> {
    match mode {
        AudienceUnpaidThresholdMode::Amount => Ok((mode, cents_amount(cents)?)),
        AudienceUnpaidThresholdMode::Inherit | AudienceUnpaidThresholdMode::None => {
            Ok((mode, None))
        }
    }
}

fn cents_amount(cents: Option<i64>) -> Result<Option<Cents>, ThresholdInputError> {
    let cents = cents.ok_or(ThresholdInputError::CentsRequired)?;
    if cents < 0 {
        return Err(ThresholdInputError::Negative);
    }
    Ok(Some(Cents(cents)))
}

/// Fine rule per media type
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FineRule {
    pub id: i32,
    pub media_type: Option<String>,
    /// Overdue daily rate as a decimal-euro string (`"0.20"`). Stored as integer cents.
    #[schema(value_type = String, example = "0.20")]
    #[serde(with = "euro_string")]
    pub daily_rate: Cents,
    /// Overdue cap as a decimal-euro string. Null means no cap. Stored as integer cents.
    #[schema(value_type = Option<String>, example = "15.00")]
    #[serde(with = "euro_string::opt")]
    pub max_amount: Option<Cents>,
    pub grace_days: i32,
    pub notes: Option<String>,
}

/// Pay fine request
#[serde_as]
#[derive(Debug, Deserialize, ToSchema)]
pub struct PayFineRequest {
    /// Payment as a decimal-euro string (`"12.50"`).
    #[schema(value_type = String, example = "12.50")]
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

#[cfg(test)]
mod tests {
    use super::{Cents, EuroParseError};

    fn parsed(raw: &str) -> Result<Option<i64>, EuroParseError> {
        Cents::parse_euro_text(raw).map(|value| value.map(Cents::get))
    }

    #[test]
    fn euro_text_accepts_the_migration_formats() {
        let accepted = [
            ("12,50", 1250),
            ("12,50 €", 1250),
            ("12,50€", 1250),
            ("12 €", 1200),
            ("12.50 €", 1250),
            ("12.50EUR", 1250),
            ("12.50 EUR", 1250),
            ("12.5", 1250),
            ("15", 1500),
            ("12.50", 1250),
            ("0", 0),
            ("0.00", 0),
            ("0,00", 0),
            ("  12.50  ", 1250),
            ("12.500", 1250),
            ("12.510", 1251),
            ("1.230", 123),
            ("+12.50", 1250),
        ];
        for (raw, cents) in accepted {
            assert_eq!(parsed(raw), Ok(Some(cents)), "{raw}");
        }
        assert_eq!(parsed(""), Ok(None));
        assert_eq!(parsed("   "), Ok(None));
    }

    #[test]
    fn euro_text_rejects_without_rounding() {
        assert_eq!(parsed("abc"), Err(EuroParseError::Unparseable));
        assert_eq!(parsed("12."), Err(EuroParseError::Unparseable));
        assert_eq!(parsed("€12.50"), Err(EuroParseError::Unparseable));
        assert_eq!(parsed("12.50.1"), Err(EuroParseError::Unparseable));
        assert_eq!(parsed("-1"), Err(EuroParseError::Negative));
        assert_eq!(parsed("-0.01"), Err(EuroParseError::Negative));
        assert_eq!(parsed("12.501"), Err(EuroParseError::TooManyDecimals));
        assert_eq!(parsed("12,501"), Err(EuroParseError::TooManyDecimals));
    }

    #[test]
    fn cents_newtype_stores_i64() {
        let Cents(raw) = Cents(1250);
        assert_eq!(raw, 1250_i64);
        assert_eq!(std::mem::size_of::<Cents>(), std::mem::size_of::<i64>());
    }

    #[test]
    fn formats_and_decimal_round_trip() {
        assert_eq!(Cents(1250).to_euro_string(), "12.50");
        assert_eq!(Cents(0).to_euro_string(), "0.00");
        assert_eq!(Cents(1500).to_csv_euro(), "15,00");
        assert_eq!(Cents::ZERO.to_csv_euro(), "0,00");
        let amount = rust_decimal::Decimal::new(1250, 2);
        assert_eq!(Cents::try_from_decimal(amount).unwrap(), Cents(1250));
        assert_eq!(
            Cents::try_from_decimal(rust_decimal::Decimal::new(12501, 3)),
            Err(EuroParseError::TooManyDecimals)
        );
        assert_eq!(
            Cents::try_from_decimal(rust_decimal::Decimal::new(-1, 2)),
            Err(EuroParseError::Negative)
        );
    }
}
