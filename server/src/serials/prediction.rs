//! Pure prediction of expected serial issues.
//!
//! No database access. Callers persist the returned rows; this module only
//! computes planned dates, counter values, and display labels.

use chrono::{Datelike, Duration, NaiveDate};

use crate::models::serial::{
    CombinedIssue, CounterFormat, CounterInners, CounterRule, CounterValues, FrequencySpec,
    FrequencyUnit, LengthKind, NumberingPatternSpec, PredictedIssue, PredictionState,
    RecurringSkip, SerialLocale, SubscriptionLength,
};

/// Hard cap so a preview cannot request an unbounded calendar.
pub const MAX_PREDICTED_ISSUES: u32 = 500;

pub const MAX_ISSUES_PER_UNIT: i32 = 366;
pub const MAX_UNITS_PER_ISSUE: i32 = 1_000;
pub const MAX_EVERY: u32 = 10_000;
pub const MAX_ZERO_PAD: u8 = 12;

const MAX_SLOTS: u32 = 5_000;

/// Why a prediction was refused. The service maps this to HTTP 400 or 422.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredictionError {
    /// `unit` is null. Dates cannot be invented for an irregular frequency.
    IrregularFrequency,
    InvalidLength,
    InvalidFrequency,
    /// `which` is `x`, `y`, or `z`.
    InvalidCounter(&'static str),
    EmptyFormula,
    LimitExceeded,
    DateOverflow,
    /// An irregularity ordinal was 0. Slots are 1-based.
    IrregularityOrdinal,
    /// `start_slot` was 0. Slots are 1-based.
    InvalidStartSlot,
    /// A recurring skip or combined-issue rule is not usable.
    InvalidSkip,
    /// A weeks or months window contained no issue.
    EmptyHorizon,
}

impl PredictionError {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::IrregularFrequency => {
                "An irregular frequency has no publication unit, so planned dates cannot be predicted".into()
            }
            Self::InvalidLength => {
                format!("Length count must be from 1 to {MAX_PREDICTED_ISSUES} issues, weeks, or months")
            }
            Self::InvalidFrequency => format!(
                "issuesPerUnit must be 1 to {MAX_ISSUES_PER_UNIT} and unitsPerIssue must be 1 to {MAX_UNITS_PER_ISSUE}"
            ),
            Self::InvalidCounter(which) => format!(
                "Counter {which} needs every between 1 and {MAX_EVERY}, a zeroPad of 1 to {MAX_ZERO_PAD} when set, and whenMoreThan together with setBackTo"
            ),
            Self::EmptyFormula => "Numbering formula must not be empty".into(),
            Self::LimitExceeded => format!(
                "Prediction would emit more than {MAX_PREDICTED_ISSUES} issues or walk more than {MAX_SLOTS} slots"
            ),
            Self::DateOverflow => "A planned date fell outside the supported calendar".into(),
            Self::IrregularityOrdinal => "Irregularity slots are 1-based".into(),
            Self::InvalidStartSlot => "startSlot is 1-based".into(),
            Self::InvalidSkip => {
                "A recurring skip needs a month from 1 to 12 and a day from 1 to 31 when set. A combined issue lists at least two distinct months and a month name; its first month is the one that is published".into()
            },
            Self::EmptyHorizon => {
                "This horizon produced no issues: every planned date in the window is skipped, or the window ends before the next issue".into()
            }
        }
    }

    /// Irregular frequency, the safety cap, and an empty horizon are
    /// well-formed requests the engine refuses. Everything else is a bad input.
    #[must_use]
    pub fn is_business_rule(&self) -> bool {
        matches!(
            self,
            Self::IrregularFrequency | Self::LimitExceeded | Self::EmptyHorizon
        )
    }
}

/// Inputs for one prediction run. Inner loops are 0 unless this continues a horizon.
#[derive(Debug, Clone)]
pub struct PredictionInput {
    pub first_issue_date: NaiveDate,
    pub frequency: FrequencySpec,
    pub pattern: NumberingPatternSpec,
    pub starting: CounterValues,
    pub inner: CounterInners,
    /// Absolute 1-based slot this run emits first. Later horizons keep
    /// `first_issue_date` and pass the previous `next_slot` here.
    pub start_slot: u32,
    pub length: SubscriptionLength,
    /// Absolute slot numbers from the original anchor, not from this horizon.
    pub irregularities: Vec<u32>,
    pub recurring_skips: Vec<RecurringSkip>,
    pub combined_issues: Vec<CombinedIssue>,
}

#[derive(Debug, Clone)]
pub struct PredictionOutput {
    pub issues: Vec<PredictedIssue>,
    pub next_state: PredictionState,
}

pub fn validate_frequency(spec: &FrequencySpec) -> Result<(), PredictionError> {
    if !(1..=MAX_ISSUES_PER_UNIT).contains(&spec.issues_per_unit)
        || !(1..=MAX_UNITS_PER_ISSUE).contains(&spec.units_per_issue)
    {
        return Err(PredictionError::InvalidFrequency);
    }
    Ok(())
}

pub fn validate_pattern(pattern: &NumberingPatternSpec) -> Result<(), PredictionError> {
    if pattern.formula.trim().is_empty() {
        return Err(PredictionError::EmptyFormula);
    }
    validate_counter(&pattern.x, "x")?;
    validate_counter(&pattern.y, "y")?;
    validate_counter(&pattern.z, "z")?;
    Ok(())
}

