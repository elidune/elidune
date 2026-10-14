import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import ConfirmDialog from '@/components/common/ConfirmDialog';

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
  /** « Forcer le prêt ». Omitted when the refusal is not forceable. */
  onForce?: () => void;
  isLoading?: boolean;
}

/** Desk popup for `item_held_for_other` only. No other reader's identity. */
export default function ToPickForceLoanDialog({
  isOpen,
  message,
  onCancel,
  onSetAside,
  onForce,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const body = message?.trim() || t('errors.apiCode.item_held_for_other');

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
      middleFooter={
        onForce ? (
          <Button type="button" variant="secondary" onClick={onForce} disabled={isLoading}>
            {t('loans.forceLoan')}
          </Button>
        ) : null
      }
    />
  );
}
