import { useTranslation } from 'react-i18next';
import type { TransitPatron } from '@/types';
import { shelfPatronLabel } from '@/utils/holdPatron';

const EMPTY = '—';

export function TransitBarcodeCell({ barcode }: { barcode: string | null }) {
  return <span className="font-mono text-sm">{barcode?.trim() || EMPTY}</span>;
}

export function TransitTitleCell({ title }: { title: string | null }) {
  return <span className="font-medium text-gray-900 dark:text-white">{title?.trim() || EMPTY}</span>;
}

/**
 * Transit-list reader. Card number, otherwise initials.
 * A null patron is an em dash. A name is never shown.
 */
export function TransitReaderCell({ patron }: { patron: TransitPatron | null }) {
  const { t } = useTranslation();
  const card = patron?.cardNumber?.trim() || null;
  const initials = patron?.initials?.trim().replace(/\./g, '') ?? '';
  const safeInitials = initials && !/\s/u.test(initials) ? initials : '';
  if (!card && !safeInitials) return <span data-transit-reader="">{EMPTY}</span>;
  const label = shelfPatronLabel({ cardNumber: card, initials: safeInitials }, t);
  return <span data-transit-reader="">{label}</span>;
}
