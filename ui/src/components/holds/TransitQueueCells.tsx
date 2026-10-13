import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import type { LoanBorrowerMinimized } from '@/types';
import { shelfPatronLabel } from '@/utils/holdPatron';
import { patronId } from '@/utils/patronLink';

const EMPTY = '—';

export function TransitBarcodeCell({ barcode }: { barcode: string | null }) {
  return <span className="font-mono text-sm">{barcode?.trim() || EMPTY}</span>;
}

export function TransitTitleCell({
  title,
  biblioId,
}: {
  title: string | null;
  biblioId?: string | null;
}) {
  const text = title?.trim() || EMPTY;
  if (!biblioId || !title?.trim()) {
    return <span className="font-medium text-gray-900 dark:text-white">{text}</span>;
  }
  return (
    <Link
      to={`/biblios/${biblioId}`}
      className="font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
    >
      {text}
    </Link>
  );
}

/**
 * Transit-list reader. Card number, otherwise initials. The name is never shown.
 */
export function TransitReaderCell({
  reader,
  linked = false,
}: {
  reader: LoanBorrowerMinimized | null;
  linked?: boolean;
}) {
  const { t } = useTranslation();
  const label = shelfPatronLabel(reader, t);
  const id = linked ? patronId(reader?.id) : null;
  if (!id) return <span data-transit-reader="">{label}</span>;
  return (
    <Link
      to={`/users/${id}`}
      data-transit-reader=""
      className="text-indigo-600 dark:text-indigo-400 hover:underline"
    >
      {label}
    </Link>
  );
}
