import { useTranslation } from 'react-i18next';
import ConfirmDialog from '@/components/common/ConfirmDialog';
import { toPickPatronPhrase } from '@/utils/holdPatron';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  cardNumber?: string | null;
  initials?: string | null;
  onCancel: () => void;
  onForce: () => void;
  isLoading?: boolean;
}

/** Desk warning when the scanned copy is `to_pick` for someone else. */
export default function ToPickForceLoanDialog({
  isOpen,
  cardNumber,
  initials,
  onCancel,
  onForce,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const patron = toPickPatronPhrase({ cardNumber, initials }, t);

  return (
    <ConfirmDialog
      isOpen={isOpen}
      onClose={onCancel}
      onConfirm={onForce}
      title={t('loans.toPickForOtherTitle')}
      message={t('loans.toPickForOtherMessage', { patron })}
      confirmLabel={t('loans.forceLoan')}
      confirmVariant="danger"
      isLoading={isLoading}
      stackOnTop
    />
  );
}
