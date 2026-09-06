//! Acquisitions domain models: vendors, yearly funds, purchase orders, receipts.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// Purchase order lifecycle (MVP: no EDI / claiming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum PurchaseOrderStatus {
    #[default]
    Draft,
    Ordered,
    Partial,
    Received,
    Cancelled,
}

impl PurchaseOrderStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ordered => "ordered",
            Self::Partial => "partial",
            Self::Received => "received",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn is_editable(self) -> bool {
        matches!(self, Self::Draft)
    }

    #[must_use]
    pub fn can_submit(self) -> bool {
        matches!(self, Self::Draft)
    }

    #[must_use]
    pub fn can_receive(self) -> bool {
        matches!(self, Self::Ordered | Self::Partial)
    }

    #[must_use]
    pub fn can_cancel(self) -> bool {
        matches!(self, Self::Draft | Self::Ordered)
    }

    #[must_use]
    pub fn encumbers_budget(self) -> bool {
        matches!(self, Self::Ordered | Self::Partial)
    }
}

impl From<&str> for PurchaseOrderStatus {
    fn from(s: &str) -> Self {
        match s {
            "ordered" => Self::Ordered,
            "partial" => Self::Partial,
            "received" => Self::Received,
            "cancelled" => Self::Cancelled,
            _ => Self::Draft,
        }
    }
}

impl From<String> for PurchaseOrderStatus {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl sqlx::Type<sqlx::Postgres> for PurchaseOrderStatus {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("varchar")
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for PurchaseOrderStatus {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Ok(Self::from(s.as_str()))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for PurchaseOrderStatus {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_str().to_string(), buf)
    }
}

/// Supplier / vendor used on purchase orders.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Vendor {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}

/// Simple yearly fund / fund code with committed vs spent totals.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionFund {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub code: String,
    pub name: String,
    pub fiscal_year: i32,
    pub allocated_amount: Decimal,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Outstanding encumbrance: unordered remaining qty × unit price on ordered/partial POs.
    #[sqlx(default)]
    pub committed: Decimal,
    /// Amount posted from receipts against this fund.
    #[sqlx(default)]
    pub spent: Decimal,
    /// `allocated − committed − spent` (may be negative if overspent).
    #[sqlx(default)]
    pub available: Decimal,
}

/// Purchase order header (vendor + optional default fund).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrder {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub vendor_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    pub order_number: String,
    pub status: PurchaseOrderStatus,
    pub notes: Option<String>,
    pub ordered_at: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(default)]
    pub vendor_name: Option<String>,
    #[sqlx(default)]
    pub fund_code: Option<String>,
}

/// One ordered title/ISBN (intent) or existing biblio, with qty and unit price.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderLine {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub purchase_order_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    pub isbn: Option<String>,
    pub title: Option<String>,
    pub quantity_ordered: i32,
    pub quantity_received: i32,
    pub unit_price: Option<Decimal>,
    pub currency: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Receipt header recorded when copies are accessioned.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub purchase_order_id: i64,
    pub received_at: DateTime<Utc>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub received_by: Option<i64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One receipt line and the catalog items created for it.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLineResult {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub purchase_order_line_id: i64,
    pub quantity: i32,
    pub unit_price: Option<Decimal>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub item_ids: Vec<i64>,
}

/// Purchase order plus lines (detail GET / receive response).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderDetail {
    pub order: PurchaseOrder,
    pub lines: Vec<PurchaseOrderLine>,
}

/// Create vendor request.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateVendor {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    #[validate(length(max = 50))]
    pub code: Option<String>,
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub active: Option<bool>,
}

/// Update vendor request (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVendor {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    #[validate(length(max = 50))]
    pub code: Option<String>,
    pub email: Option<String>,
    #[validate(length(max = 50))]
    pub phone: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub active: Option<bool>,
}

/// Create yearly fund request.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFund {
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub fiscal_year: i32,
    pub allocated_amount: Option<Decimal>,
    #[validate(length(min = 3, max = 3))]
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Update yearly fund request (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFund {
    #[validate(length(min = 1, max = 50))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    pub fiscal_year: Option<i32>,
    pub allocated_amount: Option<Decimal>,
    #[validate(length(min = 3, max = 3))]
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Line payload when creating or adding an order line.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderLine {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    #[validate(length(max = 30))]
    pub isbn: Option<String>,
    #[validate(length(max = 500))]
    pub title: Option<String>,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: Option<Decimal>,
    #[validate(length(min = 3, max = 3))]
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Create purchase order (draft) with optional lines.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePurchaseOrder {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub vendor_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    #[validate(length(max = 50))]
    pub order_number: Option<String>,
    pub notes: Option<String>,
    #[validate(nested)]
    pub lines: Option<Vec<CreateOrderLine>>,
}

/// Update purchase order header (draft only).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePurchaseOrder {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub vendor_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    #[validate(length(max = 50))]
    pub order_number: Option<String>,
    pub notes: Option<String>,
}

/// Update an order line (draft only).
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrderLine {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    #[validate(length(max = 30))]
    pub isbn: Option<String>,
    #[validate(length(max = 500))]
    pub title: Option<String>,
    #[validate(range(min = 1))]
    pub quantity: Option<i32>,
    pub unit_price: Option<Decimal>,
    #[validate(length(min = 3, max = 3))]
    pub currency: Option<String>,
    pub notes: Option<String>,
}

/// Per-copy fields applied when creating a catalog item at receipt.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveItemSpec {
    #[validate(length(max = 100))]
    pub barcode: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub source_id: Option<i64>,
    #[validate(length(max = 200))]
    pub source_name: Option<String>,
    #[validate(length(max = 100))]
    pub price: Option<String>,
    #[validate(length(max = 200))]
    pub call_number: Option<String>,
}

/// One PO line to receive.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveOrderLine {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub line_id: i64,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: Option<Decimal>,
    #[validate(nested)]
    pub items: Option<Vec<ReceiveItemSpec>>,
}

/// Receive copies against an ordered / partial purchase order.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceivePurchaseOrder {
    pub notes: Option<String>,
    #[validate(length(min = 1))]
    #[validate(nested)]
    pub lines: Vec<ReceiveOrderLine>,
}

/// Receipt response: created items plus the updated order.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReceivePurchaseOrderResult {
    pub receipt: Receipt,
    pub lines: Vec<ReceiptLineResult>,
    pub order: PurchaseOrderDetail,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct VendorQuery {
    pub q: Option<String>,
    pub include_archived: Option<bool>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct FundQuery {
    pub q: Option<String>,
    pub fiscal_year: Option<i32>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct PurchaseOrderQuery {
    pub q: Option<String>,
    pub status: Option<PurchaseOrderStatus>,
    pub vendor_id: Option<String>,
    pub fund_id: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PurchaseOrderQuery {
    pub fn vendor_id_parsed(&self) -> Option<i64> {
        self.vendor_id.as_deref().and_then(|s| s.parse().ok())
    }

    pub fn fund_id_parsed(&self) -> Option<i64> {
        self.fund_id.as_deref().and_then(|s| s.parse().ok())
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VendorListResponse {
    pub vendors: Vec<Vendor>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FundListResponse {
    pub funds: Vec<AcquisitionFund>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrderListResponse {
    pub orders: Vec<PurchaseOrder>,
    pub total: i64,
}
