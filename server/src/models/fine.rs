//! Fine / penalty model

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::models::user::UserStatus;

/// Fine status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FineStatus {
    Pending,
    Partial,
    Paid,
    Waived,
}

impl FineStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Partial => "partial",
            Self::Paid => "paid",
            Self::Waived => "waived",
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
    pub amount: rust_decimal::Decimal,
    pub paid_amount: rust_decimal::Decimal,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub status: FineStatus,
    pub notes: Option<String>,
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
}
