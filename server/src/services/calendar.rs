//! Library timezone and opening-day helpers.
//!
//! Patron-facing dates are rendered in the library IANA timezone (`library_info.timezone`,
//! default `Europe/Paris`). Open-day questions use the loan site: network-wide closures
//! and that site's own closures both count, together with the weekly schedule.
//! Nothing here reads the container timezone.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::{
    circulation_calendar::OpeningCalendar,
    error::{AppError, AppResult},
};

/// Default IANA zone when the library has not chosen another.
pub const DEFAULT_TIMEZONE: &str = "Europe/Paris";

/// Patron-facing calendar date (`dd/mm/yyyy`).
pub const PATRON_DATE_FMT: &str = "%d/%m/%Y";

/// Patron-facing date and time, 24-hour clock, no zone suffix (`dd/mm/yyyy HH:MM`).
pub const PATRON_DATETIME_FMT: &str = "%d/%m/%Y %H:%M";

/// `Europe/Paris`.
#[must_use]
pub fn default_zone() -> Tz {
    chrono_tz::Europe::Paris
}

/// Parse an IANA name. Empty and unknown names are errors.
pub fn parse_timezone(name: &str) -> Result<Tz, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidTimezone(
            "Timezone is required".to_string(),
        ));
    }
    trimmed
        .parse::<Tz>()
        .map_err(|_| AppError::InvalidTimezone(format!("Unknown IANA timezone '{trimmed}'")))
}

/// Civil date of `instant` in `zone`.
#[must_use]
pub fn local_date(zone: Tz, instant: DateTime<Utc>) -> NaiveDate {
    instant.with_timezone(&zone).date_naive()
}

/// Library-local date a public copy is expected back.
///
/// `due` and `now` are compared as civil dates in `zone`, never as UTC dates.
/// The date is returned while the local due date is today or later. It is
/// `None` from the next local day (local due date strictly before local today).
/// In the 00:00-02:00 window of a positive offset (Europe/Paris), UTC is still
/// on the previous civil day: a due date of that previous local day is omitted
/// even when its UTC date equals the UTC date of `now`.
#[must_use]
pub fn public_expected_return_date(
    zone: Tz,
    due: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<NaiveDate> {
    let due_day = local_date(zone, due);
    let today = local_date(zone, now);
    (due_day >= today).then_some(due_day)
}

/// `dd/mm/yyyy` in `zone`.
#[must_use]
pub fn format_patron_date(zone: Tz, instant: DateTime<Utc>) -> String {
    instant
        .with_timezone(&zone)
        .format(PATRON_DATE_FMT)
        .to_string()
}

/// `dd/mm/yyyy HH:MM` in `zone` (no `UTC` suffix).
#[must_use]
pub fn format_patron_datetime(zone: Tz, instant: DateTime<Utc>) -> String {
    instant
        .with_timezone(&zone)
        .format(PATRON_DATETIME_FMT)
        .to_string()
}

/// `YYYY-MM-DD` civil date in `zone`. Kept for messages the UI parses as ISO dates.
#[must_use]
pub fn format_patron_iso_date(zone: Tz, instant: DateTime<Utc>) -> String {
    local_date(zone, instant).to_string()
}

/// A stored calendar date (not an instant), `dd/mm/yyyy`.
#[must_use]
pub fn format_calendar_date(date: NaiveDate) -> String {
    date.format(PATRON_DATE_FMT).to_string()
}

/// A stored clock time, `HH:MM`.
#[must_use]
pub fn format_clock(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}

/// Whether `date` is an open day for `site_id`.
///
/// Network-wide closures and closures of `site_id` both close the day.
/// `site_id == None` still honours network-wide closures and the weekly schedule.
#[must_use]
pub fn is_open(calendar: &OpeningCalendar, site_id: Option<i64>, date: NaiveDate) -> bool {
    calendar.is_open_on(site_id, date)
}

/// First open day of `site_id` on or after `date`.
#[must_use]
pub fn next_open_day(
    calendar: &OpeningCalendar,
    site_id: Option<i64>,
    date: NaiveDate,
) -> NaiveDate {
    calendar.next_open_day(site_id, date)
}

/// Open days of `site_id` strictly after `from` and on or before `to`.
#[must_use]
pub fn open_days_between(
    calendar: &OpeningCalendar,
    site_id: Option<i64>,
    from: NaiveDate,
    to: NaiveDate,
) -> i64 {
    calendar.open_days_between(site_id, from, to)
}

/// Move `candidate` to the next open local day of `site_id`, keeping the local clock time.
#[must_use]
pub fn slide_due_date(
    candidate: DateTime<Utc>,
    zone: Tz,
    site_id: Option<i64>,
    calendar: &OpeningCalendar,
) -> DateTime<Utc> {
    let local = candidate.with_timezone(&zone);
    let open = next_open_day(calendar, site_id, local.date_naive());
    local_to_utc(zone, open, local.time()).unwrap_or(candidate)
}

/// `date` + `time` in `zone`, as UTC. During a DST gap the next valid minute is used.
#[must_use]
pub fn local_to_utc(zone: Tz, date: NaiveDate, time: NaiveTime) -> Option<DateTime<Utc>> {
    let naive = date.and_time(time);
    match zone.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
        chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
        chrono::LocalResult::None => {
            let mut probe = naive;
            for _ in 0..180 {
                probe += Duration::minutes(1);
                if let Some(dt) = zone.from_local_datetime(&probe).earliest() {
                    return Some(dt.with_timezone(&Utc));
                }
            }
            None
        }
    }
}

