//! Acquisitions business logic: vendors, yearly funds, orders, receipt → catalog items.

use std::sync::Arc;

use rust_decimal::Decimal;

use crate::{
    error::{AppError, AppResult},
    models::{
        acquisition::{
            AcquisitionFund, CreateFund, CreateOrderLine, CreatePurchaseOrder, CreateVendor,
            FundQuery, PurchaseOrder, PurchaseOrderDetail, PurchaseOrderLine, PurchaseOrderQuery,
            ReceiptLineResult, ReceiveItemSpec, ReceivePurchaseOrder, ReceivePurchaseOrderResult,
            UpdateFund, UpdateOrderLine, UpdatePurchaseOrder, UpdateVendor, Vendor, VendorQuery,
        },
        biblio::{Biblio, Isbn, MediaType},
        item::Item,
    },
    repository::AcquisitionsRepository,
    services::catalog::CatalogService,
};

#[derive(Clone)]
pub struct AcquisitionsService {
    repository: Arc<dyn AcquisitionsRepository>,
    catalog: CatalogService,
}

impl AcquisitionsService {
    pub fn new(repository: Arc<dyn AcquisitionsRepository>, catalog: CatalogService) -> Self {
        Self {
            repository,
            catalog,
        }
    }

    pub async fn list_vendors(&self, query: &VendorQuery) -> AppResult<(Vec<Vendor>, i64)> {
        self.repository.vendors_list(query).await
    }

    pub async fn get_vendor(&self, id: i64) -> AppResult<Vendor> {
        self.repository.vendors_get(id).await
    }

    pub async fn create_vendor(&self, data: &CreateVendor) -> AppResult<Vendor> {
        if data.name.trim().is_empty() {
            return Err(AppError::Validation("name is required".into()));
        }
        self.repository.vendors_create(data).await
    }

    pub async fn update_vendor(&self, id: i64, data: &UpdateVendor) -> AppResult<Vendor> {
        self.repository.vendors_update(id, data).await
    }

    pub async fn archive_vendor(&self, id: i64) -> AppResult<()> {
        self.repository.vendors_archive(id).await
    }

    pub async fn list_funds(&self, query: &FundQuery) -> AppResult<(Vec<AcquisitionFund>, i64)> {
        self.repository.funds_list(query).await
    }

    pub async fn get_fund(&self, id: i64) -> AppResult<AcquisitionFund> {
        self.repository.funds_get(id).await
    }

    pub async fn create_fund(&self, data: &CreateFund) -> AppResult<AcquisitionFund> {
        if data.fiscal_year < 1900 || data.fiscal_year > 3000 {
            return Err(AppError::Validation(
                "fiscalYear must be a four-digit year".into(),
            ));
        }
        self.repository.funds_create(data).await
    }

    pub async fn update_fund(&self, id: i64, data: &UpdateFund) -> AppResult<AcquisitionFund> {
        if let Some(year) = data.fiscal_year {
            if !(1900..=3000).contains(&year) {
                return Err(AppError::Validation(
                    "fiscalYear must be a four-digit year".into(),
                ));
            }
        }
        self.repository.funds_update(id, data).await
    }

    pub async fn list_orders(
        &self,
        query: &PurchaseOrderQuery,
    ) -> AppResult<(Vec<PurchaseOrder>, i64)> {
        self.repository.orders_list(query).await
    }

    pub async fn get_order(&self, id: i64) -> AppResult<PurchaseOrderDetail> {
        let order = self.repository.orders_get(id).await?;
        let lines = self.repository.orders_list_lines(id).await?;
        Ok(PurchaseOrderDetail { order, lines })
    }

    pub async fn create_order(
        &self,
        data: &CreatePurchaseOrder,
        created_by: i64,
    ) -> AppResult<PurchaseOrderDetail> {
        if let Some(lines) = data.lines.as_ref() {
            for line in lines {
                self.validate_new_line(line).await?;
            }
        }
        let order = self.repository.orders_create(data, created_by).await?;
        self.get_order(order.id).await
    }

