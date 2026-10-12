import { formatOverdueDueDate } from '@/utils/overdueDueDate';

export default function OverdueDueDate({
  iso,
  locale,
}: {
  iso: string | null | undefined;
  locale: string;
}) {
  const label = formatOverdueDueDate(iso, locale) ?? '—';
  return (
    <time
      dateTime={iso ?? undefined}
      className="whitespace-nowrap tabular-nums text-gray-600 dark:text-gray-300"
    >
      {label}
    </time>
  );
}
