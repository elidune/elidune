//! Acquisitions persistence: vendors, funds, purchase orders, receipts.

use async_trait::async_trait;
use chrono::{Datelike, Utc};
use rust_decimal::Decimal;
use snowflaked::Generator;
use sqlx::{Postgres, Transaction};

use super::Repository;
use crate::{
    error::{AppError, AppResult},
    models::acquisition::{
        AcquisitionFund, CreateFund, CreateOrderLine, CreatePurchaseOrder, CreateVendor, FundQuery,
        PurchaseOrder, PurchaseOrderLine, PurchaseOrderQuery, PurchaseOrderStatus, Receipt,
        ReceiveOrderLine, UpdateFund, UpdateOrderLine, UpdatePurchaseOrder, UpdateVendor, Vendor,
        VendorQuery,
    },
};

static SNOWFLAKE: std::sync::LazyLock<std::sync::Mutex<Generator>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Generator::new(9)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .generate::<i64>()
}

fn page_offset(page: Option<i64>, per_page: Option<i64>) -> (i64, i64) {
    let page = page.unwrap_or(1).max(1);
    let per_page = per_page.unwrap_or(20).clamp(1, 100);
    (per_page, (page - 1) * per_page)
}

const FUND_SELECT: &str = r#"
    SELECT f.id, f.code, f.name, f.fiscal_year, f.allocated_amount, f.currency, f.notes,
           f.created_at, f.updated_at,
           COALESCE((
               SELECT SUM(
                   (l.quantity_ordered - l.quantity_received)::numeric
                   * COALESCE(l.unit_price, 0)
               )
               FROM purchase_order_lines l
               JOIN purchase_orders o ON o.id = l.purchase_order_id
               WHERE COALESCE(l.fund_id, o.fund_id) = f.id
                 AND o.status IN ('ordered', 'partial')
           ), 0) AS committed,
           COALESCE((
               SELECT SUM(rl.quantity::numeric * COALESCE(rl.unit_price, l.unit_price, 0))
               FROM receipt_lines rl
               JOIN purchase_order_lines l ON l.id = rl.purchase_order_line_id
               JOIN purchase_orders o ON o.id = l.purchase_order_id
               WHERE COALESCE(l.fund_id, o.fund_id) = f.id
                 AND o.status <> 'cancelled'
           ), 0) AS spent,
           f.allocated_amount
             - COALESCE((
                   SELECT SUM(
                       (l.quantity_ordered - l.quantity_received)::numeric
                       * COALESCE(l.unit_price, 0)
                   )
                   FROM purchase_order_lines l
                   JOIN purchase_orders o ON o.id = l.purchase_order_id
                   WHERE COALESCE(l.fund_id, o.fund_id) = f.id
                     AND o.status IN ('ordered', 'partial')
               ), 0)
             - COALESCE((
                   SELECT SUM(rl.quantity::numeric * COALESCE(rl.unit_price, l.unit_price, 0))
                   FROM receipt_lines rl
                   JOIN purchase_order_lines l ON l.id = rl.purchase_order_line_id
                   JOIN purchase_orders o ON o.id = l.purchase_order_id
                   WHERE COALESCE(l.fund_id, o.fund_id) = f.id
                     AND o.status <> 'cancelled'
               ), 0) AS available
    FROM acquisition_funds f
"#;

const ORDER_SELECT: &str = r#"
    SELECT o.id, o.vendor_id, o.fund_id, o.order_number, o.status, o.notes, o.ordered_at,
           o.created_by, o.created_at, o.updated_at,
           v.name AS vendor_name,
           f.code AS fund_code
    FROM purchase_orders o
    JOIN vendors v ON v.id = o.vendor_id
    LEFT JOIN acquisition_funds f ON f.id = o.fund_id
"#;

