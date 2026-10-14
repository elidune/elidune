//! Age bands on `public_types` and the single rule that picks a reader's public type.
//!
//! Used by reader create/update and by the daily user-cleanup task. Do not copy this logic.

use chrono::{Datelike, NaiveDate};

/// One `public_types` row as an age band.
///
/// A row with both `age_min` and `age_max` null is not an age band (for example
/// `school` and `staff`). It never matches an age and it does not overlap other bands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicTypeAgeBand {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
}

impl PublicTypeAgeBand {
    #[must_use]
    pub fn is_age_band(&self) -> bool {
        self.age_min.is_some() || self.age_max.is_some()
    }

    /// Inclusive range text for error messages (`18-99`, `18+`, `<=12`).
    #[must_use]
    pub fn range_label(&self) -> String {
        match (self.age_min, self.age_max) {
            (Some(min), Some(max)) => format!("{min}-{max}"),
            (Some(min), None) => format!("{min}+"),
            (None, Some(max)) => format!("<={max}"),
            (None, None) => "unbounded".to_string(),
        }
    }
}

/// Completed years on `on`. The anniversary day counts as the new age.
#[must_use]
pub fn age_years_on_date(birth: NaiveDate, on: NaiveDate) -> i32 {
    let mut years = on.year() - birth.year();
    if on.month() < birth.month() || (on.month() == birth.month() && on.day() < birth.day()) {
        years -= 1;
    }
    years
}

/// `age_min` and `age_max` are both inclusive. A missing bound does not constrain that side.
#[must_use]
pub fn age_matches_public_type_bounds(
    age: i32,
    age_min: Option<i16>,
    age_max: Option<i16>,
) -> bool {
    if let Some(m) = age_min {
        if age < i32::from(m) {
            return false;
        }
    }
    if let Some(m) = age_max {
        if age > i32::from(m) {
            return false;
        }
    }
    true
}

/// `true` when both bounds are set and `age_min` is greater than `age_max`.
#[must_use]
pub fn age_band_range_inverted(age_min: Option<i16>, age_max: Option<i16>) -> bool {
    matches!((age_min, age_max), (Some(min), Some(max)) if min > max)
}

/// Smallest inclusive span wins. Open-ended bands sort after closed ones. Equal width: lowest id.
fn public_type_age_rule_rank(band: &PublicTypeAgeBand) -> (i32, i64) {
    let width = match (band.age_min, band.age_max) {
        (Some(min), Some(max)) => i32::from(max) - i32::from(min),
        _ => i32::MAX / 2,
    };
    (width, band.id)
}

/// Public type for `age`.
///
/// Bands with no age bounds are ignored, so a gap is `None` instead of a silent
/// fallback to `school` or `staff`. When legacy rows overlap, the narrowest
/// matching band wins, then the lowest id.
#[must_use]
pub fn resolve_public_type_for_age(bands: &[PublicTypeAgeBand], age: i32) -> Option<i64> {
    bands
        .iter()
        .filter(|band| band.is_age_band())
        .filter(|band| age_matches_public_type_bounds(age, band.age_min, band.age_max))
        .min_by_key(|band| public_type_age_rule_rank(band))
        .map(|band| band.id)
}

/// Other age band that overlaps `candidate`, if any. Lowest id wins when several do.
///
/// Rows that are not age bands are skipped. `candidate` is compared with every
/// other band; overlaps that do not involve `candidate` are ignored so a legacy
/// conflict does not block an unrelated save.
#[must_use]
pub fn conflicting_age_band<'a>(
    candidate: &PublicTypeAgeBand,
    others: &'a [PublicTypeAgeBand],
) -> Option<&'a PublicTypeAgeBand> {
    if !candidate.is_age_band() || age_band_range_inverted(candidate.age_min, candidate.age_max) {
        return None;
    }
    let candidate_span = age_span(candidate.age_min, candidate.age_max)?;
    others
        .iter()
        .filter(|band| band.id != candidate.id && band.is_age_band())
        .filter(|band| !age_band_range_inverted(band.age_min, band.age_max))
        .filter(|band| {
            age_span(band.age_min, band.age_max)
                .is_some_and(|span| spans_overlap(candidate_span, span))
        })
        .min_by_key(|band| band.id)
}

fn age_span(age_min: Option<i16>, age_max: Option<i16>) -> Option<(i32, i32)> {
    match (age_min, age_max) {
        (None, None) => None,
        (Some(min), Some(max)) => Some((i32::from(min), i32::from(max))),
        (Some(min), None) => Some((i32::from(min), i32::MAX)),
        (None, Some(max)) => Some((i32::MIN, i32::from(max))),
    }
}

