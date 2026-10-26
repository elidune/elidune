//! Serial frequencies, numbering patterns, the prediction preview, and subscriptions.
//!
//! The preview calls the pure engine and does not insert `serial_issues`.
//! Creating a serial subscription, or generating the next horizon, does.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};

use crate::{
    error::{AppError, AppResult},
    models::{
        fine::Cents,
        item::{Item, ReceivedCopy},
        serial::{
            CounterInners, CounterRule, CounterValues, CreateSerialSupplement, FrequencySpec,
            FrequencyWrite, NewSerialIssue, NumberingPatternSpec, NumberingPatternWrite,
            PredictionResponse, PredictionState, ReceiveSerialIssue, SerialFrequency,
            SerialFrequencyListResponse, SerialIssue, SerialIssueGenerateResponse,
            SerialIssueStatus, SerialIssuesMarkedLate, SerialListQuery, SerialNumberingPattern,
            SerialNumberingPatternListResponse, SerialSubscription, SerialSubscriptionListResponse,
            SerialSubscriptionQuery, SerialSubscriptionStatus, SerialSubscriptionWrite,
            SetSerialIssueStatus, TestPredictionRequest,
        },
    },
    repository::SerialsRepository,
    serials::prediction::{self, PredictionInput},
    services::catalog::CatalogService,
};

#[derive(Clone)]
pub struct SerialsService {
    repository: Arc<dyn SerialsRepository>,
    catalog: CatalogService,
}

impl SerialsService {
    pub fn new(repository: Arc<dyn SerialsRepository>, catalog: CatalogService) -> Self {
        Self {
            repository,
            catalog,
        }
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
    /// `next_slot` is that cursor. The sequence offset counts rows with
    /// `supplement = false` (regular issues, which have a slot and a sequence).
    /// A supplement may omit both and does not move the cursor or the next sequence.
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
            .filter(|issue| {
                !issue.supplement
                    && issue
                        .sequence
                        .is_some_and(|sequence| u64::from(sequence) > u64::from(emitted))
            })
            .cloned()
            .collect();
        Ok(SerialIssueGenerateResponse {
            subscription,
            issues,
        })
    }

    /// Receive an expected or late issue. Other issues keep their status.
    ///
    /// When `create_items` is set, the copy is built with [`Item::received_copy`] and
    /// inserted by [`CatalogService::create_item`]. `issue_price_cents` is the item
    /// price. `cost_cents` is not copied. A missing barcode uses
    /// [`crate::services::item_barcode::allocate_local_item_barcode`].
    pub async fn receive_issue(
        &self,
        subscription_id: i64,
        issue_id: i64,
        data: &ReceiveSerialIssue,
    ) -> AppResult<SerialIssue> {
        let subscription = self.repository.subscriptions_get(subscription_id).await?;
        let issue = self
            .repository
            .subscription_issue(subscription_id, issue_id)
            .await?;
        if !matches!(
            issue.status,
            SerialIssueStatus::Expected | SerialIssueStatus::Late
        ) {
            return Err(AppError::BusinessRule(
                "Only an expected or late issue can be received".into(),
            ));
        }
        let barcode = trimmed(&data.barcode);
        if !subscription.create_items && barcode.is_some() {
            return Err(AppError::BusinessRule(
                "This serial subscription does not create copies".into(),
            ));
        }
        let item_id = if subscription.create_items {
            let barcode = match barcode {
                Some(barcode) => barcode,
                None => self.repository.allocate_item_barcode().await?,
            };
            let existing = self
                .repository
                .subscriptions_issues(subscription_id)
                .await?;
            let in_library_only = subscription.latest_issue_library_use_only
                && becomes_latest_regular(&issue, &existing);
            let created = self
                .catalog
                .create_item(
                    subscription.biblio_id,
                    Item::received_copy(ReceivedCopy {
                        biblio_id: subscription.biblio_id,
                        barcode: Some(barcode),
                        site_id: Some(subscription.site_id),
                        site_name: None,
                        call_number: trimmed(&subscription.call_number),
                        volume_designation: Some(issue.display_label.clone()),
                        public_note: trimmed(&subscription.public_note),
                        internal_note: item_internal_note(&subscription),
                        price: subscription
                            .issue_price_cents
                            .map(|cents| Cents(i64::from(cents))),
                        price_deferred: false,
                        borrowable: in_library_only.then_some(false),
                    }),
                )
                .await?;
            Some(
                created
                    .id
                    .ok_or_else(|| AppError::Internal("Created item is missing id".into()))?,
            )
        } else {
            None
        };
        let received_at = received_at(data.received_on);
        let arrived = self
            .repository
            .issues_mark_arrived(subscription_id, issue_id, received_at, item_id)
            .await?
            .ok_or_else(|| {
                AppError::Conflict(
                    "This issue was updated and can no longer be received".to_string(),
                )
            })?;
        if subscription.create_items && subscription.latest_issue_library_use_only {
            self.repository
                .apply_latest_issue_borrowable(subscription_id)
                .await?;
        }
        Ok(arrived)
    }