    pub async fn update_order(
        &self,
        id: i64,
        data: &UpdatePurchaseOrder,
    ) -> AppResult<PurchaseOrderDetail> {
        self.repository.orders_update(id, data).await?;
        self.get_order(id).await
    }

    pub async fn add_line(
        &self,
        order_id: i64,
        data: &CreateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        self.validate_new_line(data).await?;
        self.repository.orders_add_line(order_id, data).await
    }

    pub async fn update_line(
        &self,
        order_id: i64,
        line_id: i64,
        data: &UpdateOrderLine,
    ) -> AppResult<PurchaseOrderLine> {
        if let Some(biblio_id) = data.biblio_id {
            self.catalog.get_biblio(biblio_id).await?;
        }
        self.repository
            .orders_update_line(order_id, line_id, data)
            .await
    }

    pub async fn delete_line(&self, order_id: i64, line_id: i64) -> AppResult<()> {
        self.repository.orders_delete_line(order_id, line_id).await
    }

    pub async fn submit_order(&self, id: i64) -> AppResult<PurchaseOrderDetail> {
        self.repository.orders_submit(id).await?;
        self.get_order(id).await
    }

    pub async fn cancel_order(&self, id: i64) -> AppResult<PurchaseOrderDetail> {
        self.repository.orders_cancel(id).await?;
        self.get_order(id).await
    }

    /// Mark lines received and create catalog items (only at receipt).
    pub async fn receive_order(
        &self,
        order_id: i64,
        received_by: i64,
        data: &ReceivePurchaseOrder,
    ) -> AppResult<ReceivePurchaseOrderResult> {
        let detail = self.get_order(order_id).await?;
        if !detail.order.status.can_receive() {
            return Err(AppError::BusinessRule(
                "Only ordered or partially received purchase orders can be received".into(),
            ));
        }

        let mut created: Vec<(i64, Vec<i64>, Option<Decimal>)> =
            Vec::with_capacity(data.lines.len());

        for req in &data.lines {
            let line = detail
                .lines
                .iter()
                .find(|l| l.id == req.line_id)
                .ok_or_else(|| {
                    AppError::Validation(format!(
                        "Line {} does not belong to purchase order {order_id}",
                        req.line_id
                    ))
                })?;

            let remaining = line.quantity_ordered - line.quantity_received;
            if req.quantity > remaining {
                return Err(AppError::BusinessRule(format!(
                    "Cannot receive {} copies on line {}: only {remaining} remaining",
                    req.quantity, line.id
                )));
            }

            if let Some(specs) = req.items.as_ref() {
                if specs.len() != req.quantity as usize {
                    return Err(AppError::Validation(format!(
                        "items length must equal quantity for line {}",
                        line.id
                    )));
                }
            }

            let unit_price = req.unit_price.or(line.unit_price);
            if let Some(price) = unit_price {
                if price < Decimal::ZERO {
                    return Err(AppError::Validation(
                        "unitPrice must be greater than or equal to 0".into(),
                    ));
                }
            }

            let biblio_id = self.resolve_line_biblio(line).await?;
            if line.biblio_id != Some(biblio_id) {
                self.repository
                    .orders_set_line_biblio(line.id, biblio_id)
                    .await?;
            }

            let specs: Vec<ReceiveItemSpec> = match req.items.as_ref() {
                Some(items) => items.clone(),
                None => (0..req.quantity)
                    .map(|_| ReceiveItemSpec {
                        barcode: None,
                        source_id: None,
                        source_name: None,
                        price: unit_price.map(|p| p.normalize().to_string()),
                        call_number: None,
                    })
                    .collect(),
            };

            let mut item_ids = Vec::with_capacity(specs.len());
            for (idx, spec) in specs.iter().enumerate() {
                let barcode = spec.barcode.clone().filter(|s| !s.trim().is_empty());
                let barcode = barcode.or_else(|| {
                    Some(format!(
                        "ACQ-{}-{}-{}",
                        line.id,
                        line.quantity_received + idx as i32 + 1,
                        crate::repository::acquisitions::short_suffix()
                    ))
                });
                let price = spec
                    .price
                    .clone()
                    .or_else(|| unit_price.map(|p| p.normalize().to_string()));

                let item = Item {
                    id: None,
                    biblio_id: Some(biblio_id),
                    source_id: spec.source_id,
                    barcode,
                    call_number: spec.call_number.clone(),
                    volume_designation: None,
                    place: None,
                    borrowable: true,
                    circulation_status: None,
                    notes: Some(format!(
                        "Received from purchase order {}",
                        detail.order.order_number
                    )),
                    price,
                    created_at: None,
                    updated_at: None,
                    archived_at: None,
                    source_name: spec.source_name.clone(),
                    borrowed: false,
                    loan_id: None,
                };

                let created_item = self.catalog.create_item(biblio_id, item).await?;
                let item_id = created_item
                    .id
                    .ok_or_else(|| AppError::Internal("Created item is missing id".into()))?;
                item_ids.push(item_id);
            }

            created.push((line.id, item_ids, unit_price));
        }

        let receipt = self
            .repository
            .orders_receive(
                order_id,
                received_by,
                data.notes.clone(),
                &data.lines,
                &created,
            )
            .await?;

        let stored = self.repository.receipt_line_results(receipt.id).await?;
        let lines = stored
            .into_iter()
            .map(
                |(id, po_line_id, quantity, unit_price, item_ids)| ReceiptLineResult {
                    id,
                    purchase_order_line_id: po_line_id,
                    quantity,
                    unit_price,
                    item_ids,
                },
            )
            .collect();

        let order = self.get_order(order_id).await?;
        Ok(ReceivePurchaseOrderResult {
            receipt,
            lines,
            order,
        })
    }

