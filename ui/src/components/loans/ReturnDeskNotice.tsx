import { AlertTriangle, Info } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { returnNoticeMessage, type ReturnDeskNotice as ReturnDeskNoticeModel } from '@/utils/returnNotice';

interface ReturnDeskNoticeProps {
  notice: ReturnDeskNoticeModel;
  /** Shelf line when the scan did not ready a hold or start transit. */
  putBack?: boolean;
}

const INFO_CLASS =
  'flex items-start gap-3 rounded-lg border border-sky-200 bg-sky-50 p-4 text-sky-950 dark:border-sky-900 dark:bg-sky-950/40 dark:text-sky-100';
const WARNING_CLASS =
  'flex items-start gap-3 rounded-lg border border-amber-300 bg-amber-50 p-4 text-amber-950 dark:border-amber-700 dark:bg-amber-950/40 dark:text-amber-50';

/**
 * Barcode check-in note.
 * Informational is neutral. A warning is the blocked-status result and is not a success.
 */
export default function ReturnDeskNotice({ notice, putBack = false }: ReturnDeskNoticeProps) {
  const { t } = useTranslation();
  const warning = notice.level === 'warning';
  const message = returnNoticeMessage(notice, (key, values) => String(t(key, values)), { putBack });
  const Icon = warning ? AlertTriangle : Info;
  return (
    <div role={warning ? 'alert' : 'status'} data-return-notice={notice.level} className={warning ? WARNING_CLASS : INFO_CLASS}>
      <Icon
        className={`mt-0.5 h-5 w-5 shrink-0 ${warning ? 'text-amber-700 dark:text-amber-300' : 'text-sky-700 dark:text-sky-300'}`}
        aria-hidden
      />
      <p className={`text-sm ${warning ? 'font-semibold' : 'font-medium'}`}>{message}</p>
    </div>
  );
}
