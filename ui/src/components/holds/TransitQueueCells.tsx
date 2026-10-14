import { useTranslation } from 'react-i18next';
import type { TransitPatron } from '@/types';
import { transitPatronLabel } from '@/utils/transitDisplay';

const EMPTY = '—';

export function TransitBarcodeCell({ barcode }: { barcode: string | null }) {
  return <span className="font-mono text-sm">{barcode?.trim() || EMPTY}</span>;
}

export function TransitTitleCell({ title }: { title: string | null }) {
  return <span className="font-medium text-gray-900 dark:text-white">{title?.trim() || EMPTY}</span>;
}

/**
 * Transit-list reader. Card number with `patron.initials` beside it, as returned.
 * A null reader renders nothing. A name is never shown.
 */
export function TransitReaderCell({ patron }: { patron: TransitPatron | null }) {
  const { t } = useTranslation();
  const label = transitPatronLabel(patron, (number) => t('holds.readerCardNo', { number }));
  if (!label) return null;
  return <span data-transit-reader="">{label}</span>;
}
