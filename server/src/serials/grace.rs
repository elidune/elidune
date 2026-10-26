//! Default lateness grace for a serial subscription that does not set one.
//!
//! `grace_days` on the subscription overrides this, including an explicit 0.
//! An irregular frequency has no interval, so the default is absent and the
//! late job does not mark those issues on its own.

use crate::models::serial::{FrequencySpec, FrequencyUnit};

/// Days after `planned_date` before an expected issue may become `late`.
///
/// `None` for an irregular frequency or a frequency the engine would reject.
#[must_use]
pub fn default_grace_days(frequency: &FrequencySpec) -> Option<u32> {
    let unit = frequency.unit?;
    if frequency.issues_per_unit < 1 || frequency.units_per_issue < 1 {
        return None;
    }
    let (numerator, denominator) =
        interval_days(unit, frequency.issues_per_unit, frequency.units_per_issue)?;
    Some(grace_for_interval(numerator, denominator))
}

/// Approximate days between issues, as a ratio `numerator / denominator`.
fn interval_days(unit: FrequencyUnit, issues: i32, units: i32) -> Option<(i64, i64)> {
    let unit_days: i64 = match unit {
        FrequencyUnit::Day => 1,
        FrequencyUnit::Week => 7,
        FrequencyUnit::Month => 30,
        FrequencyUnit::Year => 365,
    };
    let numerator = i64::from(units).checked_mul(unit_days)?;
    Some((numerator, i64::from(issues)))
}

/// Bucket the interval. Thresholds are exclusive upper bounds on the ratio.
fn grace_for_interval(numerator: i64, denominator: i64) -> u32 {
    if numerator < denominator {
        return 1;
    }
    if numerator < 4 * denominator {
        return 2;
    }
    if numerator < 10 * denominator {
        return 3;
    }
    if numerator < 20 * denominator {
        return 7;
    }
    if numerator < 45 * denominator {
        return 15;
    }
    if numerator < 120 * denominator {
        return 30;
    }
    45
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frequency(unit: Option<FrequencyUnit>, issues: i32, units: i32) -> FrequencySpec {
        FrequencySpec {
            unit,
            issues_per_unit: issues,
            units_per_issue: units,
        }
    }

    #[test]
    fn grace_follows_the_publication_interval() {
        let cases = [
            (Some(FrequencyUnit::Day), 2, 1, Some(1)),
            (Some(FrequencyUnit::Day), 1, 1, Some(2)),
            (Some(FrequencyUnit::Week), 3, 1, Some(2)),
            (Some(FrequencyUnit::Week), 1, 1, Some(3)),
            (Some(FrequencyUnit::Week), 1, 2, Some(7)),
            (Some(FrequencyUnit::Month), 2, 1, Some(7)),
            (Some(FrequencyUnit::Week), 1, 3, Some(15)),
            (Some(FrequencyUnit::Month), 1, 1, Some(15)),
            (Some(FrequencyUnit::Month), 1, 2, Some(30)),
            (Some(FrequencyUnit::Month), 1, 3, Some(30)),
            (Some(FrequencyUnit::Month), 1, 6, Some(45)),
            (Some(FrequencyUnit::Year), 1, 1, Some(45)),
            (Some(FrequencyUnit::Year), 1, 2, Some(45)),
            (None, 1, 1, None),
        ];
        for (unit, issues, units, expected) in cases {
            assert_eq!(
                default_grace_days(&frequency(unit, issues, units)),
                expected,
                "{issues}/{units} {unit:?}"
            );
        }
    }
}
