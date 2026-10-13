import { useTranslation } from 'react-i18next';

interface NoticeHoldsWaitingProps {
  /** Pending holds on the bibliographic notice. Absent on older servers. */
  holdsCount?: number | null;
}

/**
 * Notice-level pending hold count. Shown to staff, readers, and the public OPAC.
 * Hidden when the field is absent or zero. Never includes patron identity.
 */
export default function NoticeHoldsWaiting({ holdsCount }: NoticeHoldsWaitingProps) {
  const { t } = useTranslation();
  if (typeof holdsCount !== 'number' || !Number.isFinite(holdsCount) || holdsCount <= 0) {
    return null;
  }
  return (
    <p className="mt-2 text-sm text-amber-800 dark:text-amber-200" data-notice-holds="waiting">
      {t('holds.noticeWaiting', { count: holdsCount })}
    </p>
  );
}
