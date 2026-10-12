import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import Button from '@/components/common/Button';
import { deskScanAlertMessage, type DeskScanAlert } from '@/utils/deskScanAlert';

interface DeskScanNoticeProps {
  alert: DeskScanAlert;
  /** Checkout only. Runs the loan with `force` and does not open a dialog. */
  onForce?: () => void;
  forceDisabled?: boolean;
}

/**
 * Inline notice for a scanned copy that is on loan to another reader or on hold.
 * `role="status"` keeps it out of the scan field’s focus so the next barcode can be entered.
 */
export default function DeskScanNotice({ alert, onForce, forceDisabled = false }: DeskScanNoticeProps) {
  const { t } = useTranslation();
  return (
    <div
      role="status"
      className="mt-3 flex flex-col gap-2 rounded-lg border border-amber-200 bg-amber-50 p-3 text-amber-950 dark:border-amber-900/60 dark:bg-amber-950/30 dark:text-amber-100 sm:flex-row sm:items-center"
    >
      <div className="flex min-w-0 flex-1 items-start gap-2">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />
        <p className="text-sm font-medium">{deskScanAlertMessage(alert, (key, values) => t(key, values))}</p>
      </div>
      {onForce ? (
        <Button type="button" size="sm" variant="secondary" onClick={onForce} disabled={forceDisabled}>
          {t('loans.scanAlertForce')}
        </Button>
      ) : null}
    </div>
  );
}