    async fn validate_new_line(&self, data: &CreateOrderLine) -> AppResult<()> {
        crate::repository::acquisitions::validate_line_intent(
            data.biblio_id,
            data.isbn.as_deref(),
            data.title.as_deref(),
        )?;
        if let Some(biblio_id) = data.biblio_id {
            self.catalog.get_biblio(biblio_id).await?;
        }
        Ok(())
    }

    async fn resolve_line_biblio(&self, line: &PurchaseOrderLine) -> AppResult<i64> {
        if let Some(biblio_id) = line.biblio_id {
            self.catalog.get_biblio(biblio_id).await?;
            return Ok(biblio_id);
        }

        if let Some(isbn) = line.isbn.as_deref().filter(|s| !s.is_empty()) {
            if let Some(existing) = self.catalog.find_active_biblio_id_by_isbn(isbn).await? {
                return Ok(existing);
            }
            let stub = stub_biblio(
                line.title
                    .clone()
                    .unwrap_or_else(|| format!("Acquisition {isbn}")),
                Some(Isbn::new(isbn)),
            );
            let (created, _) = self.catalog.create_biblio(stub, true, None).await?;
            return created
                .id
                .ok_or_else(|| AppError::Internal("Created biblio is missing id".into()));
        }

        let title = line
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let title = title.ok_or_else(|| {
            AppError::Validation("Cannot receive a line without a biblio, ISBN, or title".into())
        })?;
        let stub = stub_biblio(title.to_string(), None);
        let (created, _) = self.catalog.create_biblio(stub, true, None).await?;
        created
            .id
            .ok_or_else(|| AppError::Internal("Created biblio is missing id".into()))
    }
}

fn stub_biblio(title: String, isbn: Option<Isbn>) -> Biblio {
    Biblio {
        id: None,
        media_type: MediaType::PrintedText,
        isbn,
        title: Some(title),
        subject: None,
        dewey: None,
        audience_type: None,
        lang: None,
        lang_orig: None,
        publication_date: None,
        page_extent: None,
        format: None,
        table_of_contents: None,
        accompanying_material: None,
        abstract_: None,
        notes: Some("Created from acquisitions receipt".into()),
        keywords: None,
        is_valid: Some(true),
        series_ids: Vec::new(),
        series_volume_numbers: Vec::new(),
        edition_id: None,
        collection_ids: Vec::new(),
        collection_volume_numbers: Vec::new(),
        created_at: None,
        updated_at: None,
        archived_at: None,
        authors: Vec::new(),
        series: Vec::new(),
        collections: Vec::new(),
        edition: None,
        items: Vec::new(),
        marc_record: None,
    }
}
