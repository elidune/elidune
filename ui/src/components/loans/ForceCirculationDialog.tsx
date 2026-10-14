import ConfirmDialog from '@/components/common/ConfirmDialog';
import CirculationOverrideReasonField from '@/components/loans/CirculationOverrideReasonField';
import { useOverrideReasonDraft } from '@/hooks/loans/useOverrideReasonDraft';
import { forceConfirmEnabled, trimmedOverrideReason } from '@/utils/circulationForce';

export interface ForceCirculationDialogProps {
  isOpen: boolean;
  message: string;
  confirmLabel: string;
  /** Server `reasonRequired`. Absent or false keeps the dialog as a plain confirm. */
  reasonRequired?: boolean;
  /** Server message for 422 `override_reason_required`. The dialog stays open. */
  error?: string | null;
  isLoading?: boolean;
  onCancel: () => void;
  onConfirm: (overrideReason?: string) => void;
}

/**
 * Checkout « Forcer le prêt » and renewal « Forcer le renouvellement ».
 * A required reason disables confirm until the trimmed text is non-empty.
 */
export default function ForceCirculationDialog({
  isOpen,
  message,
  confirmLabel,
  reasonRequired = false,
  error = null,
  isLoading = false,
  onCancel,
  onConfirm,
}: ForceCirculationDialogProps) {
  const askReason = isOpen && reasonRequired;
  const resetKey = askReason ? message : '';
  const { reason, setReason, inputRef } = useOverrideReasonDraft(resetKey, askReason, error);
  const canConfirm = forceConfirmEnabled(askReason, reason);

  return (
    <ConfirmDialog
      isOpen={isOpen}
      onClose={onCancel}
      onConfirm={() => {
        if (!canConfirm) return;
        onConfirm(askReason ? trimmedOverrideReason(reason) : undefined);
      }}
      message={message}
      confirmLabel={confirmLabel}
      isLoading={isLoading}
      stackOnTop
      confirmDisabled={!canConfirm}
      error={askReason ? null : error}
    >
      {askReason ? (
        <CirculationOverrideReasonField
          value={reason}
          onChange={setReason}
          error={error}
          disabled={isLoading}
          inputRef={inputRef}
        />
      ) : null}
    </ConfirmDialog>
  );
}