/// `(receipt_line_id, purchase_order_line_id, quantity, unit_price, item_ids)`.
pub type ReceiptLinePersist = (i64, i64, i32, Option<Decimal>, Vec<i64>);

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AcquisitionsRepository: Send + Sync {
    async fn vendors_list(&self, query: &VendorQuery) -> AppResult<(Vec<Vendor>, i64)>;
    async fn vendors_get(&self, id: i64) -> AppResult<Vendor>;
    async fn vendors_create(&self, data: &CreateVendor) -> AppResult<Vendor>;
    async fn vendors_update(&self, id: i64, data: &UpdateVendor) -> AppResult<Vendor>;
    async fn vendors_archive(&self, id: i64) -> AppResult<()>;

    async fn funds_list(&self, query: &FundQuery) -> AppResult<(Vec<AcquisitionFund>, i64)>;
    async fn funds_get(&self, id: i64) -> AppResult<AcquisitionFund>;
    async fn funds_create(&self, data: &CreateFund) -> AppResult<AcquisitionFund>;
    async fn funds_update(&self, id: i64, data: &UpdateFund) -> AppResult<AcquisitionFund>;

    async fn orders_list(&self, query: &PurchaseOrderQuery)
        -> AppResult<(Vec<PurchaseOrder>, i64)>;
    async fn orders_get(&self, id: i64) -> AppResult<PurchaseOrder>;
    async fn orders_list_lines(&self, order_id: i64) -> AppResult<Vec<PurchaseOrderLine>>;
    async fn orders_create(
        &self,
        data: &CreatePurchaseOrder,
        created_by: i64,
    ) -> AppResult<PurchaseOrder>;
    async fn orders_update(&self, id: i64, data: &UpdatePurchaseOrder) -> AppResult<PurchaseOrder>;
    async fn orders_add_line(
        &self,
        order_id: i64,
        data: &CreateOrderLine,
    ) -> AppResult<PurchaseOrderLine>;
    async fn orders_update_line(
        &self,
        order_id: i64,
        line_id: i64,
        data: &UpdateOrderLine,
    ) -> AppResult<PurchaseOrderLine>;
    async fn orders_delete_line(&self, order_id: i64, line_id: i64) -> AppResult<()>;
    async fn orders_submit(&self, id: i64) -> AppResult<PurchaseOrder>;
    async fn orders_cancel(&self, id: i64) -> AppResult<PurchaseOrder>;
    async fn orders_receive(
        &self,
        order_id: i64,
        received_by: i64,
        notes: Option<String>,
        lines: &[ReceiveOrderLine],
        created_items: &[(i64, Vec<i64>, Option<Decimal>)],
    ) -> AppResult<Receipt>;
    async fn orders_set_line_biblio(&self, line_id: i64, biblio_id: i64) -> AppResult<()>;
    async fn receipt_line_results(&self, receipt_id: i64) -> AppResult<Vec<ReceiptLinePersist>>;
}

