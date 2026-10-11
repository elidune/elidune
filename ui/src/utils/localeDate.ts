import type { TFunction } from 'i18next';

/** Locale calendar date (`11 octobre 2026`, `October 11, 2026`). */
export function formatLocaleDate(iso: string, language: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(language, { dateStyle: 'long' }).format(date);
}

/** Success copy after a renewal, including the new due date. */
export function renewedUntilMessage(
  expiryAt: string | null | undefined,
  t: TFunction,
  language: string,
): string {
  const iso = expiryAt?.trim();
  if (!iso) return t('loans.renewSuccess');
  return t('loans.renewedUntil', { date: formatLocaleDate(iso, language) });
}
