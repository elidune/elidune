//! Opening-day calendar used when assigning loan due dates.
//!
//! Source of truth is the existing schedule tables:
//! - `schedule_slots.day_of_week` (0 = Monday … 6 = Sunday) on the active period
//! - `schedule_closures` for one-off closed days
//!
//! A date with **no covering period** is treated as open so libraries that have
//! not configured a schedule keep `now + duration_days` behaviour.
//! Fine accrual (#18) should reuse [`OpeningCalendar`] rather than a second calendar.

use std::collections::HashSet;

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, Utc};

/// Safety cap when walking forward from a closed due date.
pub const MAX_LOOKAHEAD_DAYS: i64 = 366;

/// Weekdays that have at least one open slot in one schedule period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodOpenDays {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub open_weekdays: HashSet<i16>,
}

impl PeriodOpenDays {
    pub fn new(
        start: NaiveDate,
        end: NaiveDate,
        open_weekdays: impl IntoIterator<Item = i16>,
    ) -> Self {
        Self {
            start,
            end,
            open_weekdays: open_weekdays.into_iter().collect(),
        }
    }

    fn covers(&self, date: NaiveDate) -> bool {
        self.start <= date && date <= self.end
    }
}

/// Snapshot of weekly openings and one-off closures for due-date adjustment.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpeningCalendar {
    pub closures: HashSet<NaiveDate>,
    /// Periods ordered most-recent `start` first. The first covering period wins.
    pub periods: Vec<PeriodOpenDays>,
}

impl OpeningCalendar {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_weekly(
        start: NaiveDate,
        end: NaiveDate,
        open_weekdays: impl IntoIterator<Item = i16>,
    ) -> Self {
        Self {
            closures: HashSet::new(),
            periods: vec![PeriodOpenDays::new(start, end, open_weekdays)],
        }
    }

    #[must_use]
    pub fn with_closure(mut self, date: NaiveDate) -> Self {
        self.closures.insert(date);
        self
    }

    /// Monday = 0 … Sunday = 6, matching `schedule_slots.day_of_week`.
    #[must_use]
    pub fn weekday_monday0(date: NaiveDate) -> i16 {
        date.weekday().num_days_from_monday() as i16
    }

    #[must_use]
    pub fn is_open(&self, date: NaiveDate) -> bool {
        if self.closures.contains(&date) {
            return false;
        }
        match self.periods.iter().find(|period| period.covers(date)) {
            None => true,
            Some(period) => period.open_weekdays.contains(&Self::weekday_monday0(date)),
        }
    }
}

/// Move `candidate` forward to the next open calendar day, keeping the clock time.
///
/// End-of-day vs opening-time is deferred (v1 keeps the original time-of-day).
#[must_use]
pub fn adjust_due_date(candidate: DateTime<Utc>, calendar: &OpeningCalendar) -> DateTime<Utc> {
    let time = candidate.time();
    let mut date = candidate.date_naive();
    for _ in 0..MAX_LOOKAHEAD_DAYS {
        if calendar.is_open(date) {
            return join_utc(date, time);
        }
        date = match date.succ_opt() {
            Some(next) => next,
            None => break,
        };
    }
    join_utc(date, time)
}

fn join_utc(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    date.and_time(time).and_utc()
}

/// UTC calendar date of `now + duration_days` (same arithmetic as a raw loan due date).
#[must_use]
pub fn raw_due_date(now: DateTime<Utc>, duration_days: i16) -> DateTime<Utc> {
    now + Duration::days(duration_days as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        date(y, m, d)
            .and_hms_opt(h, min, 0)
            .expect("valid time")
            .and_utc()
    }

    /// 2026-09-06 is a Sunday.
    fn sunday_due() -> DateTime<Utc> {
        utc(2026, 9, 6, 15, 30)
    }

    fn mon_sat_calendar() -> OpeningCalendar {
        OpeningCalendar::with_weekly(date(2026, 1, 1), date(2026, 12, 31), 0..=5)
    }

    #[test]
    fn weekday_monday0_matches_schedule_slots() {
        assert_eq!(OpeningCalendar::weekday_monday0(date(2026, 9, 7)), 0); // Mon
        assert_eq!(OpeningCalendar::weekday_monday0(date(2026, 9, 6)), 6); // Sun
    }

    #[test]
    fn no_schedule_keeps_candidate() {
        let candidate = sunday_due();
        assert_eq!(
            adjust_due_date(candidate, &OpeningCalendar::empty()),
            candidate
        );
    }

    #[test]
    fn weekly_closed_weekday_moves_to_next_open_day() {
        // Sunday closed, Monday open → Monday 15:30.
        let adjusted = adjust_due_date(sunday_due(), &mon_sat_calendar());
        assert_eq!(adjusted, utc(2026, 9, 7, 15, 30));
    }

    #[test]
    fn one_off_closure_moves_to_next_open_day() {
        let calendar = OpeningCalendar::with_weekly(date(2026, 1, 1), date(2026, 12, 31), 0..=6)
            .with_closure(date(2026, 9, 7));
        let adjusted = adjust_due_date(utc(2026, 9, 7, 10, 0), &calendar);
        assert_eq!(adjusted, utc(2026, 9, 8, 10, 0));
    }

    #[test]
    fn weekly_closed_plus_holiday_skips_both() {
        // Sunday closed + Monday holiday → Tuesday.
        let calendar = mon_sat_calendar().with_closure(date(2026, 9, 7));
        let adjusted = adjust_due_date(sunday_due(), &calendar);
        assert_eq!(adjusted, utc(2026, 9, 8, 15, 30));
    }

    #[test]
    fn already_open_day_is_unchanged() {
        let candidate = utc(2026, 9, 7, 9, 15);
        assert_eq!(adjust_due_date(candidate, &mon_sat_calendar()), candidate);
    }

    #[test]
    fn latest_covering_period_wins() {
        let mut calendar = OpeningCalendar::empty();
        calendar.periods.push(PeriodOpenDays::new(
            date(2026, 9, 1),
            date(2026, 9, 30),
            [0, 1, 2, 3, 4], // Mon–Fri (summer override)
        ));
        calendar.periods.push(PeriodOpenDays::new(
            date(2026, 1, 1),
            date(2026, 12, 31),
            0..=6, // year-round all days — ignored while summer covers
        ));
        let saturday = utc(2026, 9, 5, 12, 0);
        assert_eq!(adjust_due_date(saturday, &calendar), utc(2026, 9, 7, 12, 0));
    }

    #[test]
    fn exhausted_lookahead_stops() {
        let start = date(2026, 1, 1);
        let calendar = OpeningCalendar::with_weekly(
            start,
            start + Duration::days(400),
            std::iter::empty::<i16>(),
        );
        let candidate = utc(2026, 1, 1, 10, 0);
        let adjusted = adjust_due_date(candidate, &calendar);
        assert_eq!(
            adjusted.date_naive(),
            start + Duration::days(MAX_LOOKAHEAD_DAYS)
        );
    }

    #[test]
    fn raw_due_date_matches_duration_arithmetic() {
        let now = utc(2026, 8, 16, 11, 0);
        assert_eq!(raw_due_date(now, 21), utc(2026, 9, 6, 11, 0));
    }
}