/// Generate the expected issue list. Does not read or write a database.
pub fn predict(input: &PredictionInput) -> Result<PredictionOutput, PredictionError> {
    validate_frequency(&input.frequency)?;
    validate_pattern(&input.pattern)?;
    if input.frequency.unit.is_none() {
        return Err(PredictionError::IrregularFrequency);
    }
    if input.length.count == 0 || input.length.count > MAX_PREDICTED_ISSUES {
        return Err(PredictionError::InvalidLength);
    }
    if input.irregularities.contains(&0) {
        return Err(PredictionError::IrregularityOrdinal);
    }
    if input.start_slot == 0 {
        return Err(PredictionError::InvalidStartSlot);
    }
    validate_skips(&input.recurring_skips, &input.combined_issues)?;

    // The window opens on this horizon's first slot, not on the original anchor.
    // A "12 more months" renewal at slot 13 must not inherit the first year's end.
    let horizon_start = date_for_slot(
        input.first_issue_date,
        &input.frequency,
        input.start_slot - 1,
    )?;
    let horizon_end = horizon_end(horizon_start, input.length)?;
    let skipped: std::collections::HashSet<u32> = input.irregularities.iter().copied().collect();
    let mut cursor = Cursors::from_input(input);
    let mut issues = Vec::new();
    let mut slot: u32 = input.start_slot;
    // Cap slots walked in this horizon, not the absolute slot. A daily at
    // slot 6000 is still a normal horizon; the old `slot <= 5000` check
    // refused it.
    let slot_limit = input.start_slot.saturating_add(MAX_SLOTS);

    while slot < slot_limit {
        let date = date_for_slot(input.first_issue_date, &input.frequency, slot - 1)?;
        if horizon_end.is_some_and(|end| date >= end) {
            break;
        }
        let month = u8::try_from(date.month()).map_err(|_| PredictionError::DateOverflow)?;
        let day = u8::try_from(date.day()).map_err(|_| PredictionError::DateOverflow)?;
        if slot_is_skipped(slot, month, day, &skipped, input) {
            slot += 1;
            continue;
        }
        if issues.len() as u32 >= MAX_PREDICTED_ISSUES {
            return Err(PredictionError::LimitExceeded);
        }
        let sequence = issues.len() as u32 + 1;
        let month_name_override = combined_month_name(&input.combined_issues, month);
        issues.push(PredictedIssue {
            sequence,
            slot,
            planned_date: date,
            display_label: render_label(&input.pattern, date, cursor.values(), month_name_override),
            x: cursor.x.value,
            y: cursor.y.value,
            z: cursor.z.value,
        });
        cursor.advance(&input.pattern)?;
        slot += 1;
        if input.length.kind == LengthKind::Issues && sequence == input.length.count {
            break;
        }
    }

    if input.length.kind == LengthKind::Issues && issues.len() as u32 != input.length.count {
        return Err(PredictionError::LimitExceeded);
    }
    if issues.is_empty() {
        return Err(PredictionError::EmptyHorizon);
    }

    Ok(PredictionOutput {
        issues,
        next_state: cursor.state(slot),
    })
}

fn validate_skips(
    recurring: &[RecurringSkip],
    combined: &[CombinedIssue],
) -> Result<(), PredictionError> {
    let mut published = std::collections::HashSet::new();
    let mut covered = std::collections::HashSet::new();
    for rule in combined {
        let name = rule.month_name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(PredictionError::InvalidSkip);
        }
        if rule.months.len() < 2 || rule.months.len() > 12 {
            return Err(PredictionError::InvalidSkip);
        }
        let mut seen = std::collections::HashSet::new();
        for (index, month) in rule.months.iter().copied().enumerate() {
            if !(1..=12).contains(&month) || !seen.insert(month) {
                return Err(PredictionError::InvalidSkip);
            }
            let clash = if index == 0 {
                !published.insert(month) || covered.contains(&month)
            } else {
                !covered.insert(month) || published.contains(&month)
            };
            if clash {
                return Err(PredictionError::InvalidSkip);
            }
        }
    }
    for rule in recurring {
        if !(1..=12).contains(&rule.month) {
            return Err(PredictionError::InvalidSkip);
        }
        if rule.day.is_some_and(|day| !(1..=31).contains(&day)) {
            return Err(PredictionError::InvalidSkip);
        }
        if rule.day.is_none() && published.contains(&rule.month) {
            return Err(PredictionError::InvalidSkip);
        }
    }
    Ok(())
}

fn slot_is_skipped(
    slot: u32,
    month: u8,
    day: u8,
    irregularities: &std::collections::HashSet<u32>,
    input: &PredictionInput,
) -> bool {
    if irregularities.contains(&slot) {
        return true;
    }
    // Yearly skips use the planned date. Slot numbers shift when the
    // subscription does not start in January, and again on the next horizon.
    if input
        .recurring_skips
        .iter()
        .any(|rule| rule.month == month && rule.day.is_none_or(|skipped_day| skipped_day == day))
    {
        return true;
    }
    input.combined_issues.iter().any(|rule| {
        rule.months
            .iter()
            .skip(1)
            .any(|skipped_month| *skipped_month == month)
    })
}