#[async_trait]
impl AcquisitionsRepository for Repository {
    async fn vendors_list(&self, query: &VendorQuery) -> AppResult<(Vec<Vendor>, i64)> {
        Repository::vendors_list(self, query).await
    }
    async fn vendors_get(&self, id: i64) -> AppResult<Vendor> {
        Repository::vendors_get(self, id).await
    }
    async fn vendors_create(&self, data: &CreateVendor) -> AppResult<Vendor> {
        Repository::vendors_create(self, data).await
    }
    async fn vendors_update(&self, id: i64, data: &UpdateVendor) -> AppResult<Vendor> {
        Repository::vendors_update(self, id, data).await
    }
    async fn vendors_archive(&self, id: i64) -> AppResult<()> {
        Repository::vendors_archive(self, id).await
    }
    async fn funds_list(&self, query: &FundQuery) -> AppResult<(Vec<AcquisitionFund>, i64)> {
        Repository::funds_list(self, query).await
    }
    async fn funds_get(&self, id: i64) -> AppResult<AcquisitionFund> {
        Repository::funds_get(self, id).await
    }
    async fn funds_create(&self, data: &CreateFund) -> AppResult<AcquisitionFund> {
        Repository::funds_create(self, data).await
    }
    async fn funds_update(&self, id: i64, data: &UpdateFund) -> AppResult<AcquisitionFund> {
        Repository::funds_update(self, id, data).await
    }
    async fn orders_list(
        &self,
        query: &PurchaseOrderQuery,
    ) -> AppResult<(Vec<PurchaseOrder>, i64)> {
        Repository::orders_list(self, query).await
    }
    async fn orders_get(&self, id: i64) -> AppResult<PurchaseOrder> {
        Repository::orders_get(self, id).await
    }
    async fn orders_list_lines(&self, order_id: i64) -> AppResult<Vec<PurchaseOrderLine>> {
        Repository::orders_list_lines(self, order_id).await
    }
    async fn orders_create(
        &self,
        data: &CreatePurchaseOrder,
        created_by: i64,
    ) -> AppResult<PurchaseOrder> {
        Repository::orders_create(self, data, created_by).await
    }
    async fn orders_update(&self, id: i64, data: &UpdatePurchaseOrder) -> AppResult<PurchaseOrder> {
        Repository::orders_update(self, id, data).await
    }
    async fn orders_add_line(
        &self,
        order_id: i64,
        data: &CreateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        Repository::orders_add_line(self, order_id, data).await
    }
    async fn orders_update_line(
        &self,
        order_id: i64,
        line_id: i64,
        data: &UpdateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        Repository::orders_update_line(self, order_id, line_id, data).await
    }
    async fn orders_delete_line(&self, order_id: i64, line_id: i64) -> AppResult<()> {
        Repository::orders_delete_line(self, order_id, line_id).await
    }
    async fn orders_submit(&self, id: i64) -> AppResult<PurchaseOrder> {
        Repository::orders_submit(self, id).await
    }
    async fn orders_cancel(&self, id: i64) -> AppResult<PurchaseOrder> {
        Repository::orders_cancel(self, id).await
    }
    async fn orders_receive(
        &self,
        order_id: i64,
        received_by: i64,
        notes: Option<String>,
        lines: &[ReceiveOrderLine],
        created_items: &[(i64, Vec<i64>, Option<Decimal>)],
    ) -> AppResult<Receipt> {
        Repository::orders_receive(
            self,
            order_id,
            received_by,
            notes.as_deref(),
            lines,
            created_items,
        )
        .await
    }
    async fn orders_set_line_biblio(&self, line_id: i64, biblio_id: i64) -> AppResult<()> {
        Repository::orders_set_line_biblio(self, line_id, biblio_id).await
    }
    async fn receipt_line_results(&self, receipt_id: i64) -> AppResult<Vec<ReceiptLinePersist>> {
        Repository::receipt_line_ids_for_receipt(self, receipt_id).await
    }
}

impl Repository {
    pub async fn vendors_list(&self, query: &VendorQuery) -> AppResult<(Vec<Vendor>, i64)> {
        let (limit, offset) = page_offset(query.page, query.per_page);
        let include_archived = query.include_archived.unwrap_or(false);
        let search = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM vendors
            WHERE ($1 OR archived_at IS NULL)
              AND ($2::text IS NULL OR name ILIKE $2 OR COALESCE(code, '') ILIKE $2)
            "#,
        )
        .bind(include_archived)
        .bind(&search)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query_as::<_, Vendor>(
            r#"
            SELECT * FROM vendors
            WHERE ($1 OR archived_at IS NULL)
              AND ($2::text IS NULL OR name ILIKE $2 OR COALESCE(code, '') ILIKE $2)
            ORDER BY name
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(include_archived)
        .bind(&search)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((rows, total))
    }

