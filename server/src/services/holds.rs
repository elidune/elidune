//! Hold queue service (physical item holds).

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::{
        dto::holds::{HoldQuota, HoldsPolicy},
        hold::{CreateHold, Hold, HoldDetails, MAX_MAX_ACTIVE_HOLDS, MIN_MAX_ACTIVE_HOLDS},
    },
    repository::HoldsRepository,
    services::audit::{self, AuditLogMeta, AuditService},
};

#[derive(Clone)]
pub struct HoldsService {
    repository: Arc<dyn HoldsRepository>,
    audit: Option<AuditService>,
}

impl HoldsService {
    pub fn new(repository: Arc<dyn HoldsRepository>) -> Self {
        Self {
            repository,
            audit: None,
        }
    }

    pub fn with_audit(repository: Arc<dyn HoldsRepository>, audit: AuditService) -> Self {
        Self {
            repository,
            audit: Some(audit),
        }
    }

    /// Paginated list of all holds (newest first).
    #[tracing::instrument(skip(self), err)]
    pub async fn list_all(
        &self,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        self.repository
            .holds_list_all(page, per_page, active_only)
            .await
    }

    /// Paginated holds for one user (`holds_rights == own` on `GET /holds`).
    #[tracing::instrument(skip(self), err)]
    pub async fn list_for_user_paginated(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
        active_only: bool,
    ) -> AppResult<(Vec<HoldDetails>, i64)> {
        self.repository
            .holds_list_for_user_paginated(user_id, page, per_page, active_only)
            .await
    }

    /// Global default cap (public-type overrides are applied in [`Self::quota_for_user`]).
    #[tracing::instrument(skip(self), err)]
    pub async fn get_policy(&self) -> AppResult<HoldsPolicy> {
        Ok(HoldsPolicy {
            max_active_holds: self.repository.holds_get_global_max_active().await?,
        })
    }

    /// Replace the global default max active holds.
    #[tracing::instrument(skip(self), err)]
    pub async fn set_policy(&self, max_active_holds: i16) -> AppResult<HoldsPolicy> {
        validate_max_active_holds(Some(max_active_holds))?;
        Ok(HoldsPolicy {
            max_active_holds: self
                .repository
                .holds_set_global_max_active(max_active_holds)
                .await?,
        })
    }

    /// Resolved cap, used count, and remaining slots for a patron.
    #[tracing::instrument(skip(self), err)]
    pub async fn quota_for_user(&self, user_id: i64) -> AppResult<HoldQuota> {
        let (active_holds, max_active_holds) = tokio::try_join!(
            self.repository.holds_count_active_for_user(user_id),
            self.repository.holds_get_max_active_for_user(user_id),
        )?;
        Ok(HoldQuota::from_counts(
            user_id,
            max_active_holds,
            active_holds,
        ))
    }