fn combined_month_name(rules: &[CombinedIssue], month: u8) -> Option<&str> {
    rules
        .iter()
        .find(|rule| rule.months.first() == Some(&month))
        .map(|rule| rule.month_name.trim())
}

fn validate_counter(rule: &CounterRule, which: &'static str) -> Result<(), PredictionError> {
    if rule.every == 0 || rule.every > MAX_EVERY {
        return Err(PredictionError::InvalidCounter(which));
    }
    if rule.when_more_than.is_some() != rule.set_back_to.is_some() {
        return Err(PredictionError::InvalidCounter(which));
    }
    if rule
        .zero_pad
        .is_some_and(|width| width == 0 || width > MAX_ZERO_PAD)
    {
        return Err(PredictionError::InvalidCounter(which));
    }
    Ok(())
}

/// Exclusive end of a weeks or months window. `start` is the planned date of
/// this horizon's first slot, which is the anchor only when `start_slot` is 1.
fn horizon_end(
    start: NaiveDate,
    length: SubscriptionLength,
) -> Result<Option<NaiveDate>, PredictionError> {
    match length.kind {
        LengthKind::Issues => Ok(None),
        LengthKind::Weeks => {
            let days = i64::from(length.count) * 7;
            add_days(start, days).map(Some)
        }
        LengthKind::Months => {
            let months = i32::try_from(length.count).map_err(|_| PredictionError::DateOverflow)?;
            add_months(start, months).map(Some)
        }
    }
}

/// Planned date of the 0-based publication slot.
fn date_for_slot(
    first: NaiveDate,
    frequency: &FrequencySpec,
    slot_index: u32,
) -> Result<NaiveDate, PredictionError> {
    let unit = frequency.unit.ok_or(PredictionError::IrregularFrequency)?;
    let issues =
        u32::try_from(frequency.issues_per_unit).map_err(|_| PredictionError::InvalidFrequency)?;
    let units =
        u32::try_from(frequency.units_per_issue).map_err(|_| PredictionError::InvalidFrequency)?;
    if issues == 0 {
        return Err(PredictionError::InvalidFrequency);
    }
    let cycle = slot_index / issues;
    let slot = slot_index % issues;

    match unit {
        FrequencyUnit::Day | FrequencyUnit::Week => {
            let unit_days: u64 = if unit == FrequencyUnit::Week { 7 } else { 1 };
            let cycle_days = u64::from(units) * unit_days;
            let offset = u64::from(slot) * cycle_days / u64::from(issues);
            let total = u64::from(cycle) * cycle_days + offset;
            let days = i64::try_from(total).map_err(|_| PredictionError::DateOverflow)?;
            add_days(first, days)
        }
        FrequencyUnit::Month | FrequencyUnit::Year => {
            let months_per_unit: i32 = if unit == FrequencyUnit::Year { 12 } else { 1 };
            let months_per_cycle = i32::try_from(units)
                .ok()
                .and_then(|units| units.checked_mul(months_per_unit))
                .ok_or(PredictionError::DateOverflow)?;
            let cycle_i = i32::try_from(cycle).map_err(|_| PredictionError::DateOverflow)?;
            let cycle_months = cycle_i
                .checked_mul(months_per_cycle)
                .ok_or(PredictionError::DateOverflow)?;
            let cycle_start = add_months(first, cycle_months)?;
            if slot == 0 {
                return Ok(cycle_start);
            }
            let cycle_end = add_months(first, cycle_months + months_per_cycle)?;
            let span = cycle_end
                .signed_duration_since(cycle_start)
                .num_days()
                .max(0);
            let offset = i64::from(slot) * span / i64::from(issues);
            add_days(cycle_start, offset)
        }
    }
}

fn add_days(date: NaiveDate, days: i64) -> Result<NaiveDate, PredictionError> {
    date.checked_add_signed(Duration::days(days))
        .ok_or(PredictionError::DateOverflow)
}

/// Add calendar months from `date`, clamping the day to the target month.
fn add_months(date: NaiveDate, months: i32) -> Result<NaiveDate, PredictionError> {
    let month0 = i32::try_from(date.month0()).map_err(|_| PredictionError::DateOverflow)?;
    let total = month0
        .checked_add(months)
        .ok_or(PredictionError::DateOverflow)?;
    let year = date
        .year()
        .checked_add(total.div_euclid(12))
        .ok_or(PredictionError::DateOverflow)?;
    let month = u32::try_from(total.rem_euclid(12)).map_err(|_| PredictionError::DateOverflow)? + 1;
    let last = last_day(year, month)?;
    let day = date.day().min(last);
    NaiveDate::from_ymd_opt(year, month, day).ok_or(PredictionError::DateOverflow)
}

