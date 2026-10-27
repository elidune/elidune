//! Serials reference data: frequencies, numbering patterns, and prediction I/O.
//!
//! "Serial subscription" is the periodical subscription. Patron membership stays
//! the existing `subscription` vocabulary and is not modeled here.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

/// How often a serial is published. `None` is the irregular frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FrequencyUnit {
    Day,
    Week,
    Month,
    Year,
}

impl FrequencyUnit {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

/// How a counter is rendered inside `{X}`, `{Y}`, or `{Z}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum CounterFormat {
    #[default]
    Number,
    DayName,
    MonthName,
    Season,
}

impl CounterFormat {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::DayName => "day_name",
            Self::MonthName => "month_name",
            Self::Season => "season",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "day_name" => Self::DayName,
            "month_name" => Self::MonthName,
            "season" => Self::Season,
            _ => Self::Number,
        }
    }
}

/// Language of `{MonthName}` and `{DayName}` in a display label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SerialLocale {
    #[default]
    Fr,
    En,
}

impl SerialLocale {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Fr => "fr",
            Self::En => "en",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "fr" => Some(Self::Fr),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

/// One Koha-style counter (X, Y, or Z).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CounterRule {
    /// Staff caption ("Volume", "Number"). Not interpolated into the formula.
    #[serde(default)]
    pub name: Option<String>,
    /// Added to the value each time the inner loop reaches `every`.
    #[serde(default)]
    pub add: i32,
    /// Advance the value every N emitted issues. At least 1.
    #[serde(default = "default_every")]
    pub every: u32,
    /// When the value is strictly greater than this, it becomes `set_back_to`.
    #[serde(default)]
    pub when_more_than: Option<i32>,
    #[serde(default)]
    pub set_back_to: Option<i32>,
    #[serde(default)]
    pub format: CounterFormat,
    /// Zero-pad width for `format = number`. Ignored for name formats.
    #[serde(default)]
    pub zero_pad: Option<u8>,
}

fn default_every() -> u32 {
    1
}

impl Default for CounterRule {
    fn default() -> Self {
        Self {
            name: None,
            add: 0,
            every: default_every(),
            when_more_than: None,
            set_back_to: None,
            format: CounterFormat::Number,
            zero_pad: None,
        }
    }
}

/// Cycle length used by the prediction engine. No database id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencySpec {
    /// Absent or null is the irregular frequency (no predicted dates).
    #[serde(default)]
    pub unit: Option<FrequencyUnit>,
    pub issues_per_unit: i32,
    pub units_per_issue: i32,
}

/// Formula and counters used by the prediction engine. No database id.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NumberingPatternSpec {
    pub formula: String,
    #[serde(default)]
    pub locale: SerialLocale,
    #[serde(default)]
    pub x: CounterRule,
    #[serde(default)]
    pub y: CounterRule,
    #[serde(default)]
    pub z: CounterRule,
}

/// X/Y/Z values shown on an issue, before the next advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CounterValues {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Issues already counted toward the next `every` advance. Zero at the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CounterInners {
    #[serde(default)]
    pub x: u32,
    #[serde(default)]
    pub y: u32,
    #[serde(default)]
    pub z: u32,
}

/// A publication slot skipped every year, by month or by month and day.
///
/// `day` absent skips every planned date in that month. A set day skips only
/// that calendar day, so a weekly that does not land on it is unaffected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecurringSkip {
    pub month: u8,
    #[serde(default)]
    pub day: Option<u8>,
}

/// One published issue covers several months. The first month is issued.
/// Later months are skipped every year, and `{MonthName}` on the published
/// issue is `month_name` (for example `juillet-août`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CombinedIssue {
    pub months: Vec<u8>,
    pub month_name: String,
}

/// How long to predict. Date kinds stop before `first + count` (exclusive end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum LengthKind {
    Issues,
    Weeks,
    Months,
}

