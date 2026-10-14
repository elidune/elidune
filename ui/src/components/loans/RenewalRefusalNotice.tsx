import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import ForceCirculationDialog from '@/components/loans/ForceCirculationDialog';
import ToPickForceLoanDialog from '@/components/loans/ToPickForceLoanDialog';
import type { RenewalDeskView } from '@/utils/renewalDesk';

export interface RenewalRefusalNoticeProps {
  view: RenewalDeskView | null;
  /** `alert` is the inline sentence. `dialog` is the force / set-aside popup. */
  part?: 'all' | 'alert' | 'dialog';
  onForce: (overrideReason?: string) => void;
  onDismiss: () => void;
  onSetAside: () => void;
  busy?: boolean;
}

/**
 * Staff renewal refusal. The force control is labeled « Forcer le renouvellement »
 * and is rendered only when `view` says the refusal is forceable.
 */
export default function RenewalRefusalNotice({
  view,
  part = 'all',
  onForce,
  onDismiss,
  onSetAside,
  busy = false,
}: RenewalRefusalNoticeProps) {
  const { t } = useTranslation();
  const alertRef = useRef<HTMLDivElement>(null);
  const sentence = view?.kind === 'sentence' ? view.message : null;
  const showAlert = (part === 'all' || part === 'alert') && sentence !== null;
  const showDialog = part === 'all' || part === 'dialog';

  useEffect(() => {
    if (!showAlert) return;
    alertRef.current?.focus();
  }, [showAlert, sentence]);

  if (!view) return null;

  return (
    <>
      {showAlert && (
        <div
          ref={alertRef}
          role="alert"
          tabIndex={-1}
          className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 flex items-start gap-2 outline-none focus-visible:ring-2 focus-visible:ring-red-500"
        >
          <AlertTriangle className="h-4 w-4 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" aria-hidden />
          <p className="text-sm font-medium text-red-800 dark:text-red-200">{sentence}</p>
        </div>
      )}
      {showDialog && view.kind === 'force' && (
        <ForceCirculationDialog
          isOpen
          message={view.message}
          confirmLabel={t('loans.forceRenew')}
          reasonRequired={view.reasonRequired}
          error={view.error}
          isLoading={busy}
          onCancel={onDismiss}
          onConfirm={onForce}
        />
      )}
      {showDialog && view.kind === 'setAside' && (
        <ToPickForceLoanDialog
          isOpen
          message={view.message}
          onCancel={onDismiss}
          onSetAside={onSetAside}
          onForce={view.force ? onForce : undefined}
          forceLabel={t('loans.forceRenew')}
          isLoading={busy}
          reasonRequired={view.reasonRequired}
          error={view.error}
        />
      )}
    </>
  );
}
