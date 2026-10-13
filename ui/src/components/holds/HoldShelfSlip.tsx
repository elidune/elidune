import { useTranslation } from 'react-i18next';
import type { LoanBorrowerMinimized } from '@/types';
import { shelfPatronLabel } from '@/utils/holdPatron';

interface HoldShelfSlipProps {
  title: string | null;
  barcode: string | null;
  callNumber: string | null;
  holdFor: LoanBorrowerMinimized | null;
  pickupDeadline: string | null;
}

function formatDeadline(iso: string | null, locale: string, empty: string): string {
  if (!iso) return empty;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return empty;
  return date.toLocaleString(locale);
}

/**
 * Printed hold slip and open-shelf label.
 * Card number or initials, plus the pickup deadline. The name is never printed.
 */
export default function HoldShelfSlip({
  title,
  barcode,
  callNumber,
  holdFor,
  pickupDeadline,
}: HoldShelfSlipProps) {
  const { t, i18n } = useTranslation();
  const patron = shelfPatronLabel(holdFor, t);
  const deadline = formatDeadline(pickupDeadline, i18n.language, t('pickList.noDue'));
  const copy = [callNumber, barcode].filter((part) => part?.trim()).join(' · ');

  return (
    <article data-hold-slip="shelf" className="break-inside-avoid border border-black p-3 text-sm">
      <p className="font-semibold">{title?.trim() || t('pickList.noTitle')}</p>
      {copy ? <p className="font-mono">{copy}</p> : null}
      <p data-hold-slip-patron="">{patron}</p>
      <p data-hold-slip-deadline="">
        {t('pickList.columnPickupDeadline')}: {deadline}
      </p>
    </article>
  );
}

/** Patron cell on the pick list. Same identity rule as the printed slip. */
export function HoldShelfPatron({ holdFor }: { holdFor: LoanBorrowerMinimized | null }) {
  const { t } = useTranslation();
  return <span data-hold-patron="shelf">{shelfPatronLabel(holdFor, t)}</span>;
}