impl LengthKind {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Issues => "issues",
            Self::Weeks => "weeks",
            Self::Months => "months",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "issues" => Some(Self::Issues),
            "weeks" => Some(Self::Weeks),
            "months" => Some(Self::Months),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionLength {
    pub kind: LengthKind,
    pub count: u32,
}

/// Stored frequency row.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialFrequency {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub description: String,
    pub unit: Option<FrequencyUnit>,
    pub issues_per_unit: i32,
    pub units_per_issue: i32,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SerialFrequency {
    #[must_use]
    pub fn spec(&self) -> FrequencySpec {
        FrequencySpec {
            unit: self.unit,
            issues_per_unit: self.issues_per_unit,
            units_per_issue: self.units_per_issue,
        }
    }
}

/// Create or replace a frequency.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyWrite {
    #[validate(length(min = 1, max = 100))]
    pub description: String,
    pub unit: Option<FrequencyUnit>,
    pub issues_per_unit: i32,
    pub units_per_issue: i32,
    #[serde(default)]
    pub display_order: i32,
}

impl FrequencyWrite {
    #[must_use]
    pub fn spec(&self) -> FrequencySpec {
        FrequencySpec {
            unit: self.unit,
            issues_per_unit: self.issues_per_unit,
            units_per_issue: self.units_per_issue,
        }
    }
}

/// Stored numbering pattern.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialNumberingPattern {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub label: String,
    pub description: Option<String>,
    pub display_order: i32,
    pub formula: String,
    pub locale: SerialLocale,
    pub x: CounterRule,
    pub y: CounterRule,
    pub z: CounterRule,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SerialNumberingPattern {
    #[must_use]
    pub fn spec(&self) -> NumberingPatternSpec {
        NumberingPatternSpec {
            formula: self.formula.clone(),
            locale: self.locale,
            x: self.x.clone(),
            y: self.y.clone(),
            z: self.z.clone(),
        }
    }
}

/// Create or replace a numbering pattern.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NumberingPatternWrite {
    #[validate(length(min = 1, max = 100))]
    pub label: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[serde(default)]
    pub display_order: i32,
    #[validate(length(min = 1, max = 500))]
    pub formula: String,
    #[serde(default)]
    pub locale: SerialLocale,
    #[serde(default)]
    pub x: CounterRule,
    #[serde(default)]
    pub y: CounterRule,
    #[serde(default)]
    pub z: CounterRule,
}