    /// Move an unreceived issue to late, missing (claim-ready), claimed, not published, or deleted.
    pub async fn set_issue_status(
        &self,
        subscription_id: i64,
        issue_id: i64,
        data: &SetSerialIssueStatus,
    ) -> AppResult<SerialIssue> {
        let issue = self
            .repository
            .subscription_issue(subscription_id, issue_id)
            .await?;
        let target = data.status.as_issue_status();
        if issue.status == target {
            return Ok(issue);
        }
        if !data.status.allowed_from().contains(&issue.status) {
            return Err(AppError::BusinessRule(format!(
                "An issue with status {} cannot become {}",
                issue.status.as_db_str(),
                target.as_db_str()
            )));
        }
        self.repository
            .issues_set_status(
                subscription_id,
                issue_id,
                target,
                data.status.allowed_from(),
            )
            .await?
            .ok_or_else(|| {
                AppError::Conflict("This issue was updated and the status was not changed".into())
            })
    }

    /// Insert a supplement with a null slot and a null sequence.
    pub async fn create_supplement(
        &self,
        subscription_id: i64,
        data: &CreateSerialSupplement,
    ) -> AppResult<SerialIssue> {
        self.repository.subscriptions_get(subscription_id).await?;
        let label = data.display_label.trim();
        if label.is_empty() {
            return Err(AppError::Validation(
                "Supplement display label must not be empty".into(),
            ));
        }
        self.repository
            .issues_insert_supplement(
                subscription_id,
                data.planned_date,
                label,
                trimmed(&data.notes),
            )
            .await
    }

    /// Mark expected issues late when today is after `planned_date` plus the effective grace.
    ///
    /// A stored `grace_days`, including 0, wins. Otherwise the frequency default applies.
    /// An irregular subscription with no grace is left unchanged. Receiving another issue
    /// does not call this.
    pub async fn mark_late_issues(
        &self,
        subscription_id: i64,
    ) -> AppResult<SerialIssuesMarkedLate> {
        let subscription = self.repository.subscriptions_get(subscription_id).await?;
        let Some(grace_days) = subscription.effective_grace_days else {
            return Ok(SerialIssuesMarkedLate { issues: Vec::new() });
        };
        let issues = self
            .repository
            .issues_mark_late(subscription_id, grace_days, Utc::now().date_naive())
            .await?;
        Ok(SerialIssuesMarkedLate { issues })
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

fn trimmed(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn item_internal_note(subscription: &SerialSubscription) -> Option<String> {
    trimmed(&subscription.internal_note).or_else(|| trimmed(&subscription.notes))
}

fn received_at(received_on: Option<NaiveDate>) -> DateTime<Utc> {
    match received_on {
        Some(date) => date
            .and_hms_opt(12, 0, 0)
            .expect("noon is a valid time")
            .and_utc(),
        None => Utc::now(),
    }
}

/// True when this regular issue's planned date is at least as recent as every arrived regular issue.
fn becomes_latest_regular(issue: &SerialIssue, existing: &[SerialIssue]) -> bool {
    if issue.supplement {
        return false;
    }
    !existing.iter().any(|other| {
        other.id != issue.id
            && other.status == SerialIssueStatus::Arrived
            && !other.supplement
            && planned_is_later(other, issue)
    })
}

fn planned_is_later(other: &SerialIssue, issue: &SerialIssue) -> bool {
    other.planned_date > issue.planned_date
        || (other.planned_date == issue.planned_date
            && other.slot.unwrap_or(0) > issue.slot.unwrap_or(0))
}
