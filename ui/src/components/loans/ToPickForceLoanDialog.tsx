import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import ConfirmDialog from '@/components/common/ConfirmDialog';
import CirculationOverrideReasonField from '@/components/loans/CirculationOverrideReasonField';
import { useOverrideReasonDraft } from '@/hooks/loans/useOverrideReasonDraft';
import { forceConfirmEnabled, trimmedOverrideReason } from '@/utils/circulationForce';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  /**
   * Refusal sentence. Callers pass the held-copy sentence.
   * Another reader's card, initials, and name are never accepted.
   */
  message?: string | null;
  onCancel: () => void;
  /** Check the copy in on the existing to_pick path (hold shelf or transit). */
  onSetAside: () => void;
  /** « Forcer le prêt » by default. Renewal passes « Forcer le renouvellement ». */
  onForce?: (overrideReason?: string) => void;
  forceLabel?: string;
  isLoading?: boolean;
  /** Server `reasonRequired`. The force button waits for a non-blank reason. */
  reasonRequired?: boolean;
  /** Server message for 422 `override_reason_required`. The dialog stays open. */
  error?: string | null;
}

/** Desk popup for `item_held_for_other` only. No other reader's identity. */
export default function ToPickForceLoanDialog({
  isOpen,
  message,
  onCancel,
  onSetAside,
  onForce,
  forceLabel,
  isLoading = false,
  reasonRequired = false,
  error = null,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const body = message?.trim() || t('errors.apiCode.item_held_for_other');
  const askReason = Boolean(onForce) && reasonRequired;
  const resetKey = isOpen && askReason ? body : '';
  const { reason, setReason, inputRef } = useOverrideReasonDraft(resetKey, isOpen && askReason, error);
  const forceEnabled = forceConfirmEnabled(askReason, reason);

  return (
    <ConfirmDialog
      isOpen={isOpen}
      onClose={onCancel}
      onConfirm={onSetAside}
      title={t('loans.toPickForOtherTitle')}
      message={body}
      confirmLabel={t('loans.toPickSetAside')}
      confirmVariant="primary"
      isLoading={isLoading}
      stackOnTop
      error={askReason ? null : error}
      middleFooter={
        onForce ? (
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              if (!forceEnabled) return;
              onForce(askReason ? trimmedOverrideReason(reason) : undefined);
            }}
            disabled={isLoading || !forceEnabled}
          >
            {forceLabel ?? t('loans.forceLoan')}
          </Button>
        ) : null
      }
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