impl NumberingPatternWrite {
    #[must_use]
    pub fn spec(&self) -> NumberingPatternSpec {
        NumberingPatternSpec {
            formula: self.formula.clone(),
            locale: self.locale,
            x: self.x.clone(),
            y: self.y.clone(),
            z: self.z.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SerialListQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialFrequencyListResponse {
    pub frequencies: Vec<SerialFrequency>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialNumberingPatternListResponse {
    pub patterns: Vec<SerialNumberingPattern>,
    pub total: i64,
}

/// Preview body. Provide exactly one of `frequencyId` / `frequency`, and one of
/// `patternId` / `pattern`.
#[serde_as]
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TestPredictionRequest {
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub frequency_id: Option<i64>,
    pub frequency: Option<FrequencySpec>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub pattern_id: Option<i64>,
    pub pattern: Option<NumberingPatternSpec>,
    pub first_issue_date: NaiveDate,
    pub starting: CounterValues,
    #[serde(default)]
    pub inner: CounterInners,
    /// Absolute 1-based slot to emit first. The anchor date stays `first_issue_date`.
    #[serde(default = "default_start_slot")]
    pub start_slot: u32,
    pub length: SubscriptionLength,
    /// Absolute 1-based publication slots to skip. The date moves on; numbering does not.
    #[serde(default)]
    pub irregularities: Vec<u32>,
    /// Yearly skips by month or month/day. Applied on top of `irregularities`.
    #[serde(default)]
    pub recurring_skips: Vec<RecurringSkip>,
    /// Double issues such as July–August. The later months are yearly skips.
    #[serde(default)]
    pub combined_issues: Vec<CombinedIssue>,
}

fn default_start_slot() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictedIssue {
    /// 1-based index among emitted issues.
    pub sequence: u32,
    /// 1-based publication slot, including skipped irregularities.
    pub slot: u32,
    pub planned_date: NaiveDate,
    /// Numbering and date, as the desk and the OPAC will show it.
    pub display_label: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Cursor to store on the serial subscription after this horizon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictionState {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub inner_x: u32,
    pub inner_y: u32,
    pub inner_z: u32,
    /// Absolute 1-based slot the next horizon must pass as `startSlot`.
    pub next_slot: u32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictionResponse {
    pub issues: Vec<PredictedIssue>,
    pub next_state: PredictionState,
}

/// Open, expired, or closed serial subscription. `active` is the open status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum SerialSubscriptionStatus {
    #[default]
    Active,
    Expired,
    Closed,
}

impl SerialSubscriptionStatus {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Closed => "closed",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "expired" => Some(Self::Expired),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

/// Stored issue status. Prediction writes `expected`. Receipt fills the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SerialIssueStatus {
    Expected,
    Arrived,
    Late,
    Missing,
    Claimed,
    NotAvailable,
    Deleted,
    Bound,
}

impl SerialIssueStatus {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Expected => "expected",
            Self::Arrived => "arrived",
            Self::Late => "late",
            Self::Missing => "missing",
            Self::Claimed => "claimed",
            Self::NotAvailable => "not_available",
            Self::Deleted => "deleted",
            Self::Bound => "bound",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "expected" => Some(Self::Expected),
            "arrived" => Some(Self::Arrived),
            "late" => Some(Self::Late),
            "missing" => Some(Self::Missing),
            "claimed" => Some(Self::Claimed),
            "not_available" => Some(Self::NotAvailable),
            "deleted" => Some(Self::Deleted),
            "bound" => Some(Self::Bound),
            _ => None,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Create or replace a serial subscription.
///
/// `x`/`y`/`z` and the inner loops are the prediction cursor. On create they
/// are the starting values. After the first horizon they are `nextState`, and
/// a later update must send that cursor back unchanged.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialSubscriptionWrite {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub site_id: i64,
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub vendor_id: Option<i64>,
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    /// Annual serial subscription cost in integer euro cents. Never copied to the item price.
    #[serde(default)]
    pub cost_cents: Option<i64>,
    /// Price of one issue, in cents. Null leaves the received item price empty.
    #[serde(default)]
    pub issue_price_cents: Option<i32>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub frequency_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub numbering_pattern_id: i64,
    pub first_issue_date: NaiveDate,
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default)]
    pub inner_x: u32,
    #[serde(default)]
    pub inner_y: u32,
    #[serde(default)]
    pub inner_z: u32,
    pub length: SubscriptionLength,
    #[serde(default)]
    pub irregularities: Vec<u32>,
    #[serde(default)]
    pub recurring_skips: Vec<RecurringSkip>,
    #[serde(default)]
    pub combined_issues: Vec<CombinedIssue>,
    /// Null uses the frequency-derived default. `0` is an explicit override.
    #[serde(default)]
    pub grace_days: Option<i32>,
    #[serde(default)]
    pub status: SerialSubscriptionStatus,
    #[serde(default = "default_true")]
    pub create_items: bool,
    #[serde(default = "default_true")]
    pub latest_issue_library_use_only: bool,
    #[serde(default)]
    #[validate(length(max = 4000))]
    pub holdings_statement: Option<String>,
    #[serde(default)]
    #[validate(length(max = 100))]
    pub call_number: Option<String>,
    #[serde(default)]
    #[validate(length(max = 200))]
    pub location: Option<String>,
    #[serde(default)]
    #[validate(length(max = 4000))]
    pub public_note: Option<String>,
    #[serde(default)]
    #[validate(length(max = 4000))]
    pub internal_note: Option<String>,
    #[serde(default)]
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

/// Search serial subscriptions by title, ISSN, site, vendor, or status.
#[serde_as]
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct SerialSubscriptionQuery {
    /// Title or ISSN substring. ISSN matches a linked series or collection.
    pub q: Option<String>,
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    #[serde(default)]
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub vendor_id: Option<i64>,
    pub status: Option<SerialSubscriptionStatus>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// One predicted or received issue row.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialIssue {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub serial_subscription_id: i64,
    pub planned_date: NaiveDate,
    pub display_label: String,
    /// Absent for a supplement (hors-série). Regular predicted issues always have a sequence.
    pub sequence: Option<u32>,
    /// Absent for a supplement (hors-série). Regular predicted issues always have a slot.
    pub slot: Option<u32>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub status: SerialIssueStatus,
    pub supplement: bool,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub item_id: Option<i64>,
    pub received_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Serial subscription at one site, plus the prediction cursor.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialSubscription {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub biblio_id: i64,
    pub biblio_title: Option<String>,
    /// ISSNs of series and collections linked to the periodical. Not `biblios.issn`.
    pub issns: Vec<String>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub site_id: i64,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub vendor_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub fund_id: Option<i64>,
    pub cost_cents: Option<i64>,
    pub issue_price_cents: Option<i32>,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub frequency_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub numbering_pattern_id: i64,
    pub first_issue_date: NaiveDate,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub inner_x: u32,
    pub inner_y: u32,
    pub inner_z: u32,
    pub next_slot: u32,
    pub length: SubscriptionLength,
    pub irregularities: Vec<u32>,
    pub recurring_skips: Vec<RecurringSkip>,
    pub combined_issues: Vec<CombinedIssue>,
    /// Stored override. Null means the frequency default applies.
    pub grace_days: Option<i32>,
    /// `grace_days` when set, otherwise the frequency default. Null for irregular.
    pub effective_grace_days: Option<i32>,
    pub status: SerialSubscriptionStatus,
    pub create_items: bool,
    pub latest_issue_library_use_only: bool,
    pub holdings_statement: Option<String>,
    pub call_number: Option<String>,
    pub location: Option<String>,
    pub public_note: Option<String>,
    pub internal_note: Option<String>,
    pub notes: Option<String>,
    pub issue_count: i64,
    /// Empty on list responses. Get, create, and update include the stored rows.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<SerialIssue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialSubscriptionListResponse {
    pub subscriptions: Vec<SerialSubscription>,
    pub total: i64,
}

/// Issues appended by `POST /serials/subscriptions/{id}/generate`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialIssueGenerateResponse {
    pub subscription: SerialSubscription,
    pub issues: Vec<SerialIssue>,
}

/// Receive an expected or late issue. An empty body allocates a barcode when the subscription creates items.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveSerialIssue {
    /// Scanned barcode. Omitted allocates one. Ignored only when absent; a barcode on a subscription that does not create items is rejected.
    #[serde(default)]
    #[validate(length(max = 100))]
    pub barcode: Option<String>,
    /// Calendar date the issue arrived. Omitted means today.
    #[serde(default)]
    pub received_on: Option<NaiveDate>,
}

/// Staff status for an issue that has not been received.
///
/// `missing` is claim-ready: the row can later move to `claimed`. Sending the claim is a later phase.
/// `notAvailable` is a issue that will not be published.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SerialIssueStaffStatus {
    Late,
    Missing,
    Claimed,
    NotAvailable,
    Deleted,
}

impl SerialIssueStaffStatus {
    #[must_use]
    pub fn as_issue_status(self) -> SerialIssueStatus {
        match self {
            Self::Late => SerialIssueStatus::Late,
            Self::Missing => SerialIssueStatus::Missing,
            Self::Claimed => SerialIssueStatus::Claimed,
            Self::NotAvailable => SerialIssueStatus::NotAvailable,
            Self::Deleted => SerialIssueStatus::Deleted,
        }
    }

    /// Statuses this target may replace. `claimed` may also follow `missing`.
    #[must_use]
    pub fn allowed_from(self) -> &'static [SerialIssueStatus] {
        match self {
            Self::Claimed => &[
                SerialIssueStatus::Expected,
                SerialIssueStatus::Late,
                SerialIssueStatus::Missing,
            ],
            Self::Late | Self::Missing | Self::NotAvailable | Self::Deleted => {
                &[SerialIssueStatus::Expected, SerialIssueStatus::Late]
            }
        }
    }
}

/// Body of `POST /serials/subscriptions/{id}/issues/{issueId}/status`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetSerialIssueStatus {
    pub status: SerialIssueStaffStatus,
}

/// A hors-série. Slot and sequence stay null so prediction is unchanged.
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSerialSupplement {
    pub planned_date: NaiveDate,
    #[validate(length(min = 1, max = 100))]
    pub display_label: String,
    #[serde(default)]
    #[validate(length(max = 4000))]
    pub notes: Option<String>,
}

/// Expected issues moved to `late` by grace.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SerialIssuesMarkedLate {
    pub issues: Vec<SerialIssue>,
}

/// One issue row to insert. Sequence is the stored sequence, not the engine's per-call index.
#[derive(Debug, Clone)]
pub struct NewSerialIssue {
    pub planned_date: NaiveDate,
    pub display_label: String,
    pub sequence: i32,
    pub slot: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
