import { useTranslation } from 'react-i18next';
import ConfirmDialog from '@/components/common/ConfirmDialog';
import { toPickPatronPhrase } from '@/utils/holdPatron';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  cardNumber?: string | null;
  initials?: string | null;
  onCancel: () => void;
  /** Check the copy in on the existing to_pick path (hold shelf or transit). */
  onSetAside: () => void;
  isLoading?: boolean;
}

/** Desk warning when the scanned copy is `to_pick` for someone else. */
export default function ToPickForceLoanDialog({
  isOpen,
  cardNumber,
  initials,
  onCancel,
  onSetAside,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const patron = toPickPatronPhrase({ cardNumber, initials }, t);

  return (
    <ConfirmDialog
      isOpen={isOpen}
      onClose={onCancel}
      onConfirm={onSetAside}
      title={t('loans.toPickForOtherTitle')}
      message={t('loans.toPickForOtherMessage', { patron })}
      confirmLabel={t('loans.toPickSetAside')}
      confirmVariant="primary"
      isLoading={isLoading}
      stackOnTop
    />
  );
}