/// Loads the library zone and an opening-calendar snapshot.
#[async_trait::async_trait]
pub trait OpeningDays: Send + Sync {
    async fn zone(&self) -> AppResult<Tz>;
    async fn opening_between(&self, from: NaiveDate, to: NaiveDate) -> AppResult<OpeningCalendar>;
}

/// Calendar that treats every day as open in [`default_zone`]. Used by unit tests.
pub struct AlwaysOpenCalendar;

#[async_trait::async_trait]
impl OpeningDays for AlwaysOpenCalendar {
    async fn zone(&self) -> AppResult<Tz> {
        Ok(default_zone())
    }

    async fn opening_between(
        &self,
        _from: NaiveDate,
        _to: NaiveDate,
    ) -> AppResult<OpeningCalendar> {
        Ok(OpeningCalendar::empty())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn paris() -> Tz {
        default_zone()
    }

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn expected_return_date_follows_paris_morning_not_utc() {
        // 2026-07-15 22:30 UTC is 2026-07-16 00:30 in Paris (CEST, UTC+2).
        // UTC is still on the 15th.
        let now = utc(2026, 7, 15, 22, 30);
        assert_eq!(
            now.date_naive(),
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()
        );
        assert_eq!(
            local_date(paris(), now),
            NaiveDate::from_ymd_opt(2026, 7, 16).unwrap()
        );

        // Due today in Paris, at 00:30, which is still 2026-07-15 in UTC.
        let due_today = paris()
            .with_ymd_and_hms(2026, 7, 16, 0, 30, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            due_today.date_naive(),
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()
        );
        assert_eq!(
            public_expected_return_date(paris(), due_today, now),
            Some(NaiveDate::from_ymd_opt(2026, 7, 16).unwrap())
        );

        // Due yesterday in Paris. That UTC date equals UTC "today", so a UTC
        // comparison would still show the date.
        let due_yesterday = paris()
            .with_ymd_and_hms(2026, 7, 15, 23, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            due_yesterday.date_naive(),
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap()
        );
        assert!(public_expected_return_date(paris(), due_yesterday, now).is_none());
    }

    #[test]
    fn summer_2230_utc_is_next_day_0030_in_paris() {
        let instant = utc(2026, 7, 15, 22, 30);
        assert_eq!(
            local_date(paris(), instant),
            NaiveDate::from_ymd_opt(2026, 7, 16).unwrap()
        );
        assert_eq!(format_patron_datetime(paris(), instant), "16/07/2026 00:30");
        assert_eq!(format_patron_date(paris(), instant), "16/07/2026");
    }

    #[test]
    fn dst_spring_forward_changes_the_paris_offset() {
        // 2026-03-29 01:00 UTC is the Europe/Paris spring-forward (02:00 CET → 03:00 CEST).
        let before = utc(2026, 3, 29, 0, 30);
        let after = utc(2026, 3, 29, 1, 30);
        assert_eq!(format_patron_datetime(paris(), before), "29/03/2026 01:30");
        assert_eq!(format_patron_datetime(paris(), after), "29/03/2026 03:30");
    }

    #[test]
    fn unknown_timezone_is_rejected() {
        let err = parse_timezone("Not/AZone").expect_err("invalid");
        match err {
            AppError::InvalidTimezone(msg) => assert!(msg.contains("Not/AZone"), "{msg}"),
            other => panic!("expected InvalidTimezone, got {other:?}"),
        }
        assert!(parse_timezone("  ").is_err());
        assert_eq!(parse_timezone(" Europe/Paris ").unwrap(), paris());
    }

    #[test]
    fn slide_keeps_local_clock_time() {
        let sunday = utc(2026, 9, 6, 15, 30); // 17:30 in Paris, still Sunday
        let calendar = OpeningCalendar::with_weekly(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            0..=5,
        );
        let slid = slide_due_date(sunday, paris(), Some(1), &calendar);
        assert_eq!(format_patron_datetime(paris(), slid), "07/09/2026 17:30");
    }
}
