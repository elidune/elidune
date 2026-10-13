import { useTranslation } from 'react-i18next';
import ConfirmDialog from '@/components/common/ConfirmDialog';
import { toPickPatronPhrase } from '@/utils/holdPatron';

export interface ToPickForceLoanDialogProps {
  isOpen: boolean;
  /** Full name. Set only when holdFor is a users-read profile. */
  patronName?: string | null;
  cardNumber?: string | null;
  initials?: string | null;
  onCancel: () => void;
  onForce: () => void;
  isLoading?: boolean;
}

/** Desk warning when the scanned copy is `to_pick` for someone else. */
export default function ToPickForceLoanDialog({
  isOpen,
  patronName,
  cardNumber,
  initials,
  onCancel,
  onForce,
  isLoading = false,
}: ToPickForceLoanDialogProps) {
  const { t } = useTranslation();
  const named = patronName?.trim() ?? '';
  const patron = named || toPickPatronPhrase({ cardNumber, initials }, t);

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