fn spans_overlap(left: (i32, i32), right: (i32, i32)) -> bool {
    left.0 <= right.1 && right.0 <= left.1
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{
        age_band_range_inverted, age_matches_public_type_bounds, age_years_on_date,
        conflicting_age_band, resolve_public_type_for_age, PublicTypeAgeBand,
    };

    fn band(id: i64, name: &str, age_min: Option<i16>, age_max: Option<i16>) -> PublicTypeAgeBand {
        PublicTypeAgeBand {
            id,
            name: name.to_string(),
            label: name.to_string(),
            age_min,
            age_max,
        }
    }

    fn rules_sample() -> Vec<PublicTypeAgeBand> {
        vec![
            band(1, "child", Some(0), Some(12)),
            band(2, "teen", Some(0), Some(17)),
            band(3, "adult", Some(18), None),
            band(4, "school", None, None),
        ]
    }

    #[test]
    fn picks_narrowest_matching_band_then_lowest_id() {
        let rules = rules_sample();
        assert_eq!(resolve_public_type_for_age(&rules, 10), Some(1));
        assert_eq!(resolve_public_type_for_age(&rules, 15), Some(2));
        assert_eq!(resolve_public_type_for_age(&rules, 30), Some(3));
    }

    #[test]
    fn unbounded_category_is_not_a_fallback() {
        let rules = rules_sample();
        assert_eq!(resolve_public_type_for_age(&rules, 13), Some(2));
        let only_school = vec![band(4, "school", None, None)];
        assert_eq!(resolve_public_type_for_age(&only_school, 40), None);
    }

    #[test]
    fn equal_width_uses_lowest_id() {
        let rules = vec![
            band(8, "b", Some(18), Some(30)),
            band(3, "a", Some(18), Some(30)),
        ];
        assert_eq!(resolve_public_type_for_age(&rules, 20), Some(3));
    }

    #[test]
    fn bounds_are_inclusive() {
        assert!(age_matches_public_type_bounds(18, Some(18), Some(64)));
        assert!(age_matches_public_type_bounds(64, Some(18), Some(64)));
        assert!(!age_matches_public_type_bounds(17, Some(18), Some(64)));
        assert!(!age_matches_public_type_bounds(65, Some(18), Some(64)));
    }

    #[test]
    fn birthday_on_the_reference_day_counts() {
        let birth = NaiveDate::from_ymd_opt(2010, 6, 15).unwrap();
        let before = NaiveDate::from_ymd_opt(2025, 6, 14).unwrap();
        assert_eq!(age_years_on_date(birth, before), 14);
        let on = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        assert_eq!(age_years_on_date(birth, on), 15);
    }

    #[test]
    fn overlap_names_the_other_band_and_ignores_unrelated_pairs() {
        let adult = band(1, "adult", Some(18), Some(99));
        let senior = band(2, "senior", Some(60), Some(99));
        let school = band(3, "school", None, None);
        let gap = band(4, "youth", Some(13), Some(17));
        let all = vec![adult.clone(), senior.clone(), school, gap.clone()];

        let conflict = conflicting_age_band(&gap, &all);
        assert!(conflict.is_none());

        let conflict = conflicting_age_band(&adult, &all).expect("adult overlaps senior");
        assert_eq!(conflict.name, "senior");

        let narrower = band(5, "slice", Some(70), Some(80));
        let with_slice = vec![adult.clone(), senior.clone(), narrower.clone()];
        let conflict = conflicting_age_band(&narrower, &with_slice).expect("slice overlaps");
        assert_eq!(conflict.id, 1, "lowest conflicting id wins");
    }

    #[test]
    fn adjacent_inclusive_bands_do_not_overlap() {
        let child = band(1, "child", Some(0), Some(12));
        let next = band(2, "youth", Some(13), Some(17));
        assert!(conflicting_age_band(&next, &[child, next.clone()]).is_none());
    }

    #[test]
    fn shared_boundary_is_an_overlap() {
        let child = band(1, "child", Some(0), Some(12));
        let next = band(2, "youth", Some(12), Some(17));
        assert_eq!(
            conflicting_age_band(&next, &[child, next.clone()]).map(|band| band.id),
            Some(1)
        );
    }

    #[test]
    fn inverted_range_is_detected() {
        assert!(age_band_range_inverted(Some(18), Some(12)));
        assert!(!age_band_range_inverted(Some(12), Some(18)));
        assert!(!age_band_range_inverted(Some(12), None));
    }
}
