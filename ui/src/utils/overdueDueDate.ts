/** Classes for the overdue detail facts row. The date must stay fully visible. */
export const OVERDUE_FACTS_CLASS =
  'flex flex-wrap items-center justify-start gap-x-2 gap-y-1 text-xs sm:justify-end sm:text-right';

export function formatOverdueDueDate(iso: string | null | undefined, locale: string): string | null {
  if (!iso?.trim()) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleDateString(locale, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}
