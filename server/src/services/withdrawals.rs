//! Weeding withdrawals: one event per copy, batch, cancel, disposal, legacy completion, list, CSV.

use std::collections::HashSet;

use chrono::{DateTime, Utc};

use crate::{
    error::{error_code, AppError, AppResult},
    models::{
        withdrawal::{
            normalize_reason_detail, parse_export_language, withdrawals_csv, BatchWithdrawalResponse, ItemWithdrawal, WithdrawalInputError, WithdrawalList, WithdrawalQuery, WithdrawalWrite,
        },
        Language,
    },
};

use super::catalog::CatalogService;

const BATCH_LIMIT: usize = 100;

impl CatalogService {
    pub async fn withdraw_item(&self, item_id: i64, agent_id: i64, input: &WithdrawalWrite) -> AppResult<ItemWithdrawal> {
        let input = prepare_write(input)?;
        let withdrawal = self.repository.item_withdrawals_create(item_id, agent_id, &input).await?;
        if let Some(biblio_id) = withdrawal.biblio_id {
            self.sync_index(biblio_id).await;
        }
        Ok(withdrawal)
    }

    pub async fn withdraw_items(&self, item_ids: &[i64], agent_id: i64, input: &WithdrawalWrite) -> AppResult<BatchWithdrawalResponse> {
        if item_ids.is_empty() {
            return Err(AppError::Validation("itemIds must contain at least one copy".into()));
        }
        if item_ids.len() > BATCH_LIMIT {
            return Err(AppError::Validation(format!("A batch withdraws at most {BATCH_LIMIT} copies")));
        }
        let mut seen = HashSet::new();
        if item_ids.iter().any(|id| !seen.insert(*id)) {
            return Err(AppError::Validation("itemIds contains a duplicate copy".into()));
        }
        let input = prepare_write(input)?;
        let withdrawals = self.repository.item_withdrawals_create_many(item_ids, agent_id, &input).await?;
        let mut biblios = HashSet::new();
        for withdrawal in &withdrawals {
            if let Some(biblio_id) = withdrawal.biblio_id {
                if biblios.insert(biblio_id) {
                    self.sync_index(biblio_id).await;
                }
            }
        }
        Ok(BatchWithdrawalResponse { withdrawals })
    }

    pub async fn cancel_withdrawal(&self, id: i64, agent_id: i64) -> AppResult<ItemWithdrawal> {
        let withdrawal = self.repository.item_withdrawals_cancel(id, agent_id).await?;
        if let Some(biblio_id) = withdrawal.biblio_id {
            self.sync_index(biblio_id).await;
        }
        Ok(withdrawal)
    }

    pub async fn dispose_withdrawals(&self, ids: &[i64], agent_id: i64, disposed_at: Option<&str>) -> AppResult<BatchWithdrawalResponse> {
        if ids.is_empty() {
            return Err(AppError::Validation("withdrawalIds must contain at least one withdrawal".into()));
        }
        if ids.len() > BATCH_LIMIT {
            return Err(AppError::Validation(format!("A batch disposes at most {BATCH_LIMIT} withdrawals")));
        }
        let mut seen = HashSet::new();
        if ids.iter().any(|id| !seen.insert(*id)) {
            return Err(AppError::Validation("withdrawalIds contains a duplicate withdrawal".into()));
        }
        let when = match disposed_at.map(str::trim).filter(|value| !value.is_empty()) {
            None => Utc::now(),
            Some(raw) => DateTime::parse_from_rfc3339(raw)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| AppError::Validation("disposedAt must be an RFC3339 timestamp".into()))?,
        };
        let withdrawals = self.repository.item_withdrawals_dispose(ids, agent_id, when).await?;
        Ok(BatchWithdrawalResponse { withdrawals })
    }

    pub async fn complete_legacy_withdrawal(&self, id: i64, input: &WithdrawalWrite) -> AppResult<ItemWithdrawal> {
        let input = prepare_write(input)?;
        self.repository.item_withdrawals_complete(id, &input).await
    }

    pub async fn list_withdrawals(&self, query: &WithdrawalQuery) -> AppResult<WithdrawalList> {
        self.repository.item_withdrawals_list(query).await
    }

    pub async fn get_withdrawal(&self, id: i64) -> AppResult<ItemWithdrawal> {
        self.repository.item_withdrawals_get(id).await
    }

    pub async fn get_active_item_withdrawal(&self, item_id: i64) -> AppResult<ItemWithdrawal> {
        self.repository.item_withdrawals_active_for_item(item_id).await
    }

    pub async fn export_withdrawals_csv(&self, query: &WithdrawalQuery, user_id: i64) -> AppResult<String> {
        let language = self.export_language(query, user_id).await?;
        let zone = self.repository.library_zone().await?;
        let rows = self.repository.item_withdrawals_export_rows(query).await?;
        Ok(withdrawals_csv(&rows, language, zone))
    }

    async fn export_language(&self, query: &WithdrawalQuery, user_id: i64) -> AppResult<Language> {
        if let Some(raw) = query.lang.as_deref() {
            return parse_export_language(raw).ok_or_else(|| AppError::Validation("lang must be french or english".into()));
        }
        let stored = self.repository.user_preferred_language(user_id).await?;
        Ok(stored
            .as_deref()
            .map(Language::from)
            .filter(|language| !matches!(language, Language::Unknown))
            .unwrap_or(Language::French))
    }
}

fn prepare_write(input: &WithdrawalWrite) -> AppResult<WithdrawalWrite> {
    if input.destination.is_none() {
        return Err(AppError::coded(400, error_code::WITHDRAWAL_DESTINATION_REQUIRED, "destination is required"));
    }
    let reason_detail = match normalize_reason_detail(input.reason, input.reason_detail.as_deref()) {
        Ok(detail) => detail,
        Err(WithdrawalInputError::DetailRequired) => {
            return Err(AppError::coded(
                400,
                error_code::WITHDRAWAL_REASON_DETAIL_REQUIRED,
                "reasonDetail is required and must be at least 3 characters when reason is other",
            ));
        }
        Err(WithdrawalInputError::DetailTooLong) => {
            return Err(AppError::Validation("reasonDetail must be at most 2000 characters".into()));
        }
        Err(WithdrawalInputError::DestinationRequired) => {
            return Err(AppError::coded(400, error_code::WITHDRAWAL_DESTINATION_REQUIRED, "destination is required"));
        }
    };
    Ok(WithdrawalWrite {
        reason: input.reason,
        reason_detail,
        destination: input.destination,
    })
}
