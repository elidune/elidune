import { useTranslation } from 'react-i18next';
import ConfirmDialog from '@/components/common/ConfirmDialog';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  patronName: string | null;
  onCancel: () => void;
  onForce: () => void;
  isLoading?: boolean;
}

/** Desk warning when the scanned copy is `to_pick` for someone else. */
export default function ToPickForceLoanDialog({
  isOpen,
  patronName,
  onCancel,
  onForce,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const patron = patronName?.trim() || t('loans.pickPatronUnknown');

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