fn last_day(year: i32, month: u32) -> Result<u32, PredictionError> {
    let next = if month == 12 {
        let next_year = year.checked_add(1).ok_or(PredictionError::DateOverflow)?;
        NaiveDate::from_ymd_opt(next_year, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.and_then(|day| day.pred_opt())
        .map(|day| day.day())
        .ok_or(PredictionError::DateOverflow)
}

#[derive(Clone, Copy)]
struct CounterCursor {
    value: i32,
    inner: u32,
}

struct Cursors {
    x: CounterCursor,
    y: CounterCursor,
    z: CounterCursor,
}

impl Cursors {
    fn from_input(input: &PredictionInput) -> Self {
        Self {
            x: CounterCursor {
                value: input.starting.x,
                inner: input.inner.x,
            },
            y: CounterCursor {
                value: input.starting.y,
                inner: input.inner.y,
            },
            z: CounterCursor {
                value: input.starting.z,
                inner: input.inner.z,
            },
        }
    }

    fn values(&self) -> CounterValues {
        CounterValues {
            x: self.x.value,
            y: self.y.value,
            z: self.z.value,
        }
    }

    fn state(&self, next_slot: u32) -> PredictionState {
        PredictionState {
            x: self.x.value,
            y: self.y.value,
            z: self.z.value,
            inner_x: self.x.inner,
            inner_y: self.y.inner,
            inner_z: self.z.inner,
            next_slot,
        }
    }

    fn advance(&mut self, pattern: &NumberingPatternSpec) -> Result<(), PredictionError> {
        self.x = advance_counter(self.x, &pattern.x, "x")?;
        self.y = advance_counter(self.y, &pattern.y, "y")?;
        self.z = advance_counter(self.z, &pattern.z, "z")?;
        Ok(())
    }
}

fn advance_counter(
    cursor: CounterCursor,
    rule: &CounterRule,
    which: &'static str,
) -> Result<CounterCursor, PredictionError> {
    if rule.every == 0 {
        return Err(PredictionError::InvalidCounter(which));
    }
    let inner = cursor.inner.saturating_add(1);
    if inner < rule.every {
        return Ok(CounterCursor {
            value: cursor.value,
            inner,
        });
    }
    let previous = cursor.value;
    let mut value = previous.saturating_add(rule.add);
    if let Some(limit) = rule.when_more_than {
        let reset = rule
            .set_back_to
            .ok_or(PredictionError::InvalidCounter(which))?;
        // Cross the limit once. A set-back above the limit (3 → 100) keeps
        // counting; a set-back at or below it starts the next cycle (12 → 1).
        if previous <= limit && value > limit {
            value = reset;
        }
    }
    Ok(CounterCursor { value, inner: 0 })
}

fn render_label(
    pattern: &NumberingPatternSpec,
    date: NaiveDate,
    values: CounterValues,
    month_name_override: Option<&str>,
) -> String {
    let mut label = pattern.formula.clone();
    label = label.replace("{X}", &render_counter(values.x, &pattern.x, pattern.locale));
    label = label.replace("{Y}", &render_counter(values.y, &pattern.y, pattern.locale));
    label = label.replace("{Z}", &render_counter(values.z, &pattern.z, pattern.locale));
    let month_label = month_name_override
        .unwrap_or_else(|| month_name(date.month(), pattern.locale).unwrap_or(""));
    label = label.replace("{MonthName}", month_label);
    label = label.replace("{DayName}", day_name_from_date(date, pattern.locale));
    label = label.replace("{Year}", &date.year().to_string());
    label = label.replace("{Month}", &date.month().to_string());
    label = label.replace("{Day}", &date.day().to_string());
    label
}

fn render_counter(value: i32, rule: &CounterRule, locale: SerialLocale) -> String {
    match rule.format {
        CounterFormat::Number => match rule.zero_pad {
            Some(width) if value >= 0 => format!("{value:0width$}", width = usize::from(width)),
            _ => value.to_string(),
        },
        CounterFormat::DayName => {
            named(value, day_names(locale)).unwrap_or_else(|| value.to_string())
        }
        CounterFormat::MonthName => month_name(u32::try_from(value).unwrap_or(0), locale)
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string()),
        CounterFormat::Season => {
            named(value, season_names(locale)).unwrap_or_else(|| value.to_string())
        }
    }
}

fn named(value: i32, names: &'static [&'static str]) -> Option<String> {
    let index = usize::try_from(value.checked_sub(1)?).ok()?;
    names.get(index).map(|name| (*name).to_string())
}

fn month_name(month: u32, locale: SerialLocale) -> Option<&'static str> {
    let names: &[&str] = match locale {
        SerialLocale::Fr => &[
            "janvier",
            "février",
            "mars",
            "avril",
            "mai",
            "juin",
            "juillet",
            "août",
            "septembre",
            "octobre",
            "novembre",
            "décembre",
        ],
        SerialLocale::En => &[
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
    };
    let index = usize::try_from(month.checked_sub(1)?).ok()?;
    names.get(index).copied()
}

fn day_names(locale: SerialLocale) -> &'static [&'static str] {
    match locale {
        SerialLocale::Fr => &[
            "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
        ],
        SerialLocale::En => &[
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ],
    }
}

fn season_names(locale: SerialLocale) -> &'static [&'static str] {
    match locale {
        SerialLocale::Fr => &["hiver", "printemps", "été", "automne"],
        SerialLocale::En => &["winter", "spring", "summer", "autumn"],
    }
}

