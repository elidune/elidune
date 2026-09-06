import { useTranslation } from 'react-i18next';
import type { Hold } from '@/types';

interface HoldExpiresCellProps {
  hold: Hold;
  /** Highlight pickup deadline for ready holds. */
  emphasizePickup?: boolean;
  /** Prefix with the expires label (mobile cards). */
  showLabel?: boolean;
}

export default function HoldExpiresCell({
  hold,
  emphasizePickup = false,
  showLabel = false,
}: HoldExpiresCellProps) {
  const { t, i18n } = useTranslation();
  const line = hold.expiresAt ? new Date(hold.expiresAt).toLocaleString(i18n.language) : '—';
  const readyPickup = emphasizePickup && hold.status === 'ready' && Boolean(hold.expiresAt);

  return (
    <div
      className={
        readyPickup
          ? 'rounded-md px-2 py-1.5 -mx-1 bg-amber-50 dark:bg-amber-900/25 border border-amber-200 dark:border-amber-800'
          : ''
      }
    >
      {showLabel && <span className="text-gray-500">{t('holds.expiresAt')}: </span>}
      <span className={readyPickup ? 'font-semibold text-amber-900 dark:text-amber-100' : ''}>{line}</span>
      {readyPickup && (
        <p className="text-xs mt-1 text-amber-800 dark:text-amber-200/90 leading-snug">
          {t('holds.pickupDeadlineHint')}
        </p>
      )}
    </div>
  );
}
