//! Schedule API endpoints (periods, slots, closures)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;

use crate::{
    error::AppResult,
    models::schedule::{
        CreateScheduleClosure, CreateSchedulePeriod, CreateScheduleSlot,
        ScheduleClosure, ScheduleClosureQuery, SchedulePeriod, ScheduleSlot,
        UpdateSchedulePeriod,
    },
};

use super::AuthenticatedUser;

// ---- Periods ----

/// List schedule periods
#[utoipa::path(
    get,
    path = "/schedules/periods",
    tag = "schedules",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Schedule periods", body = Vec<SchedulePeriod>)
    )
)]
pub async fn list_periods(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
) -> AppResult<Json<Vec<SchedulePeriod>>> {
    claims.require_read_settings()?;
    let periods = state.services.schedules.list_periods().await?;
    Ok(Json(periods))
}

/// Create a schedule period
#[utoipa::path(
    post,
    path = "/schedules/periods",
    tag = "schedules",
    security(("bearer_auth" = [])),
    request_body = CreateSchedulePeriod,
    responses(
        (status = 201, description = "Period created", body = SchedulePeriod)
    )
)]
pub async fn create_period(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(data): Json<CreateSchedulePeriod>,
) -> AppResult<(StatusCode, Json<SchedulePeriod>)> {
    claims.require_write_settings()?;
    let period = state.services.schedules.create_period(&data).await?;
    Ok((StatusCode::CREATED, Json(period)))
}

/// Update a schedule period
#[utoipa::path(
    put,
    path = "/schedules/periods/{id}",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Period ID")),
    request_body = UpdateSchedulePeriod,
    responses(
        (status = 200, description = "Period updated", body = SchedulePeriod)
    )
)]
pub async fn update_period(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
    Json(data): Json<UpdateSchedulePeriod>,
) -> AppResult<Json<SchedulePeriod>> {
    claims.require_write_settings()?;
    let period = state.services.schedules.update_period(id, &data).await?;
    Ok(Json(period))
}

/// Delete a schedule period
#[utoipa::path(
    delete,
    path = "/schedules/periods/{id}",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Period ID")),
    responses(
        (status = 204, description = "Period deleted")
    )
)]
pub async fn delete_period(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
) -> AppResult<StatusCode> {
    claims.require_write_settings()?;
    state.services.schedules.delete_period(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- Slots ----

/// List slots for a period
#[utoipa::path(
    get,
    path = "/schedules/periods/{id}/slots",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Period ID")),
    responses(
        (status = 200, description = "Period slots", body = Vec<ScheduleSlot>)
    )
)]
pub async fn list_slots(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(period_id): Path<i32>,
) -> AppResult<Json<Vec<ScheduleSlot>>> {
    claims.require_read_settings()?;
    let slots = state.services.schedules.list_slots(period_id).await?;
    Ok(Json(slots))
}

/// Create a slot for a period
#[utoipa::path(
    post,
    path = "/schedules/periods/{id}/slots",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Period ID")),
    request_body = CreateScheduleSlot,
    responses(
        (status = 201, description = "Slot created", body = ScheduleSlot)
    )
)]
pub async fn create_slot(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(period_id): Path<i32>,
    Json(data): Json<CreateScheduleSlot>,
) -> AppResult<(StatusCode, Json<ScheduleSlot>)> {
    claims.require_write_settings()?;
    let slot = state.services.schedules.create_slot(period_id, &data).await?;
    Ok((StatusCode::CREATED, Json(slot)))
}

/// Delete a slot
#[utoipa::path(
    delete,
    path = "/schedules/slots/{id}",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Slot ID")),
    responses(
        (status = 204, description = "Slot deleted")
    )
)]
pub async fn delete_slot(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
) -> AppResult<StatusCode> {
    claims.require_write_settings()?;
    state.services.schedules.delete_slot(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- Closures ----

/// List schedule closures
#[utoipa::path(
    get,
    path = "/schedules/closures",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(ScheduleClosureQuery),
    responses(
        (status = 200, description = "Closures list", body = Vec<ScheduleClosure>)
    )
)]
pub async fn list_closures(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Query(query): Query<ScheduleClosureQuery>,
) -> AppResult<Json<Vec<ScheduleClosure>>> {
    claims.require_read_settings()?;
    let start = query.start_date.as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let end = query.end_date.as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let closures = state.services.schedules.list_closures(start, end).await?;
    Ok(Json(closures))
}

/// Create a closure
#[utoipa::path(
    post,
    path = "/schedules/closures",
    tag = "schedules",
    security(("bearer_auth" = [])),
    request_body = CreateScheduleClosure,
    responses(
        (status = 201, description = "Closure created", body = ScheduleClosure)
    )
)]
pub async fn create_closure(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Json(data): Json<CreateScheduleClosure>,
) -> AppResult<(StatusCode, Json<ScheduleClosure>)> {
    claims.require_write_settings()?;
    let closure = state.services.schedules.create_closure(&data).await?;
    Ok((StatusCode::CREATED, Json(closure)))
}

/// Delete a closure
#[utoipa::path(
    delete,
    path = "/schedules/closures/{id}",
    tag = "schedules",
    security(("bearer_auth" = [])),
    params(("id" = i32, Path, description = "Closure ID")),
    responses(
        (status = 204, description = "Closure deleted")
    )
)]
pub async fn delete_closure(
    State(state): State<crate::AppState>,
    AuthenticatedUser(claims): AuthenticatedUser,
    Path(id): Path<i32>,
) -> AppResult<StatusCode> {
    claims.require_write_settings()?;
    state.services.schedules.delete_closure(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
