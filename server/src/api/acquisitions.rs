//! Acquisitions API: vendors, yearly funds, purchase orders, and receipt.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    error::AppResult,
    models::acquisition::{
        AcquisitionFund, CreateFund, CreateOrderLine, CreatePurchaseOrder, CreateVendor,
        FundListResponse, FundQuery, PurchaseOrderDetail, PurchaseOrderLine,
        PurchaseOrderListResponse, PurchaseOrderQuery, ReceivePurchaseOrder,
        ReceivePurchaseOrderResult, UpdateFund, UpdateOrderLine, UpdatePurchaseOrder, UpdateVendor,
        Vendor, VendorListResponse, VendorQuery,
    },
    models::audit::AuditQueryParams,
    services::audit,
};

#[allow(unused_imports)]
use crate::error::ErrorResponse;

use super::{AuthenticatedUser, ClientIp, ValidatedJson};

/// Build the acquisitions routes for this domain.
pub fn router() -> axum::Router<crate::AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route(
            "/acquisitions/vendors",
            get(list_vendors).post(create_vendor),
        )
        .route(
            "/acquisitions/vendors/:id",
            get(get_vendor).put(update_vendor).delete(archive_vendor),
        )
        .route("/acquisitions/funds", get(list_funds).post(create_fund))
        .route("/acquisitions/funds/:id", get(get_fund).put(update_fund))
        .route("/acquisitions/orders", get(list_orders).post(create_order))
        .route("/acquisitions/orders/:id", get(get_order).put(update_order))
        .route("/acquisitions/orders/:id/lines", post(add_order_line))
        .route(
            "/acquisitions/orders/:id/lines/:line_id",
            axum::routing::put(update_order_line).delete(delete_order_line),
        )
        .route("/acquisitions/orders/:id/submit", post(submit_order))
        .route("/acquisitions/orders/:id/cancel", post(cancel_order))
        .route("/acquisitions/orders/:id/receive", post(receive_order))
        .route("/acquisitions/orders/:id/audit", get(order_audit))
}

#[utoipa::path(
    get,
    path = "/acquisitions/vendors",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(VendorQuery),
    responses(
        (status = 200, description = "Vendor list", body = VendorListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
    )
)]
pub async fn list_vendors(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<VendorQuery>,
) -> AppResult<Json<VendorListResponse>> {
    claims.require_read_acquisitions()?;
    let (vendors, total) = state.services.acquisitions.list_vendors(&query).await?;
    Ok(Json(VendorListResponse { vendors, total }))
}

