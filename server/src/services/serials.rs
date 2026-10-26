//! Serial frequencies, numbering patterns, and the prediction preview.
//!
//! The preview calls the pure engine and does not insert `serial_issues`.

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::serial::{
        CounterRule, FrequencySpec, FrequencyWrite, NumberingPatternSpec, NumberingPatternWrite,
        PredictionResponse, SerialFrequency, SerialFrequencyListResponse, SerialListQuery,
        SerialNumberingPattern, SerialNumberingPatternListResponse, TestPredictionRequest,
    },
    repository::SerialsRepository,
    serials::prediction::{self, PredictionInput},
};

#[derive(Clone)]
pub struct SerialsService {
    repository: Arc<dyn SerialsRepository>,
}

impl SerialsService {
    pub fn new(repository: Arc<dyn SerialsRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_frequencies(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<SerialFrequencyListResponse> {
        let (frequencies, total) = self.repository.frequencies_list(query).await?;
        Ok(SerialFrequencyListResponse { frequencies, total })
    }

    pub async fn get_frequency(&self, id: i64) -> AppResult<SerialFrequency> {
        self.repository.frequencies_get(id).await
    }

    pub async fn create_frequency(&self, data: &FrequencyWrite) -> AppResult<SerialFrequency> {
        validate_frequency_write(data)?;
        self.repository.frequencies_create(data).await
    }

    pub async fn update_frequency(
        &self,
        id: i64,
        data: &FrequencyWrite,
    ) -> AppResult<SerialFrequency> {
        validate_frequency_write(data)?;
        self.repository.frequencies_update(id, data).await
    }

    pub async fn delete_frequency(&self, id: i64) -> AppResult<SerialFrequency> {
        self.repository.frequencies_delete(id).await
    }

    pub async fn list_patterns(
        &self,
        query: &SerialListQuery,
    ) -> AppResult<SerialNumberingPatternListResponse> {
        let (patterns, total) = self.repository.patterns_list(query).await?;
        Ok(SerialNumberingPatternListResponse { patterns, total })
    }

    pub async fn get_pattern(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        self.repository.patterns_get(id).await
    }

    pub async fn create_pattern(
        &self,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        validate_pattern_write(data)?;
        self.repository.patterns_create(data).await
    }

    pub async fn update_pattern(
        &self,
        id: i64,
        data: &NumberingPatternWrite,
    ) -> AppResult<SerialNumberingPattern> {
        validate_pattern_write(data)?;
        self.repository.patterns_update(id, data).await
    }

    pub async fn delete_pattern(&self, id: i64) -> AppResult<SerialNumberingPattern> {
        self.repository.patterns_delete(id).await
    }

    /// Preview the expected issue list. Nothing is stored.
    pub async fn test_prediction(
        &self,
        request: &TestPredictionRequest,
    ) -> AppResult<PredictionResponse> {
        let frequency = self.resolve_frequency(request).await?;
        let pattern = self.resolve_pattern(request).await?;
        let output = prediction::predict(&PredictionInput {
            first_issue_date: request.first_issue_date,
            frequency,
            pattern,
            starting: request.starting,
            inner: request.inner,
            start_slot: request.start_slot,
            length: request.length,
            irregularities: request.irregularities.clone(),
            recurring_skips: request.recurring_skips.clone(),
            combined_issues: request.combined_issues.clone(),
        })
        .map_err(prediction_error)?;
        Ok(PredictionResponse {
            issues: output.issues,
            next_state: output.next_state,
        })
    }

    async fn resolve_frequency(&self, request: &TestPredictionRequest) -> AppResult<FrequencySpec> {
        match (request.frequency_id, request.frequency) {
            (Some(id), None) => Ok(self.repository.frequencies_get(id).await?.spec()),
            (None, Some(spec)) => Ok(spec),
            _ => Err(AppError::Validation(
                "Provide exactly one of frequencyId or frequency".into(),
            )),
        }
    }

    async fn resolve_pattern(
        &self,
        request: &TestPredictionRequest,
    ) -> AppResult<NumberingPatternSpec> {
        match (request.pattern_id, request.pattern.clone()) {
            (Some(id), None) => Ok(self.repository.patterns_get(id).await?.spec()),
            (None, Some(spec)) => Ok(spec),
            _ => Err(AppError::Validation(
                "Provide exactly one of patternId or pattern".into(),
            )),
        }
    }
}

fn validate_frequency_write(data: &FrequencyWrite) -> AppResult<()> {
    if data.description.trim().is_empty() {
        return Err(AppError::Validation(
            "Frequency description must not be empty".into(),
        ));
    }
    prediction::validate_frequency(&data.spec()).map_err(prediction_error)
}

fn validate_pattern_write(data: &NumberingPatternWrite) -> AppResult<()> {
    if data.label.trim().is_empty() {
        return Err(AppError::Validation(
            "Numbering pattern label must not be empty".into(),
        ));
    }
    validate_counter_name(&data.x)?;
    validate_counter_name(&data.y)?;
    validate_counter_name(&data.z)?;
    prediction::validate_pattern(&data.spec()).map_err(prediction_error)
}

fn validate_counter_name(rule: &CounterRule) -> AppResult<()> {
    if rule
        .name
        .as_ref()
        .is_some_and(|name| name.trim().len() > 50)
    {
        return Err(AppError::Validation(
            "Counter name must be at most 50 characters".into(),
        ));
    }
    Ok(())
}

fn prediction_error(err: prediction::PredictionError) -> AppError {
    if err.is_business_rule() {
        AppError::BusinessRule(err.message())
    } else {
        AppError::Validation(err.message())
    }
}
