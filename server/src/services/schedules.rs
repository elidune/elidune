//! Schedules service (periods, slots, closures)

use chrono::NaiveDate;

use crate::{
    error::AppResult,
    models::schedule::{
        CreateScheduleClosure, CreateSchedulePeriod, CreateScheduleSlot,
        ScheduleClosure, SchedulePeriod, ScheduleSlot, UpdateSchedulePeriod,
    },
    repository::Repository,
};

#[derive(Clone)]
pub struct SchedulesService {
    repository: Repository,
}

impl SchedulesService {
    pub fn new(repository: Repository) -> Self {
        Self { repository }
    }

    // ---- Periods ----
    pub async fn list_periods(&self) -> AppResult<Vec<SchedulePeriod>> {
        self.repository.schedules.list_periods().await
    }

    pub async fn get_period(&self, id: i32) -> AppResult<SchedulePeriod> {
        self.repository.schedules.get_period(id).await
    }

    pub async fn create_period(&self, data: &CreateSchedulePeriod) -> AppResult<SchedulePeriod> {
        self.repository.schedules.create_period(data).await
    }

    pub async fn update_period(&self, id: i32, data: &UpdateSchedulePeriod) -> AppResult<SchedulePeriod> {
        self.repository.schedules.update_period(id, data).await
    }

    pub async fn delete_period(&self, id: i32) -> AppResult<()> {
        self.repository.schedules.delete_period(id).await
    }

    // ---- Slots ----
    pub async fn list_slots(&self, period_id: i32) -> AppResult<Vec<ScheduleSlot>> {
        self.repository.schedules.list_slots(period_id).await
    }

    pub async fn create_slot(&self, period_id: i32, data: &CreateScheduleSlot) -> AppResult<ScheduleSlot> {
        self.repository.schedules.create_slot(period_id, data).await
    }

    pub async fn delete_slot(&self, id: i32) -> AppResult<()> {
        self.repository.schedules.delete_slot(id).await
    }

    // ---- Closures ----
    pub async fn list_closures(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> AppResult<Vec<ScheduleClosure>> {
        self.repository.schedules.list_closures(start_date, end_date).await
    }

    pub async fn create_closure(&self, data: &CreateScheduleClosure) -> AppResult<ScheduleClosure> {
        self.repository.schedules.create_closure(data).await
    }

    pub async fn delete_closure(&self, id: i32) -> AppResult<()> {
        self.repository.schedules.delete_closure(id).await
    }

    // ---- Stats helpers ----
    pub async fn count_opening_days(&self, year: i32) -> AppResult<i64> {
        self.repository.schedules.count_opening_days(year).await
    }

    pub async fn weekly_hours(&self, year: i32) -> AppResult<f64> {
        self.repository.schedules.weekly_hours(year).await
    }
}