#[utoipa::path(
    get,
    path = "/acquisitions/vendors/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Vendor id")),
    responses(
        (status = 200, description = "Vendor", body = Vendor),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Vendor not found", body = ErrorResponse),
    )
)]
pub async fn get_vendor(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<Vendor>> {
    claims.require_read_acquisitions()?;
    Ok(Json(state.services.acquisitions.get_vendor(id).await?))
}

#[utoipa::path(
    post,
    path = "/acquisitions/vendors",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    request_body = CreateVendor,
    responses(
        (status = 201, description = "Vendor created", body = Vendor),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 409, description = "Vendor code already exists", body = ErrorResponse),
    )
)]
pub async fn create_vendor(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreateVendor>,
) -> AppResult<(StatusCode, Json<Vendor>)> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.create_vendor(&data).await {
        Ok(vendor) => {
            state.services.audit.log(
                audit::event::VENDOR_CREATED,
                Some(claims.user_id),
                Some("vendor"),
                Some(vendor.id),
                ip,
                Some(&vendor),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(vendor)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::VENDOR_CREATED,
                Some(claims.user_id),
                Some("vendor"),
                None,
                ip,
                Some(&data.name),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    put,
    path = "/acquisitions/vendors/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Vendor id")),
    request_body = UpdateVendor,
    responses(
        (status = 200, description = "Vendor updated", body = Vendor),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Vendor not found", body = ErrorResponse),
    )
)]
pub async fn update_vendor(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<UpdateVendor>,
) -> AppResult<Json<Vendor>> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.update_vendor(id, &data).await {
        Ok(vendor) => {
            state.services.audit.log(
                audit::event::VENDOR_UPDATED,
                Some(claims.user_id),
                Some("vendor"),
                Some(id),
                ip,
                Some(&vendor),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(vendor))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::VENDOR_UPDATED,
                Some(claims.user_id),
                Some("vendor"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/acquisitions/vendors/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Vendor id")),
    responses(
        (status = 204, description = "Vendor archived"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Vendor not found", body = ErrorResponse),
    )
)]
pub async fn archive_vendor(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.archive_vendor(id).await {
        Ok(()) => {
            state.services.audit.log(
                audit::event::VENDOR_ARCHIVED,
                Some(claims.user_id),
                Some("vendor"),
                Some(id),
                ip,
                Some(id),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::VENDOR_ARCHIVED,
                Some(claims.user_id),
                Some("vendor"),
                Some(id),
                ip,
                Some(id),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    get,
    path = "/acquisitions/funds",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(FundQuery),
    responses(
        (status = 200, description = "Fund list with committed / spent / available", body = FundListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
    )
)]
pub async fn list_funds(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<FundQuery>,
) -> AppResult<Json<FundListResponse>> {
    claims.require_read_acquisitions()?;
    let (funds, total) = state.services.acquisitions.list_funds(&query).await?;
    Ok(Json(FundListResponse { funds, total }))
}

#[utoipa::path(
    get,
    path = "/acquisitions/funds/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Fund id")),
    responses(
        (status = 200, description = "Fund with committed / spent / available", body = AcquisitionFund),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Fund not found", body = ErrorResponse),
    )
)]
pub async fn get_fund(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<AcquisitionFund>> {
    claims.require_read_acquisitions()?;
    Ok(Json(state.services.acquisitions.get_fund(id).await?))
}

#[utoipa::path(
    post,
    path = "/acquisitions/funds",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    request_body = CreateFund,
    responses(
        (status = 201, description = "Fund created", body = AcquisitionFund),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 409, description = "Fund code already exists for that year", body = ErrorResponse),
    )
)]
pub async fn create_fund(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreateFund>,
) -> AppResult<(StatusCode, Json<AcquisitionFund>)> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.create_fund(&data).await {
        Ok(fund) => {
            state.services.audit.log(
                audit::event::FUND_CREATED,
                Some(claims.user_id),
                Some("acquisition_fund"),
                Some(fund.id),
                ip,
                Some(&fund),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(fund)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::FUND_CREATED,
                Some(claims.user_id),
                Some("acquisition_fund"),
                None,
                ip,
                Some(&data.code),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    put,
    path = "/acquisitions/funds/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Fund id")),
    request_body = UpdateFund,
    responses(
        (status = 200, description = "Fund updated", body = AcquisitionFund),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Fund not found", body = ErrorResponse),
    )
)]
pub async fn update_fund(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<UpdateFund>,
) -> AppResult<Json<AcquisitionFund>> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.update_fund(id, &data).await {
        Ok(fund) => {
            state.services.audit.log(
                audit::event::FUND_UPDATED,
                Some(claims.user_id),
                Some("acquisition_fund"),
                Some(id),
                ip,
                Some(&fund),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(fund))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::FUND_UPDATED,
                Some(claims.user_id),
                Some("acquisition_fund"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    get,
    path = "/acquisitions/orders",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(PurchaseOrderQuery),
    responses(
        (status = 200, description = "Purchase order list", body = PurchaseOrderListResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
    )
)]
pub async fn list_orders(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<PurchaseOrderQuery>,
) -> AppResult<Json<PurchaseOrderListResponse>> {
    claims.require_read_acquisitions()?;
    let (orders, total) = state.services.acquisitions.list_orders(&query).await?;
    Ok(Json(PurchaseOrderListResponse { orders, total }))
}

#[utoipa::path(
    get,
    path = "/acquisitions/orders/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    responses(
        (status = 200, description = "Purchase order with lines", body = PurchaseOrderDetail),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
    )
)]
pub async fn get_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<PurchaseOrderDetail>> {
    claims.require_read_acquisitions()?;
    Ok(Json(state.services.acquisitions.get_order(id).await?))
}

