import { formatIsoDate } from '@/utils/serialSubscriptionDisplay';

/** Issue caption shown as the copy name. The cover date is added when the label does not already carry the year. */
export function serialCopyHeading(
  caption: { label?: string | null; date?: string | null } | null | undefined,
  locale: string,
): string | null {
  const label = caption?.label?.trim() ?? '';
  const rawDate = caption?.date?.slice(0, 10) ?? '';
  const formatted = /^\d{4}-\d{2}-\d{2}$/.test(rawDate) ? formatIsoDate(rawDate, locale) : '';
  if (!label) return formatted || null;
  const year = rawDate.slice(0, 4);
  if (!formatted || (year.length === 4 && label.includes(year))) return label;
  return `${label} · ${formatted}`;
}