    pub async fn vendors_get(&self, id: i64) -> AppResult<Vendor> {
        sqlx::query_as::<_, Vendor>("SELECT * FROM vendors WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Vendor {id} not found")))
    }

    pub async fn vendors_create(&self, data: &CreateVendor) -> AppResult<Vendor> {
        let id = next_id();
        let now = Utc::now();
        let code = normalize_opt_code(data.code.as_deref());
        sqlx::query_as::<_, Vendor>(
            r#"
            INSERT INTO vendors (id, name, code, email, phone, address, notes, active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(data.name.trim())
        .bind(code)
        .bind(empty_to_none(data.email.as_deref()))
        .bind(empty_to_none(data.phone.as_deref()))
        .bind(empty_to_none(data.address.as_deref()))
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(data.active.unwrap_or(true))
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_unique_vendor)
    }

    pub async fn vendors_update(&self, id: i64, data: &UpdateVendor) -> AppResult<Vendor> {
        let existing = self.vendors_get(id).await?;
        if existing.archived_at.is_some() {
            return Err(AppError::BusinessRule(
                "Cannot update an archived vendor".into(),
            ));
        }
        let now = Utc::now();
        sqlx::query_as::<_, Vendor>(
            r#"
            UPDATE vendors SET
                name = COALESCE($2, name),
                code = CASE WHEN $3 THEN $4 ELSE code END,
                email = CASE WHEN $5 THEN $6 ELSE email END,
                phone = CASE WHEN $7 THEN $8 ELSE phone END,
                address = CASE WHEN $9 THEN $10 ELSE address END,
                notes = CASE WHEN $11 THEN $12 ELSE notes END,
                active = COALESCE($13, active),
                updated_at = $14
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(data.name.as_deref().map(str::trim))
        .bind(data.code.is_some())
        .bind(normalize_opt_code(data.code.as_deref()))
        .bind(data.email.is_some())
        .bind(empty_to_none(data.email.as_deref()))
        .bind(data.phone.is_some())
        .bind(empty_to_none(data.phone.as_deref()))
        .bind(data.address.is_some())
        .bind(empty_to_none(data.address.as_deref()))
        .bind(data.notes.is_some())
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(data.active)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(map_unique_vendor)
    }

    pub async fn vendors_archive(&self, id: i64) -> AppResult<()> {
        let now = Utc::now();
        let rows = sqlx::query(
            r#"
            UPDATE vendors SET archived_at = $2, active = FALSE, updated_at = $2
            WHERE id = $1 AND archived_at IS NULL
            "#,
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(AppError::NotFound(format!("Vendor {id} not found")));
        }
        Ok(())
    }

    pub async fn funds_list(&self, query: &FundQuery) -> AppResult<(Vec<AcquisitionFund>, i64)> {
        let (limit, offset) = page_offset(query.page, query.per_page);
        let search = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM acquisition_funds
            WHERE ($1::int IS NULL OR fiscal_year = $1)
              AND ($2::text IS NULL OR code ILIKE $2 OR name ILIKE $2)
            "#,
        )
        .bind(query.fiscal_year)
        .bind(&search)
        .fetch_one(&self.pool)
        .await?;

        let sql = format!(
            "{FUND_SELECT}
             WHERE ($1::int IS NULL OR f.fiscal_year = $1)
               AND ($2::text IS NULL OR f.code ILIKE $2 OR f.name ILIKE $2)
             ORDER BY f.fiscal_year DESC, f.code
             LIMIT $3 OFFSET $4"
        );
        let rows = sqlx::query_as::<_, AcquisitionFund>(&sql)
            .bind(query.fiscal_year)
            .bind(&search)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        Ok((rows, total))
    }

    pub async fn funds_get(&self, id: i64) -> AppResult<AcquisitionFund> {
        let sql = format!("{FUND_SELECT} WHERE f.id = $1");
        sqlx::query_as::<_, AcquisitionFund>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Fund {id} not found")))
    }

    pub async fn funds_create(&self, data: &CreateFund) -> AppResult<AcquisitionFund> {
        let id = next_id();
        let now = Utc::now();
        let currency = normalize_currency(data.currency.as_deref())?;
        let allocated = data.allocated_amount.unwrap_or(Decimal::ZERO);
        if allocated < Decimal::ZERO {
            return Err(AppError::Validation(
                "allocatedAmount must be greater than or equal to 0".into(),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO acquisition_funds
                (id, code, name, fiscal_year, allocated_amount, currency, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)
            "#,
        )
        .bind(id)
        .bind(data.code.trim())
        .bind(data.name.trim())
        .bind(data.fiscal_year)
        .bind(allocated)
        .bind(currency)
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_unique_fund)?;
        self.funds_get(id).await
    }

    pub async fn funds_update(&self, id: i64, data: &UpdateFund) -> AppResult<AcquisitionFund> {
        let _ = self.funds_get(id).await?;
        if let Some(amount) = data.allocated_amount {
            if amount < Decimal::ZERO {
                return Err(AppError::Validation(
                    "allocatedAmount must be greater than or equal to 0".into(),
                ));
            }
        }
        let currency = match data.currency.as_deref() {
            Some(c) => Some(normalize_currency(Some(c))?),
            None => None,
        };
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE acquisition_funds SET
                code = COALESCE($2, code),
                name = COALESCE($3, name),
                fiscal_year = COALESCE($4, fiscal_year),
                allocated_amount = COALESCE($5, allocated_amount),
                currency = COALESCE($6, currency),
                notes = CASE WHEN $7 THEN $8 ELSE notes END,
                updated_at = $9
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(data.code.as_deref().map(str::trim))
        .bind(data.name.as_deref().map(str::trim))
        .bind(data.fiscal_year)
        .bind(data.allocated_amount)
        .bind(currency.as_deref())
        .bind(data.notes.is_some())
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_unique_fund)?;
        self.funds_get(id).await
    }

    pub async fn orders_list(
        &self,
        query: &PurchaseOrderQuery,
    ) -> AppResult<(Vec<PurchaseOrder>, i64)> {
        let (limit, offset) = page_offset(query.page, query.per_page);
        let search = query
            .q
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{s}%"));
        let status = query.status.map(|s| s.as_str().to_string());
        let vendor_id = query.vendor_id_parsed();
        let fund_id = query.fund_id_parsed();

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM purchase_orders o
            JOIN vendors v ON v.id = o.vendor_id
            WHERE ($1::text IS NULL OR o.status = $1)
              AND ($2::bigint IS NULL OR o.vendor_id = $2)
              AND ($3::bigint IS NULL OR o.fund_id = $3)
              AND ($4::text IS NULL OR o.order_number ILIKE $4 OR v.name ILIKE $4)
            "#,
        )
        .bind(&status)
        .bind(vendor_id)
        .bind(fund_id)
        .bind(&search)
        .fetch_one(&self.pool)
        .await?;

        let sql = format!(
            "{ORDER_SELECT}
             WHERE ($1::text IS NULL OR o.status = $1)
               AND ($2::bigint IS NULL OR o.vendor_id = $2)
               AND ($3::bigint IS NULL OR o.fund_id = $3)
               AND ($4::text IS NULL OR o.order_number ILIKE $4 OR v.name ILIKE $4)
             ORDER BY o.created_at DESC
             LIMIT $5 OFFSET $6"
        );
        let rows = sqlx::query_as::<_, PurchaseOrder>(&sql)
            .bind(&status)
            .bind(vendor_id)
            .bind(fund_id)
            .bind(&search)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        Ok((rows, total))
    }

    pub async fn orders_get(&self, id: i64) -> AppResult<PurchaseOrder> {
        let sql = format!("{ORDER_SELECT} WHERE o.id = $1");
        sqlx::query_as::<_, PurchaseOrder>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Purchase order {id} not found")))
    }

    pub async fn orders_list_lines(&self, order_id: i64) -> AppResult<Vec<PurchaseOrderLine>> {
        sqlx::query_as::<_, PurchaseOrderLine>(
            r#"
            SELECT * FROM purchase_order_lines
            WHERE purchase_order_id = $1
            ORDER BY created_at, id
            "#,
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn orders_create(
        &self,
        data: &CreatePurchaseOrder,
        created_by: i64,
    ) -> AppResult<PurchaseOrder> {
        self.vendors_get(data.vendor_id).await?;
        if let Some(fund_id) = data.fund_id {
            let _ = self.funds_get(fund_id).await?;
        }

        let id = next_id();
        let now = Utc::now();
        let year = now.year();
        let order_number = match data
            .order_number
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(n) => n.to_string(),
            None => format!("PO-{year}-{id}"),
        };

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO purchase_orders
                (id, vendor_id, fund_id, order_number, status, notes, created_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $7)
            "#,
        )
        .bind(id)
        .bind(data.vendor_id)
        .bind(data.fund_id)
        .bind(&order_number)
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(created_by)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_unique_order)?;

        if let Some(lines) = data.lines.as_ref() {
            for line in lines {
                insert_order_line_tx(&mut tx, id, line).await?;
            }
        }
        tx.commit().await?;
        self.orders_get(id).await
    }

    pub async fn orders_update(
        &self,
        id: i64,
        data: &UpdatePurchaseOrder,
    ) -> AppResult<PurchaseOrder> {
        let order = self.orders_get(id).await?;
        if !order.status.is_editable() {
            return Err(AppError::BusinessRule(
                "Only draft purchase orders can be updated".into(),
            ));
        }
        if let Some(vendor_id) = data.vendor_id {
            self.vendors_get(vendor_id).await?;
        }
        if let Some(fund_id) = data.fund_id {
            let _ = self.funds_get(fund_id).await?;
        }
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE purchase_orders SET
                vendor_id = COALESCE($2, vendor_id),
                fund_id = CASE WHEN $3 THEN $4 ELSE fund_id END,
                order_number = COALESCE($5, order_number),
                notes = CASE WHEN $6 THEN $7 ELSE notes END,
                updated_at = $8
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(data.vendor_id)
        .bind(data.fund_id.is_some())
        .bind(data.fund_id)
        .bind(
            data.order_number
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
        )
        .bind(data.notes.is_some())
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_unique_order)?;
        self.orders_get(id).await
    }

    pub async fn orders_add_line(
        &self,
        order_id: i64,
        data: &CreateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        let order = self.orders_get(order_id).await?;
        if !order.status.is_editable() {
            return Err(AppError::BusinessRule(
                "Lines can only be added to draft purchase orders".into(),
            ));
        }
        if let Some(fund_id) = data.fund_id {
            let _ = self.funds_get(fund_id).await?;
        }
        let mut tx = self.pool.begin().await?;
        let line = insert_order_line_tx(&mut tx, order_id, data).await?;
        tx.commit().await?;
        Ok(line)
    }

    pub async fn orders_update_line(
        &self,
        order_id: i64,
        line_id: i64,
        data: &UpdateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        let order = self.orders_get(order_id).await?;
        if !order.status.is_editable() {
            return Err(AppError::BusinessRule(
                "Lines can only be updated on draft purchase orders".into(),
            ));
        }
        if let Some(fund_id) = data.fund_id {
            let _ = self.funds_get(fund_id).await?;
        }
        let now = Utc::now();
        sqlx::query_as::<_, PurchaseOrderLine>(
            r#"
            UPDATE purchase_order_lines SET
                fund_id = CASE WHEN $3 THEN $4 ELSE fund_id END,
                biblio_id = CASE WHEN $5 THEN $6 ELSE biblio_id END,
                isbn = CASE WHEN $7 THEN $8 ELSE isbn END,
                title = CASE WHEN $9 THEN $10 ELSE title END,
                quantity_ordered = COALESCE($11, quantity_ordered),
                unit_price = CASE WHEN $12 THEN $13 ELSE unit_price END,
                currency = COALESCE($14, currency),
                notes = CASE WHEN $15 THEN $16 ELSE notes END,
                updated_at = $17
            WHERE id = $1 AND purchase_order_id = $2
            RETURNING *
            "#,
        )
        .bind(line_id)
        .bind(order_id)
        .bind(data.fund_id.is_some())
        .bind(data.fund_id)
        .bind(data.biblio_id.is_some())
        .bind(data.biblio_id)
        .bind(data.isbn.is_some())
        .bind(normalize_isbn(data.isbn.as_deref()))
        .bind(data.title.is_some())
        .bind(empty_to_none(data.title.as_deref()))
        .bind(data.quantity)
        .bind(data.unit_price.is_some())
        .bind(data.unit_price)
        .bind(data.currency.as_deref().map(|c| c.to_ascii_uppercase()))
        .bind(data.notes.is_some())
        .bind(empty_to_none(data.notes.as_deref()))
        .bind(now)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Order line {line_id} not found")))
    }

    pub async fn orders_delete_line(&self, order_id: i64, line_id: i64) -> AppResult<()> {
        let order = self.orders_get(order_id).await?;
        if !order.status.is_editable() {
            return Err(AppError::BusinessRule(
                "Lines can only be removed from draft purchase orders".into(),
            ));
        }
        let rows = sqlx::query(
            "DELETE FROM purchase_order_lines WHERE id = $1 AND purchase_order_id = $2",
        )
        .bind(line_id)
        .bind(order_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if rows == 0 {
            return Err(AppError::NotFound(format!(
                "Order line {line_id} not found"
            )));
        }
        Ok(())
    }

    pub async fn orders_submit(&self, id: i64) -> AppResult<PurchaseOrder> {
        let order = self.orders_get(id).await?;
        if !order.status.can_submit() {
            return Err(AppError::BusinessRule(
                "Only draft purchase orders can be submitted".into(),
            ));
        }
        let lines = self.orders_list_lines(id).await?;
        if lines.is_empty() {
            return Err(AppError::Validation(
                "A purchase order must have at least one line before it is submitted".into(),
            ));
        }
        for line in &lines {
            validate_line_intent(line.biblio_id, line.isbn.as_deref(), line.title.as_deref())?;
        }
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE purchase_orders
            SET status = 'ordered', ordered_at = $2, updated_at = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.orders_get(id).await
    }

    pub async fn orders_cancel(&self, id: i64) -> AppResult<PurchaseOrder> {
        let order = self.orders_get(id).await?;
        if !order.status.can_cancel() {
            return Err(AppError::BusinessRule(
                "Only draft or fully unreceived ordered purchase orders can be cancelled".into(),
            ));
        }
        if order.status == PurchaseOrderStatus::Ordered {
            let lines = self.orders_list_lines(id).await?;
            if lines.iter().any(|l| l.quantity_received > 0) {
                return Err(AppError::BusinessRule(
                    "Cannot cancel a purchase order that already has receipts".into(),
                ));
            }
        }
        let now = Utc::now();
        sqlx::query(
            "UPDATE purchase_orders SET status = 'cancelled', updated_at = $2 WHERE id = $1",
        )
        .bind(id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.orders_get(id).await
    }

    /// Persist a receipt after catalog items have already been created.
    ///
    /// `created_items` is `(po_line_id, item_ids, unit_price)` in the same order as `lines`.
    pub async fn orders_receive(
        &self,
        order_id: i64,
        received_by: i64,
        notes: Option<&str>,
        lines: &[ReceiveOrderLine],
        created_items: &[(i64, Vec<i64>, Option<Decimal>)],
    ) -> AppResult<Receipt> {
        if lines.len() != created_items.len() {
            return Err(AppError::Internal(
                "Receipt line count does not match created items".into(),
            ));
        }

        let mut tx = self.pool.begin().await?;
        let receipt_id = next_id();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO receipts (id, purchase_order_id, received_at, received_by, notes, created_at)
            VALUES ($1, $2, $3, $4, $5, $3)
            "#,
        )
        .bind(receipt_id)
        .bind(order_id)
        .bind(now)
        .bind(received_by)
        .bind(empty_to_none(notes))
        .execute(&mut *tx)
        .await?;

        for (req, (po_line_id, item_ids, unit_price)) in lines.iter().zip(created_items.iter()) {
            if req.line_id != *po_line_id {
                return Err(AppError::Internal(
                    "Receipt line id mismatch while persisting".into(),
                ));
            }
            let receipt_line_id = next_id();
            sqlx::query(
                r#"
                INSERT INTO receipt_lines
                    (id, receipt_id, purchase_order_line_id, quantity, unit_price, created_at)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(receipt_line_id)
            .bind(receipt_id)
            .bind(po_line_id)
            .bind(req.quantity)
            .bind(unit_price)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                UPDATE purchase_order_lines
                SET quantity_received = quantity_received + $2, updated_at = $3
                WHERE id = $1
                "#,
            )
            .bind(po_line_id)
            .bind(req.quantity)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            for item_id in item_ids {
                sqlx::query("INSERT INTO receipt_items (receipt_line_id, item_id) VALUES ($1, $2)")
                    .bind(receipt_line_id)
                    .bind(item_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        let remaining: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM purchase_order_lines
            WHERE purchase_order_id = $1 AND quantity_received < quantity_ordered
            "#,
        )
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;

        let new_status = if remaining == 0 {
            PurchaseOrderStatus::Received
        } else {
            PurchaseOrderStatus::Partial
        };

        sqlx::query("UPDATE purchase_orders SET status = $2, updated_at = $3 WHERE id = $1")
            .bind(order_id)
            .bind(new_status)
            .bind(now)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        sqlx::query_as::<_, Receipt>("SELECT * FROM receipts WHERE id = $1")
            .bind(receipt_id)
            .fetch_one(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn orders_set_line_biblio(&self, line_id: i64, biblio_id: i64) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE purchase_order_lines
            SET biblio_id = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(line_id)
        .bind(biblio_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn receipt_line_ids_for_receipt(
        &self,
        receipt_id: i64,
    ) -> AppResult<Vec<ReceiptLinePersist>> {
        let rows = sqlx::query_as::<_, (i64, i64, i32, Option<Decimal>)>(
            r#"
            SELECT id, purchase_order_line_id, quantity, unit_price
            FROM receipt_lines
            WHERE receipt_id = $1
            ORDER BY id
            "#,
        )
        .bind(receipt_id)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for (id, po_line_id, qty, price) in rows {
            let item_ids = sqlx::query_scalar::<_, i64>(
                "SELECT item_id FROM receipt_items WHERE receipt_line_id = $1 ORDER BY item_id",
            )
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
            out.push((id, po_line_id, qty, price, item_ids));
        }
        Ok(out)
    }
}

async fn insert_order_line_tx(
    tx: &mut Transaction<'_, Postgres>,
    order_id: i64,
    data: &CreateOrderLine,
) -> AppResult<PurchaseOrderLine> {
    validate_line_intent(data.biblio_id, data.isbn.as_deref(), data.title.as_deref())?;
    if data.quantity < 1 {
        return Err(AppError::Validation("quantity must be at least 1".into()));
    }
    if let Some(price) = data.unit_price {
        if price < Decimal::ZERO {
            return Err(AppError::Validation(
                "unitPrice must be greater than or equal to 0".into(),
            ));
        }
    }
    let currency = normalize_currency(data.currency.as_deref())?;
    let id = next_id();
    let now = Utc::now();
    sqlx::query_as::<_, PurchaseOrderLine>(
        r#"
        INSERT INTO purchase_order_lines (
            id, purchase_order_id, fund_id, biblio_id, isbn, title,
            quantity_ordered, quantity_received, unit_price, currency, notes, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8, $9, $10, $11, $11)
        RETURNING *
        "#,
    )
    .bind(id)
    .bind(order_id)
    .bind(data.fund_id)
    .bind(data.biblio_id)
    .bind(normalize_isbn(data.isbn.as_deref()))
    .bind(empty_to_none(data.title.as_deref()))
    .bind(data.quantity)
    .bind(data.unit_price)
    .bind(currency)
    .bind(empty_to_none(data.notes.as_deref()))
    .bind(now)
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}

pub fn validate_line_intent(
    biblio_id: Option<i64>,
    isbn: Option<&str>,
    title: Option<&str>,
) -> AppResult<()> {
    let has_biblio = biblio_id.is_some();
    let has_isbn = isbn.map(str::trim).is_some_and(|s| !s.is_empty());
    let has_title = title.map(str::trim).is_some_and(|s| !s.is_empty());
    if has_biblio || has_isbn || has_title {
        return Ok(());
    }
    Err(AppError::Validation(
        "Each order line must reference a biblio, an ISBN, or a title".into(),
    ))
}

fn empty_to_none(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn normalize_opt_code(s: Option<&str>) -> Option<String> {
    empty_to_none(s)
}

fn normalize_isbn(s: Option<&str>) -> Option<String> {
    s.map(|raw| {
        raw.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect::<String>()
    })
    .filter(|s| !s.is_empty())
}

fn normalize_currency(raw: Option<&str>) -> AppResult<String> {
    let value = raw.unwrap_or("EUR").trim().to_ascii_uppercase();
    if value.len() != 3 || !value.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::Validation(
            "currency must be a 3-letter ISO code".into(),
        ));
    }
    Ok(value)
}

fn map_unique_vendor(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err) {
        return AppError::Conflict("A vendor with this code already exists".into());
    }
    err.into()
}

fn map_unique_fund(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err) {
        return AppError::Conflict(
            "A fund with this code already exists for that fiscal year".into(),
        );
    }
    err.into()
}

fn map_unique_order(err: sqlx::Error) -> AppError {
    if is_unique_violation(&err) {
        return AppError::Conflict("A purchase order with this order number already exists".into());
    }
    err.into()
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")
    )
}

#[cfg(test)]
mod tests {
    use super::validate_line_intent;

    #[test]
    fn line_intent_requires_biblio_isbn_or_title() {
        assert!(validate_line_intent(None, None, None).is_err());
        assert!(validate_line_intent(None, Some("  "), Some("")).is_err());
        assert!(validate_line_intent(Some(1), None, None).is_ok());
        assert!(validate_line_intent(None, Some("9780000000000"), None).is_ok());
        assert!(validate_line_intent(None, None, Some("A title")).is_ok());
    }
}