#[utoipa::path(
    post,
    path = "/acquisitions/orders",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    request_body = CreatePurchaseOrder,
    responses(
        (status = 201, description = "Draft purchase order created", body = PurchaseOrderDetail),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
    )
)]
pub async fn create_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    ValidatedJson(data): ValidatedJson<CreatePurchaseOrder>,
) -> AppResult<(StatusCode, Json<PurchaseOrderDetail>)> {
    claims.require_write_acquisitions()?;
    match state
        .services
        .acquisitions
        .create_order(&data, claims.user_id)
        .await
    {
        Ok(order) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_CREATED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(order.order.id),
                ip,
                Some(&order),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(order)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_CREATED,
                Some(claims.user_id),
                Some("purchase_order"),
                None,
                ip,
                Some(data.vendor_id),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    put,
    path = "/acquisitions/orders/{id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    request_body = UpdatePurchaseOrder,
    responses(
        (status = 200, description = "Draft purchase order updated", body = PurchaseOrderDetail),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
        (status = 422, description = "Order is not a draft", body = ErrorResponse),
    )
)]
pub async fn update_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<UpdatePurchaseOrder>,
) -> AppResult<Json<PurchaseOrderDetail>> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.update_order(id, &data).await {
        Ok(order) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_UPDATED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&order),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(order))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_UPDATED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/acquisitions/orders/{id}/lines",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    request_body = CreateOrderLine,
    responses(
        (status = 201, description = "Line added", body = PurchaseOrderLine),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
        (status = 422, description = "Order is not a draft", body = ErrorResponse),
    )
)]
pub async fn add_order_line(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<CreateOrderLine>,
) -> AppResult<(StatusCode, Json<PurchaseOrderLine>)> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.add_line(id, &data).await {
        Ok(line) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_ADDED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&line),
                audit::AuditLogMeta::success(),
            );
            Ok((StatusCode::CREATED, Json(line)))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_ADDED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    put,
    path = "/acquisitions/orders/{id}/lines/{line_id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Purchase order id"),
        ("line_id" = String, Path, description = "Order line id"),
    ),
    request_body = UpdateOrderLine,
    responses(
        (status = 200, description = "Line updated", body = PurchaseOrderLine),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Line not found", body = ErrorResponse),
    )
)]
pub async fn update_order_line(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path((id, line_id)): Path<(i64, i64)>,
    ValidatedJson(data): ValidatedJson<UpdateOrderLine>,
) -> AppResult<Json<PurchaseOrderLine>> {
    claims.require_write_acquisitions()?;
    match state
        .services
        .acquisitions
        .update_line(id, line_id, &data)
        .await
    {
        Ok(line) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_UPDATED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&line),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(line))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_UPDATED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    delete,
    path = "/acquisitions/orders/{id}/lines/{line_id}",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(
        ("id" = String, Path, description = "Purchase order id"),
        ("line_id" = String, Path, description = "Order line id"),
    ),
    responses(
        (status = 204, description = "Line removed"),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Line not found", body = ErrorResponse),
    )
)]
pub async fn delete_order_line(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path((id, line_id)): Path<(i64, i64)>,
) -> AppResult<StatusCode> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.delete_line(id, line_id).await {
        Ok(()) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_REMOVED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(line_id),
                audit::AuditLogMeta::success(),
            );
            Ok(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_LINE_REMOVED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(line_id),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/acquisitions/orders/{id}/submit",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    responses(
        (status = 200, description = "Order submitted (budget committed)", body = PurchaseOrderDetail),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
        (status = 422, description = "Order cannot be submitted", body = ErrorResponse),
    )
)]
pub async fn submit_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<PurchaseOrderDetail>> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.submit_order(id).await {
        Ok(order) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_SUBMITTED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&order),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(order))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_SUBMITTED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(id),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/acquisitions/orders/{id}/cancel",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    responses(
        (status = 200, description = "Order cancelled (commitment released)", body = PurchaseOrderDetail),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
        (status = 422, description = "Order cannot be cancelled", body = ErrorResponse),
    )
)]
pub async fn cancel_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
) -> AppResult<Json<PurchaseOrderDetail>> {
    claims.require_write_acquisitions()?;
    match state.services.acquisitions.cancel_order(id).await {
        Ok(order) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_CANCELLED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&order),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(order))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_CANCELLED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(id),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    post,
    path = "/acquisitions/orders/{id}/receive",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    request_body = ReceivePurchaseOrder,
    responses(
        (status = 200, description = "Lines received and catalog items created", body = ReceivePurchaseOrderResult),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
        (status = 422, description = "Order cannot be received", body = ErrorResponse),
    )
)]
pub async fn receive_order(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    ClientIp(ip): ClientIp,
    Path(id): Path<i64>,
    ValidatedJson(data): ValidatedJson<ReceivePurchaseOrder>,
) -> AppResult<Json<ReceivePurchaseOrderResult>> {
    claims.require_write_acquisitions()?;
    match state
        .services
        .acquisitions
        .receive_order(id, claims.user_id, &data)
        .await
    {
        Ok(result) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_RECEIVED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&result),
                audit::AuditLogMeta::success(),
            );
            Ok(Json(result))
        }
        Err(e) => {
            state.services.audit.log(
                audit::event::PURCHASE_ORDER_RECEIVED,
                Some(claims.user_id),
                Some("purchase_order"),
                Some(id),
                ip,
                Some(&data),
                audit::AuditLogMeta::from_app_error(&e),
            );
            Err(e)
        }
    }
}

#[utoipa::path(
    get,
    path = "/acquisitions/orders/{id}/audit",
    tag = "acquisitions",
    security(("bearer_auth" = [])),
    params(("id" = String, Path, description = "Purchase order id")),
    responses(
        (status = 200, description = "Order and receipt audit events", body = crate::models::audit::AuditLogPage),
        (status = 401, description = "Not authenticated", body = ErrorResponse),
        (status = 403, description = "Insufficient acquisitions rights", body = ErrorResponse),
        (status = 404, description = "Purchase order not found", body = ErrorResponse),
    )
)]
pub async fn order_audit(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i64>,
) -> AppResult<Json<crate::models::audit::AuditLogPage>> {
    claims.require_read_acquisitions()?;
    let _ = state.services.acquisitions.get_order(id).await?;
    let page = state
        .services
        .audit
        .query(AuditQueryParams {
            event_type: None,
            entity_type: Some("purchase_order".into()),
            entity_id: Some(id),
            user_id: None,
            from_date: None,
            to_date: None,
            outcome: None,
            error_code: None,
            page: Some(1),
            per_page: Some(100),
        })
        .await?;
    Ok(Json(page))
}