fn day_name_from_date(date: NaiveDate, locale: SerialLocale) -> &'static str {
    let index = date.weekday().num_days_from_monday() as usize;
    day_names(locale).get(index).copied().unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("date")
    }

    fn monthly() -> FrequencySpec {
        FrequencySpec {
            unit: Some(FrequencyUnit::Month),
            issues_per_unit: 1,
            units_per_issue: 1,
        }
    }

    fn weekly() -> FrequencySpec {
        FrequencySpec {
            unit: Some(FrequencyUnit::Week),
            issues_per_unit: 1,
            units_per_issue: 1,
        }
    }

    fn counter(add: i32, every: u32) -> CounterRule {
        CounterRule {
            add,
            every,
            ..CounterRule::default()
        }
    }

    fn numbering(formula: &str) -> NumberingPatternSpec {
        NumberingPatternSpec {
            formula: formula.to_string(),
            locale: SerialLocale::Fr,
            x: counter(1, 1),
            y: CounterRule::default(),
            z: CounterRule::default(),
        }
    }

    fn run(input: PredictionInput) -> PredictionOutput {
        predict(&input).expect("prediction")
    }

    fn base(frequency: FrequencySpec, pattern: NumberingPatternSpec) -> PredictionInput {
        PredictionInput {
            first_issue_date: date(2026, 3, 1),
            frequency,
            pattern,
            starting: CounterValues { x: 1, y: 1, z: 1 },
            inner: CounterInners::default(),
            length: SubscriptionLength {
                kind: LengthKind::Issues,
                count: 1,
            },
            start_slot: 1,
            irregularities: Vec::new(),
            recurring_skips: Vec::new(),
            combined_issues: Vec::new(),
        }
    }

    #[test]
    fn display_label_combines_number_and_french_month() {
        let mut input = base(monthly(), numbering("n° {X}, {MonthName} {Year}"));
        input.starting.x = 412;
        input.length.count = 3;
        let out = run(input);
        let labels: Vec<&str> = out
            .issues
            .iter()
            .map(|issue| issue.display_label.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "n° 412, mars 2026",
                "n° 413, avril 2026",
                "n° 414, mai 2026",
            ]
        );
    }

    #[test]
    fn english_month_and_unpadded_date_tokens() {
        assert_eq!(date(2026, 3, 2).weekday(), Weekday::Mon);
        let mut pattern = numbering("{DayName} {Day}/{Month}/{Year}");
        pattern.locale = SerialLocale::En;
        let mut input = base(monthly(), pattern);
        input.first_issue_date = date(2026, 3, 2);
        let out = run(input);
        assert_eq!(out.issues[0].display_label, "Monday 2/3/2026");
    }

    #[test]
    fn year_rollover_and_month_end_do_not_drift() {
        let mut input = base(monthly(), numbering("{MonthName} {Year}"));
        input.first_issue_date = date(2026, 11, 15);
        input.length.count = 4;
        let out = run(input);
        assert_eq!(
            out.issues
                .iter()
                .map(|issue| (issue.planned_date, issue.display_label.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (date(2026, 11, 15), "novembre 2026"),
                (date(2026, 12, 15), "décembre 2026"),
                (date(2027, 1, 15), "janvier 2027"),
                (date(2027, 2, 15), "février 2027"),
            ]
        );

        let mut clamped = base(monthly(), numbering("{Day}"));
        clamped.first_issue_date = date(2026, 1, 31);
        clamped.length.count = 4;
        let days: Vec<NaiveDate> = run(clamped)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(
            days,
            vec![
                date(2026, 1, 31),
                date(2026, 2, 28),
                date(2026, 3, 31),
                date(2026, 4, 30),
            ]
        );
    }

    #[test]
    fn leap_day_plus_one_year_clamps_to_february_28() {
        let mut input = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Year),
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{Year}-{Month}-{Day}"),
        );
        input.first_issue_date = date(2024, 2, 29);
        input.length.count = 2;
        let out = run(input);
        assert_eq!(out.issues[0].planned_date, date(2024, 2, 29));
        assert_eq!(out.issues[1].planned_date, date(2025, 2, 28));
        assert_eq!(out.issues[1].display_label, "2025-2-28");
    }

    #[test]
    fn several_issues_per_unit_are_spaced_inside_the_cycle() {
        let mut three_week = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Week),
                issues_per_unit: 3,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        three_week.first_issue_date = date(2026, 1, 5);
        three_week.length.count = 4;
        let dates: Vec<_> = run(three_week)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(
            dates,
            vec![
                date(2026, 1, 5),
                date(2026, 1, 7),
                date(2026, 1, 9),
                date(2026, 1, 12),
            ]
        );

        let mut twice_a_day = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Day),
                issues_per_unit: 2,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        twice_a_day.first_issue_date = date(2026, 1, 1);
        twice_a_day.length.count = 4;
        let dates: Vec<_> = run(twice_a_day)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(
            dates,
            vec![
                date(2026, 1, 1),
                date(2026, 1, 1),
                date(2026, 1, 2),
                date(2026, 1, 2),
            ]
        );

        let mut twice_a_month = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Month),
                issues_per_unit: 2,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        twice_a_month.first_issue_date = date(2026, 1, 1);
        twice_a_month.length.count = 3;
        let dates: Vec<_> = run(twice_a_month)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(
            dates,
            vec![date(2026, 1, 1), date(2026, 1, 16), date(2026, 2, 1)]
        );
    }

    #[test]
    fn counter_resets_at_volume_boundary_and_can_renumber() {
        let mut pattern = numbering("v{Y} n{X}");
        pattern.x.when_more_than = Some(12);
        pattern.x.set_back_to = Some(1);
        pattern.y = counter(1, 12);
        let mut input = base(monthly(), pattern);
        input.starting = CounterValues { x: 1, y: 1, z: 0 };
        input.length.count = 13;
        let out = run(input);
        assert_eq!(out.issues[0].display_label, "v1 n1");
        assert_eq!(out.issues[11].display_label, "v1 n12");
        assert_eq!(out.issues[12].display_label, "v2 n1");
        assert_eq!(out.next_state.x, 2);
        assert_eq!(out.next_state.y, 2);
        // Issue 12 resets the Y inner loop when the volume advances. Issue 13
        // then increments that loop once, so the cursor is 1 of 12 toward volume 3.
        assert_eq!(out.next_state.inner_y, 1);

        let mut renumber = numbering("{X}");
        renumber.x.when_more_than = Some(3);
        renumber.x.set_back_to = Some(100);
        let mut input = base(weekly(), renumber);
        input.length.count = 5;
        let labels: Vec<_> = run(input)
            .issues
            .into_iter()
            .map(|issue| issue.display_label)
            .collect();
        assert_eq!(labels, vec!["1", "2", "3", "100", "101"]);
    }

    #[test]
    fn every_two_issues_and_inner_loop_continuation() {
        let mut pattern = numbering("{X}");
        pattern.x = counter(1, 2);
        let mut input = base(weekly(), pattern.clone());
        input.length.count = 4;
        let values: Vec<i32> = run(input).issues.into_iter().map(|issue| issue.x).collect();
        assert_eq!(values, vec![1, 1, 2, 2]);

        let mut continued = base(weekly(), pattern);
        continued.starting.x = 1;
        continued.inner.x = 1;
        continued.length.count = 2;
        let out = run(continued);
        assert_eq!(out.issues[0].x, 1);
        assert_eq!(out.issues[1].x, 2);
    }

    #[test]
    fn irregularities_skip_the_slot_without_advancing_numbering() {
        let mut input = base(weekly(), numbering("{X}"));
        input.first_issue_date = date(2026, 1, 5);
        input.starting.x = 5;
        input.length.count = 3;
        input.irregularities = vec![2, 3, 2];
        let out = run(input);
        assert_eq!(
            out.issues
                .iter()
                .map(|issue| (issue.slot, issue.planned_date, issue.x))
                .collect::<Vec<_>>(),
            vec![
                (1, date(2026, 1, 5), 5),
                (4, date(2026, 1, 26), 6),
                (5, date(2026, 2, 2), 7),
            ]
        );
    }

    #[test]
    fn week_and_month_lengths_stop_before_the_exclusive_end() {
        let mut weeks = base(weekly(), numbering("{X}"));
        weeks.first_issue_date = date(2026, 1, 5);
        weeks.length = SubscriptionLength {
            kind: LengthKind::Weeks,
            count: 2,
        };
        let dates: Vec<_> = run(weeks)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(dates, vec![date(2026, 1, 5), date(2026, 1, 12)]);

        let mut months = base(monthly(), numbering("{X}"));
        months.first_issue_date = date(2026, 1, 15);
        months.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 12,
        };
        assert_eq!(run(months).issues.len(), 12);
    }

    #[test]
    fn season_cycle_and_out_of_range_name_falls_back_to_the_number() {
        let mut pattern = numbering("{X}");
        pattern.x.format = CounterFormat::Season;
        pattern.x.when_more_than = Some(4);
        pattern.x.set_back_to = Some(1);
        let mut input = base(monthly(), pattern);
        input.length.count = 5;
        let labels: Vec<_> = run(input)
            .issues
            .into_iter()
            .map(|issue| issue.display_label)
            .collect();
        assert_eq!(
            labels,
            vec!["hiver", "printemps", "été", "automne", "hiver"]
        );

        let mut odd = numbering("{X}");
        odd.x.format = CounterFormat::DayName;
        odd.x.add = 0;
        let mut input = base(weekly(), odd);
        input.starting.x = 9;
        assert_eq!(run(input).issues[0].display_label, "9");
    }

    #[test]
    fn zero_pad_and_two_week_frequency() {
        let mut pattern = numbering("{X}");
        pattern.x.zero_pad = Some(3);
        let mut input = base(weekly(), pattern);
        input.starting.x = 7;
        assert_eq!(run(input).issues[0].display_label, "007");

        let mut fortnight = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Week),
                issues_per_unit: 1,
                units_per_issue: 2,
            },
            numbering("{X}"),
        );
        fortnight.first_issue_date = date(2026, 1, 5);
        fortnight.length.count = 3;
        let dates: Vec<_> = run(fortnight)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(
            dates,
            vec![date(2026, 1, 5), date(2026, 1, 19), date(2026, 2, 2)]
        );
    }

    #[test]
    fn irregular_frequency_length_and_cap_are_refused() {
        let mut irregular = base(
            FrequencySpec {
                unit: None,
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        irregular.length.count = 2;
        assert_eq!(
            predict(&irregular).unwrap_err(),
            PredictionError::IrregularFrequency
        );

        let mut zero = base(weekly(), numbering("{X}"));
        zero.length.count = 0;
        assert_eq!(predict(&zero).unwrap_err(), PredictionError::InvalidLength);

        let mut huge = base(weekly(), numbering("{X}"));
        huge.length.count = MAX_PREDICTED_ISSUES + 1;
        assert_eq!(predict(&huge).unwrap_err(), PredictionError::InvalidLength);

        let mut daily = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Day),
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        daily.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 24,
        };
        assert_eq!(predict(&daily).unwrap_err(), PredictionError::LimitExceeded);

        let mut bad_counter = numbering("{X}");
        bad_counter.x.every = 0;
        assert!(validate_pattern(&bad_counter).is_err());
        bad_counter.x.every = 1;
        bad_counter.x.when_more_than = Some(10);
        assert!(validate_pattern(&bad_counter).is_err());

        let mut zero_slot = base(weekly(), numbering("{X}"));
        zero_slot.irregularities = vec![0];
        assert_eq!(
            predict(&zero_slot).unwrap_err(),
            PredictionError::IrregularityOrdinal
        );

        let mut zero_start = base(weekly(), numbering("{X}"));
        zero_start.start_slot = 0;
        assert_eq!(
            predict(&zero_start).unwrap_err(),
            PredictionError::InvalidStartSlot
        );
    }

    #[test]
    fn two_horizons_match_one_horizon() {
        let mut end_of_month = base(monthly(), numbering("{Day}"));
        end_of_month.first_issue_date = date(2026, 1, 31);
        assert_horizons_match_lengths(end_of_month, &[LengthKind::Issues, LengthKind::Months]);

        let mut twice_a_month = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Month),
                issues_per_unit: 2,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        twice_a_month.first_issue_date = date(2026, 1, 1);
        assert_horizons_match_lengths(
            twice_a_month.clone(),
            &[LengthKind::Issues, LengthKind::Months],
        );
        let full = {
            let mut once = twice_a_month.clone();
            once.length.count = 12;
            run(once)
        };
        assert_eq!(full.issues[1].planned_date, date(2026, 1, 16));
        assert_eq!(full.issues[6].planned_date, date(2026, 4, 1));

        let mut skipped_august_slot = base(monthly(), numbering("{X}"));
        skipped_august_slot.first_issue_date = date(2026, 1, 1);
        skipped_august_slot.irregularities = vec![8];
        assert_horizons_match_lengths(
            skipped_august_slot,
            &[LengthKind::Issues, LengthKind::Months],
        );

        let mut recurring = base(monthly(), numbering("{MonthName}"));
        recurring.first_issue_date = date(2026, 1, 15);
        recurring.recurring_skips = vec![RecurringSkip {
            month: 8,
            day: None,
        }];
        assert_horizons_match_lengths(recurring, &[LengthKind::Issues, LengthKind::Months]);

        // October start: August is not slot 8, and it falls in the second horizon.
        let mut from_october = base(monthly(), numbering("{MonthName}"));
        from_october.first_issue_date = date(2026, 10, 1);
        from_october.recurring_skips = vec![RecurringSkip {
            month: 8,
            day: None,
        }];
        assert_horizons_match_lengths(
            from_october.clone(),
            &[LengthKind::Issues, LengthKind::Months],
        );

        let mut weekly_run = base(weekly(), numbering("{X}"));
        weekly_run.first_issue_date = date(2026, 1, 5);
        assert_horizons_match_lengths(
            weekly_run,
            &[LengthKind::Issues, LengthKind::Weeks, LengthKind::Months],
        );
        let mut issues_year = from_october.clone();
        issues_year.length.kind = LengthKind::Issues;
        issues_year.length.count = 12;
        let issued = run(issues_year);
        let dates: Vec<_> = issued
            .issues
            .iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert!(dates.contains(&date(2027, 5, 1)));
        assert!(!dates.contains(&date(2027, 8, 1)));
        assert!(dates.iter().all(|planned| planned.month() != 8));

        let mut renewal = base(monthly(), numbering("{Day}"));
        renewal.first_issue_date = date(2026, 1, 31);
        renewal.start_slot = 13;
        renewal.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 12,
        };
        let renewed = run(renewal);
        assert_eq!(renewed.issues[0].planned_date, date(2027, 1, 31));
        assert_eq!(
            renewed.issues.last().expect("last issue").planned_date,
            date(2027, 12, 31)
        );

        let mut blocked = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Day),
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{Day}"),
        );
        blocked.first_issue_date = date(2026, 8, 1);
        blocked.length = SubscriptionLength {
            kind: LengthKind::Weeks,
            count: 1,
        };
        blocked.recurring_skips = vec![RecurringSkip {
            month: 8,
            day: None,
        }];
        assert_eq!(
            predict(&blocked).unwrap_err(),
            PredictionError::EmptyHorizon
        );
    }

    #[test]
    fn a_late_daily_horizon_is_not_capped_by_the_absolute_slot() {
        let anchor = date(2026, 1, 1) + chrono::Duration::days(5_999);
        let mut input = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Day),
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{X}"),
        );
        input.first_issue_date = date(2026, 1, 1);
        input.start_slot = 6_000;
        input.length = SubscriptionLength {
            kind: LengthKind::Weeks,
            count: 2,
        };
        let out = run(input);
        assert_eq!(out.issues.len(), 14);
        assert_eq!(out.issues[0].slot, 6_000);
        assert_eq!(out.issues[0].planned_date, anchor);
        assert_eq!(out.issues[13].slot, 6_013);
        assert_eq!(
            out.issues[13].planned_date,
            anchor + chrono::Duration::days(13)
        );
        assert_eq!(out.next_state.next_slot, 6_014);
    }

    #[test]
    fn yearly_skips_cover_ten_and_eleven_issues_and_a_double_issue() {
        let mut eleven = base(monthly(), numbering("{MonthName}"));
        eleven.first_issue_date = date(2026, 1, 15);
        eleven.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 12,
        };
        eleven.recurring_skips = vec![RecurringSkip {
            month: 8,
            day: None,
        }];
        let dates: Vec<_> = run(eleven.clone())
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(dates.len(), 11);
        assert_eq!(dates[6], date(2026, 7, 15));
        assert_eq!(dates[7], date(2026, 9, 15));
        assert!(dates.iter().all(|planned| planned.month() != 8));

        eleven.length.count = 24;
        let second_year = run(eleven);
        assert_eq!(second_year.issues.len(), 22);
        assert!(second_year
            .issues
            .iter()
            .all(|issue| issue.planned_date.month() != 8));

        let mut ten = base(monthly(), numbering("{X}"));
        ten.first_issue_date = date(2026, 1, 1);
        ten.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 12,
        };
        ten.recurring_skips = vec![
            RecurringSkip {
                month: 7,
                day: None,
            },
            RecurringSkip {
                month: 8,
                day: None,
            },
        ];
        let summer = run(ten);
        assert_eq!(summer.issues.len(), 10);
        assert!(summer.issues.iter().all(|issue| {
            let month = issue.planned_date.month();
            month != 7 && month != 8
        }));

        let mut combined = base(monthly(), numbering("n° {X}, {MonthName} {Year}"));
        combined.first_issue_date = date(2026, 1, 1);
        combined.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 24,
        };
        combined.combined_issues = vec![CombinedIssue {
            months: vec![7, 8],
            month_name: "juillet-août".into(),
        }];
        let out = run(combined);
        assert_eq!(out.issues.len(), 22);
        assert!(out
            .issues
            .iter()
            .all(|issue| issue.planned_date.month() != 8));
        assert_eq!(out.issues[6].display_label, "n° 7, juillet-août 2026");
        assert_eq!(out.issues[6].planned_date, date(2026, 7, 1));
        assert_eq!(out.issues[7].display_label, "n° 8, septembre 2026");
        assert_eq!(out.issues[17].display_label, "n° 18, juillet-août 2027");

        let mut christmas = base(
            FrequencySpec {
                unit: Some(FrequencyUnit::Day),
                issues_per_unit: 1,
                units_per_issue: 1,
            },
            numbering("{Day}"),
        );
        christmas.first_issue_date = date(2026, 12, 1);
        christmas.length = SubscriptionLength {
            kind: LengthKind::Months,
            count: 1,
        };
        christmas.recurring_skips = vec![RecurringSkip {
            month: 12,
            day: Some(25),
        }];
        let days: Vec<_> = run(christmas)
            .issues
            .into_iter()
            .map(|issue| issue.planned_date)
            .collect();
        assert_eq!(days.len(), 30);
        assert!(!days.contains(&date(2026, 12, 25)));
        assert!(days.contains(&date(2026, 12, 24)));
        assert!(days.contains(&date(2026, 12, 26)));
    }

    fn assert_horizons_match_lengths(input: PredictionInput, kinds: &[LengthKind]) {
        for kind in kinds {
            assert_horizons_match(&input, *kind);
        }
    }

    fn assert_horizons_match(input: &PredictionInput, kind: LengthKind) {
        let mut full_input = input.clone();
        full_input.length.kind = kind;
        full_input.length.count = 12;
        let full = run(full_input.clone());
        let mut first_input = full_input.clone();
        first_input.length.count = 6;
        let first = run(first_input);
        let mut second_input = full_input;
        second_input.length.count = 6;
        second_input.start_slot = first.next_state.next_slot;
        second_input.starting = CounterValues {
            x: first.next_state.x,
            y: first.next_state.y,
            z: first.next_state.z,
        };
        second_input.inner = CounterInners {
            x: first.next_state.inner_x,
            y: first.next_state.inner_y,
            z: first.next_state.inner_z,
        };
        let second = run(second_input);
        let combined: Vec<_> = first
            .issues
            .iter()
            .chain(second.issues.iter())
            .map(issue_fingerprint)
            .collect();
        assert_eq!(
            combined,
            full.issues
                .iter()
                .map(issue_fingerprint)
                .collect::<Vec<_>>(),
            "{kind:?}"
        );
        assert_eq!(second.next_state, full.next_state, "{kind:?}");
    }

    fn issue_fingerprint(issue: &PredictedIssue) -> (u32, NaiveDate, String, i32, i32, i32) {
        (
            issue.slot,
            issue.planned_date,
            issue.display_label.clone(),
            issue.x,
            issue.y,
            issue.z,
        )
    }
}