    /// Place a hold — rejects if the user already has a pending/ready hold for this item,
    /// or if they are at/over the active-hold cap (unless `force`).
    /// Uniqueness is also enforced in the database (`idx_holds_one_active_per_user_item`).
    #[tracing::instrument(skip(self), err)]
    pub async fn place_hold(
        &self,
        data: CreateHold,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<Hold> {
        if self
            .repository
            .holds_has_active_for_user_item(data.user_id, data.item_id)
            .await?
        {
            return Err(AppError::Conflict(
                "User already has an active hold for this item".to_string(),
            ));
        }

        self.enforce_max_active_holds(&data, audit_actor, client_ip)
            .await?;

        self.repository.holds_create(&data).await
    }

    /// Cap of `pending`/`ready` holds by patron category. `force=true` bypasses and is audited.
    async fn enforce_max_active_holds(
        &self,
        data: &CreateHold,
        audit_actor: Option<i64>,
        client_ip: Option<String>,
    ) -> AppResult<()> {
        let (active, max) = tokio::try_join!(
            self.repository.holds_count_active_for_user(data.user_id),
            self.repository.holds_get_max_active_for_user(data.user_id),
        )?;
        if active < i64::from(max) {
            return Ok(());
        }
        if !data.force {
            return Err(AppError::BusinessRule(hold_cap_block_message(active, max)));
        }
        if let Some(audit) = &self.audit {
            audit.log(
                audit::event::HOLD_MAX_OVERRIDDEN,
                audit_actor,
                Some("user"),
                Some(data.user_id),
                client_ip,
                Some(serde_json::json!({
                    "active": active,
                    "max": max,
                    "itemId": data.item_id.to_string(),
                    "operation": "place_hold",
                })),
                AuditLogMeta::success(),
            );
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_for_item(&self, item_id: i64) -> AppResult<Vec<HoldDetails>> {
        self.repository.holds_list_for_item(item_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn get_for_user(&self, user_id: i64) -> AppResult<Vec<HoldDetails>> {
        self.repository.holds_list_for_user(user_id).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn cancel(
        &self,
        id: i64,
        requesting_user_id: i64,
        can_manage_others: bool,
    ) -> AppResult<Hold> {
        let hold = self.repository.holds_get_by_id(id).await?;
        if !can_manage_others && hold.user_id != requesting_user_id {
            return Err(AppError::Authorization(
                "Cannot cancel another user's hold".to_string(),
            ));
        }
        self.repository.holds_cancel(id).await
    }

    /// Notify the first pending hold when a loan is returned.
    #[tracing::instrument(skip(self), err)]
    pub async fn notify_next(&self, item_id: i64, expiry_days: i32) -> AppResult<Option<Hold>> {
        self.repository
            .holds_notify_next(item_id, expiry_days)
            .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn expire_overdue(&self) -> AppResult<Vec<i64>> {
        self.repository.holds_expire_overdue().await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn count_for_item(&self, item_id: i64) -> AppResult<i64> {
        self.repository.holds_count_for_item(item_id).await
    }

    /// Active holds (`pending` / `ready`) across all copies of a biblio.
    #[tracing::instrument(skip(self), err)]
    pub async fn count_active_for_biblio(&self, biblio_id: i64) -> AppResult<i64> {
        self.repository
            .holds_count_active_for_biblio(biblio_id)
            .await
    }
}

pub(crate) fn validate_max_active_holds(max: Option<i16>) -> AppResult<()> {
    let Some(max) = max else {
        return Ok(());
    };
    if !(MIN_MAX_ACTIVE_HOLDS..=MAX_MAX_ACTIVE_HOLDS).contains(&max) {
        return Err(AppError::Validation(format!(
            "maxActiveHolds must be between {MIN_MAX_ACTIVE_HOLDS} and {MAX_MAX_ACTIVE_HOLDS}"
        )));
    }
    Ok(())
}

fn hold_cap_block_message(active: i64, max: i16) -> String {
    format!(
        "Maximum of {max} active holds reached ({active} pending/ready) — cancel a hold or use force=true to override"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::hold::DEFAULT_MAX_ACTIVE_HOLDS;
    use async_trait::async_trait;
    use chrono::Utc;

    struct FakeHoldsRepo {
        duplicate: bool,
        hold_user_id: i64,
        active_count: i64,
        max_active: i16,
    }

    impl FakeHoldsRepo {
        fn ok() -> Self {
            Self {
                duplicate: false,
                hold_user_id: 1,
                active_count: 0,
                max_active: 2,
            }
        }
    }

    #[async_trait]
    impl HoldsRepository for FakeHoldsRepo {
        async fn holds_list_all(
            &self,
            _: i64,
            _: i64,
            _: bool,
        ) -> AppResult<(Vec<HoldDetails>, i64)> {
            Ok((vec![], 0))
        }
        async fn holds_list_for_user_paginated(
            &self,
            _: i64,
            _: i64,
            _: i64,
            _: bool,
        ) -> AppResult<(Vec<HoldDetails>, i64)> {
            Ok((vec![], 0))
        }
        async fn holds_has_active_for_user_item(&self, _: i64, _: i64) -> AppResult<bool> {
            Ok(self.duplicate)
        }
        async fn holds_create(&self, data: &CreateHold) -> AppResult<Hold> {
            Ok(Hold {
                id: 1,
                user_id: data.user_id,
                item_id: data.item_id,
                position: 1,
                status: crate::models::hold::HoldStatus::Pending,
                notes: data.notes.clone(),
                created_at: Utc::now(),
                notified_at: None,
                expires_at: None,
            })
        }
        async fn holds_list_for_item(&self, _: i64) -> AppResult<Vec<HoldDetails>> {
            Ok(vec![])
        }
        async fn holds_list_for_user(&self, _: i64) -> AppResult<Vec<HoldDetails>> {
            Ok(vec![])
        }
        async fn holds_get_by_id(&self, id: i64) -> AppResult<Hold> {
            Ok(Hold {
                id,
                user_id: self.hold_user_id,
                item_id: 99,
                position: 1,
                status: crate::models::hold::HoldStatus::Pending,
                notes: None,
                created_at: Utc::now(),
                notified_at: None,
                expires_at: None,
            })
        }
        async fn holds_cancel(&self, id: i64) -> AppResult<Hold> {
            self.holds_get_by_id(id).await
        }
        async fn holds_mark_ready(&self, id: i64, _: i32) -> AppResult<Hold> {
            self.holds_get_by_id(id).await
        }
        async fn holds_get_next_pending(&self, _: i64) -> AppResult<Option<Hold>> {
            Ok(None)
        }
        async fn holds_fulfill(&self, id: i64) -> AppResult<Hold> {
            self.holds_get_by_id(id).await
        }
        async fn holds_notify_next(&self, _: i64, _: i32) -> AppResult<Option<Hold>> {
            Ok(None)
        }
        async fn holds_expire_overdue(&self) -> AppResult<Vec<i64>> {
            Ok(vec![])
        }
        async fn holds_count_for_item(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn holds_count_active_for_biblio(&self, _: i64) -> AppResult<i64> {
            Ok(0)
        }
        async fn holds_count_active_for_user(&self, _: i64) -> AppResult<i64> {
            Ok(self.active_count)
        }
        async fn holds_get_max_active_for_user(&self, _: i64) -> AppResult<i16> {
            Ok(self.max_active)
        }
        async fn holds_get_global_max_active(&self) -> AppResult<i16> {
            Ok(self.max_active)
        }
        async fn holds_set_global_max_active(&self, max_active_holds: i16) -> AppResult<i16> {
            Ok(max_active_holds)
        }
    }

    fn create_hold(force: bool) -> CreateHold {
        CreateHold {
            user_id: 1,
            item_id: 42,
            notes: None,
            force,
        }
    }

    #[tokio::test]
    async fn place_hold_rejects_duplicate() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            duplicate: true,
            ..FakeHoldsRepo::ok()
        }));
        let err = svc
            .place_hold(create_hold(false), None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
    }

    #[tokio::test]
    async fn place_hold_under_limit_succeeds() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            active_count: 1,
            max_active: 2,
            ..FakeHoldsRepo::ok()
        }));
        let hold = svc
            .place_hold(create_hold(false), None, None)
            .await
            .expect("under cap");
        assert_eq!(hold.item_id, 42);
    }

    #[tokio::test]
    async fn place_hold_at_limit_is_business_rule() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            active_count: 2,
            max_active: 2,
            ..FakeHoldsRepo::ok()
        }));
        let err = svc
            .place_hold(create_hold(false), None, None)
            .await
            .unwrap_err();
        match err {
            AppError::BusinessRule(msg) => {
                assert!(msg.contains("Maximum of 2"), "{msg}");
                assert!(msg.contains("force=true"), "{msg}");
            }
            other => panic!("expected BusinessRule, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn place_hold_over_limit_is_business_rule() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            active_count: 3,
            max_active: 2,
            ..FakeHoldsRepo::ok()
        }));
        let err = svc
            .place_hold(create_hold(false), None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BusinessRule(_)));
    }

    #[tokio::test]
    async fn place_hold_at_limit_force_succeeds() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            active_count: 2,
            max_active: 2,
            ..FakeHoldsRepo::ok()
        }));
        let hold = svc
            .place_hold(create_hold(true), None, None)
            .await
            .expect("staff force");
        assert_eq!(hold.user_id, 1);
    }

    #[tokio::test]
    async fn quota_reports_remaining_slots() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            active_count: 1,
            max_active: 3,
            ..FakeHoldsRepo::ok()
        }));
        let quota = svc.quota_for_user(7).await.expect("quota");
        assert_eq!(quota.user_id, 7);
        assert_eq!(quota.max_active_holds, 3);
        assert_eq!(quota.active_holds, 1);
        assert_eq!(quota.remaining, 2);
    }

    #[tokio::test]
    async fn set_policy_rejects_out_of_range() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo::ok()));
        let err = svc.set_policy(0).await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        let err = svc.set_policy(1001).await.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
        let policy = svc.set_policy(DEFAULT_MAX_ACTIVE_HOLDS).await.expect("ok");
        assert_eq!(policy.max_active_holds, DEFAULT_MAX_ACTIVE_HOLDS);
    }

    #[tokio::test]
    async fn cancel_hold_rejects_other_user() {
        let svc = HoldsService::new(Arc::new(FakeHoldsRepo {
            hold_user_id: 2,
            ..FakeHoldsRepo::ok()
        }));
        let err = svc.cancel(1, 1, false).await.unwrap_err();
        assert!(matches!(err, AppError::Authorization(_)));
    }
}
