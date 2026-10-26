//! Serial frequencies, numbering patterns, the prediction preview, and subscriptions.
//!
//! The preview calls the pure engine and does not insert `serial_issues`.
//! Creating a serial subscription, or generating the next horizon, does.

use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    models::serial::{
        CounterInners, CounterRule, CounterValues, FrequencySpec, FrequencyWrite, NewSerialIssue,
        NumberingPatternSpec, NumberingPatternWrite, PredictionResponse, PredictionState,
        SerialFrequency, SerialFrequencyListResponse, SerialIssue, SerialIssueGenerateResponse,
        SerialListQuery, SerialNumberingPattern, SerialNumberingPatternListResponse,
        SerialSubscription, SerialSubscriptionListResponse, SerialSubscriptionQuery,
        SerialSubscriptionStatus, SerialSubscriptionWrite, TestPredictionRequest,
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

    pub async fn list_subscriptions(
        &self,
        query: &SerialSubscriptionQuery,
    ) -> AppResult<SerialSubscriptionListResponse> {
        let (subscriptions, total) = self.repository.subscriptions_list(query).await?;
        Ok(SerialSubscriptionListResponse {
            subscriptions,
            total,
        })
    }

    pub async fn get_subscription(&self, id: i64) -> AppResult<SerialSubscription> {
        self.subscription_detail(id).await
    }

    pub async fn list_subscription_issues(&self, id: i64) -> AppResult<Vec<SerialIssue>> {
        self.repository.subscriptions_get(id).await?;
        self.repository.subscriptions_issues(id).await
    }

    pub async fn create_subscription(
        &self,
        data: &SerialSubscriptionWrite,
    ) -> AppResult<SerialSubscription> {
        validate_subscription_write(data)?;
        self.repository.periodical_biblio(data.biblio_id).await?;
        let (frequency, pattern) = self.subscription_specs(data).await?;
        let input = input_from_write(data, frequency, pattern, 1);
        let (issues, cursor) = predict_stored_horizon(&input, 0, data.end_date)?;
        let id = self
            .repository
            .subscriptions_insert(data, &cursor, &issues)
            .await?;
        self.subscription_detail(id).await
    }

    pub async fn update_subscription(
        &self,
        id: i64,
        data: &SerialSubscriptionWrite,
    ) -> AppResult<SerialSubscription> {
        validate_subscription_write(data)?;
        let existing = self.repository.subscriptions_get(id).await?;
        if existing.biblio_id != data.biblio_id {
            return Err(AppError::Validation(
                "The periodical record of a serial subscription cannot change".into(),
            ));
        }
        self.repository.periodical_biblio(data.biblio_id).await?;
        let (frequency, pattern) = self.subscription_specs(data).await?;
        let (cursor, new_issues) = if prediction_changed(&existing, data) {
            if existing.issue_count > 0 {
                return Err(AppError::Conflict(
                    "Prediction inputs cannot change after issues exist. Generate the next horizon instead.".into(),
                ));
            }
            let input = input_from_write(data, frequency, pattern, 1);
            let (issues, cursor) = predict_stored_horizon(&input, 0, data.end_date)?;
            (cursor, issues)
        } else {
            (cursor_of(&existing), Vec::new())
        };
        self.repository
            .subscriptions_update(id, data, &cursor, &new_issues)
            .await?;
        self.subscription_detail(id).await
    }

    pub async fn delete_subscription(&self, id: i64) -> AppResult<SerialSubscription> {
        let existing = self.subscription_detail(id).await?;
        self.repository.subscriptions_delete(id).await?;
        Ok(existing)
    }

    /// Append the next 12-month horizon from the stored cursor. Existing rows stay.
    ///
    /// `next_slot` is that cursor. Supplement rows are not publication slots, so
    /// they are excluded from the count of issues already emitted and do not
    /// move the cursor.
    pub async fn generate_subscription(&self, id: i64) -> AppResult<SerialIssueGenerateResponse> {
        let existing = self.repository.subscriptions_get(id).await?;
        if existing.status != SerialSubscriptionStatus::Active {
            return Err(AppError::BusinessRule(
                "Only an active serial subscription can generate issues".into(),
            ));
        }
        let frequency = self
            .repository
            .frequencies_get(existing.frequency_id)
            .await?
            .spec();
        let pattern = self
            .repository
            .patterns_get(existing.numbering_pattern_id)
            .await?
            .spec();
        let input = input_from_subscription(&existing, frequency, pattern);
        let emitted = emitted_count(
            self.repository
                .subscriptions_regular_issue_count(id)
                .await?,
        )?;
        let (new_issues, cursor) = predict_stored_horizon(&input, emitted, existing.end_date)?;
        let expected_slot = i32::try_from(existing.next_slot)
            .map_err(|_| AppError::Internal("Stored next slot is out of range".to_string()))?;
        self.repository
            .subscriptions_append(id, expected_slot, &cursor, &new_issues)
            .await?;
        let subscription = self.subscription_detail(id).await?;
        let issues = subscription
            .issues
            .iter()
            .filter(|issue| !issue.supplement && u64::from(issue.sequence) > u64::from(emitted))
            .cloned()
            .collect();
        Ok(SerialIssueGenerateResponse {
            subscription,
            issues,
        })
    }

    async fn subscription_detail(&self, id: i64) -> AppResult<SerialSubscription> {
        let mut subscription = self.repository.subscriptions_get(id).await?;
        subscription.issues = self.repository.subscriptions_issues(id).await?;
        Ok(subscription)
    }

    async fn subscription_specs(
        &self,
        data: &SerialSubscriptionWrite,
    ) -> AppResult<(FrequencySpec, NumberingPatternSpec)> {
        let frequency = self
            .repository
            .frequencies_get(data.frequency_id)
            .await?
            .spec();
        let pattern = self
            .repository
            .patterns_get(data.numbering_pattern_id)
            .await?
            .spec();
        Ok((frequency, pattern))
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

fn validate_subscription_write(data: &SerialSubscriptionWrite) -> AppResult<()> {
    if data.length.count == 0 || data.length.count > prediction::MAX_SUBSCRIPTION_LENGTH {
        return Err(AppError::Validation(format!(
            "Length count must be from 1 to {} issues, weeks, or months",
            prediction::MAX_SUBSCRIPTION_LENGTH
        )));
    }
    if data.cost_cents.is_some_and(|cents| cents < 0) {
        return Err(AppError::Validation(
            "Annual cost must not be negative".into(),
        ));
    }
    if data.issue_price_cents.is_some_and(|cents| cents < 0) {
        return Err(AppError::Validation(
            "Issue price must not be negative".into(),
        ));
    }
    if data.grace_days.is_some_and(|days| days < 0) {
        return Err(AppError::Validation(
            "Grace days must not be negative".into(),
        ));
    }
    if let (Some(start), Some(end)) = (data.start_date, data.end_date) {
        if end < start {
            return Err(AppError::Validation(
                "End date must not be before the start date".into(),
            ));
        }
    }
    if data.irregularities.contains(&0) {
        return Err(AppError::Validation(
            "Irregularity slots are 1-based".into(),
        ));
    }
    Ok(())
}

fn predict_stored_horizon(
    input: &PredictionInput,
    already_emitted: u32,
    end_date: Option<chrono::NaiveDate>,
) -> AppResult<(Vec<NewSerialIssue>, PredictionState)> {
    let horizon =
        prediction::horizon_length(input, already_emitted, end_date).map_err(prediction_error)?;
    let mut trial = input.clone();
    trial.length = horizon;
    let output = prediction::predict(&trial).map_err(prediction_error)?;
    let issues = to_new_issues(&output.issues, already_emitted)?;
    Ok((issues, output.next_state))
}

fn to_new_issues(
    issues: &[crate::models::serial::PredictedIssue],
    offset: u32,
) -> AppResult<Vec<NewSerialIssue>> {
    issues
        .iter()
        .map(|issue| {
            let sequence = offset
                .checked_add(issue.sequence)
                .ok_or_else(|| AppError::Internal("Issue sequence overflow".into()))?;
            Ok(NewSerialIssue {
                planned_date: issue.planned_date,
                display_label: issue.display_label.clone(),
                sequence: i32::try_from(sequence)
                    .map_err(|_| AppError::Internal("Issue sequence is out of range".into()))?,
                slot: i32::try_from(issue.slot)
                    .map_err(|_| AppError::Internal("Issue slot is out of range".into()))?,
                x: issue.x,
                y: issue.y,
                z: issue.z,
            })
        })
        .collect()
}

fn emitted_count(count: i64) -> AppResult<u32> {
    u32::try_from(count).map_err(|_| AppError::Internal("Issue count is out of range".into()))
}

fn cursor_of(subscription: &SerialSubscription) -> PredictionState {
    PredictionState {
        x: subscription.x,
        y: subscription.y,
        z: subscription.z,
        inner_x: subscription.inner_x,
        inner_y: subscription.inner_y,
        inner_z: subscription.inner_z,
        next_slot: subscription.next_slot,
    }
}

fn input_from_write(
    data: &SerialSubscriptionWrite,
    frequency: FrequencySpec,
    pattern: NumberingPatternSpec,
    start_slot: u32,
) -> PredictionInput {
    PredictionInput {
        first_issue_date: data.first_issue_date,
        frequency,
        pattern,
        starting: CounterValues {
            x: data.x,
            y: data.y,
            z: data.z,
        },
        inner: CounterInners {
            x: data.inner_x,
            y: data.inner_y,
            z: data.inner_z,
        },
        start_slot,
        length: data.length,
        irregularities: data.irregularities.clone(),
        recurring_skips: data.recurring_skips.clone(),
        combined_issues: data.combined_issues.clone(),
    }
}

fn input_from_subscription(
    subscription: &SerialSubscription,
    frequency: FrequencySpec,
    pattern: NumberingPatternSpec,
) -> PredictionInput {
    PredictionInput {
        first_issue_date: subscription.first_issue_date,
        frequency,
        pattern,
        starting: CounterValues {
            x: subscription.x,
            y: subscription.y,
            z: subscription.z,
        },
        inner: CounterInners {
            x: subscription.inner_x,
            y: subscription.inner_y,
            z: subscription.inner_z,
        },
        start_slot: subscription.next_slot,
        length: subscription.length,
        irregularities: subscription.irregularities.clone(),
        recurring_skips: subscription.recurring_skips.clone(),
        combined_issues: subscription.combined_issues.clone(),
    }
}

fn prediction_changed(existing: &SerialSubscription, data: &SerialSubscriptionWrite) -> bool {
    existing.frequency_id != data.frequency_id
        || existing.numbering_pattern_id != data.numbering_pattern_id
        || existing.first_issue_date != data.first_issue_date
        || existing.end_date != data.end_date
        || existing.x != data.x
        || existing.y != data.y
        || existing.z != data.z
        || existing.inner_x != data.inner_x
        || existing.inner_y != data.inner_y
        || existing.inner_z != data.inner_z
        || existing.length != data.length
        || slot_set(&existing.irregularities) != slot_set(&data.irregularities)
        || existing.recurring_skips != data.recurring_skips
        || existing.combined_issues != data.combined_issues
}

fn slot_set(slots: &[u32]) -> Vec<u32> {
    let mut copy = slots.to_vec();
    copy.sort_unstable();
    copy.dedup();
    copy
}
