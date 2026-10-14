import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import ConfirmDialog from '@/components/common/ConfirmDialog';
import { toPickPatronPhrase } from '@/utils/holdPatron';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  cardNumber?: string | null;
  initials?: string | null;
  /** Replaces the to-pick sentence. Used for `item_held_for_other`. */
  message?: string | null;
  onCancel: () => void;
  /** Check the copy in on the existing to_pick path (hold shelf or transit). */
  onSetAside: () => void;
  /** « Forcer le prêt ». Omitted when the refusal is not forceable. */
  onForce?: () => void;
  isLoading?: boolean;
}

/** Desk warning when the scanned copy is `to_pick` for someone else. */
export default function ToPickForceLoanDialog({
  isOpen,
  cardNumber,
  initials,
  message,
  onCancel,
  onSetAside,
  onForce,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const patron = toPickPatronPhrase({ cardNumber, initials }, t);
  const body = message?.trim() || t('loans.toPickForOtherMessage', { patron });

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
