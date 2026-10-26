/**
 * Translation key for a seeded serial frequency.
 *
 * The database `description` stays the English Koha sample (`1/month`).
 * Screens look up this key from the cycle and fall back to `description`
 * when the cycle has no key. There is no frequency screen until MVP 4.
 */
const SERIAL_FREQUENCY_LABEL_KEYS: Record<string, string> = {
  '2/1/day': 'serials.frequencies.twiceDaily',
  '1/1/day': 'serials.frequencies.daily',
  '3/1/week': 'serials.frequencies.threePerWeek',
  '1/1/week': 'serials.frequencies.weekly',
  '1/2/week': 'serials.frequencies.fortnightly',
  '1/3/week': 'serials.frequencies.everyThreeWeeks',
  '1/1/month': 'serials.frequencies.monthly',
  '1/2/month': 'serials.frequencies.everyTwoMonths',
  '1/3/month': 'serials.frequencies.quarterly',
  '2/1/month': 'serials.frequencies.twiceMonthly',
  '1/6/month': 'serials.frequencies.semiannual',
  '1/1/year': 'serials.frequencies.annual',
  '1/2/year': 'serials.frequencies.everyTwoYears',
  irregular: 'serials.frequencies.irregular',
};

export function serialFrequencyLabelKey(
  unit: string | null | undefined,
  issuesPerUnit: number,
  unitsPerIssue: number,
): string | null {
  const code = unit == null ? 'irregular' : `${issuesPerUnit}/${unitsPerIssue}/${unit}`;
  return SERIAL_FREQUENCY_LABEL_KEYS[code] ?? null;
}
