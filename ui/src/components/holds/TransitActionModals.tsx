import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '@/components/common';
import StartTransitDialog from '@/components/holds/StartTransitDialog';
import type { useTransitActions } from '@/hooks/holds/useTransitActions';

type TransitActions = ReturnType<typeof useTransitActions>;

export default function TransitActionModals({ actions }: { actions: TransitActions }) {
  const { t } = useTranslation();
  const { pending, isLoading, error, close, confirmStart, confirmShip, confirmReceive, confirmCancel } =
    actions;

  return (
    <>
      <StartTransitDialog
        hold={pending?.kind === 'start' ? pending.hold : null}
        isOpen={pending?.kind === 'start'}
        isLoading={isLoading}
        error={error}
        onClose={close}
        onConfirm={confirmStart}
      />
      <ConfirmDialog
        isOpen={pending?.kind === 'ship'}
        onClose={close}
        onConfirm={() => void confirmShip()}
        title={t('holds.shipTransit')}
        message={t('holds.shipConfirm')}
        confirmLabel={t('holds.shipTransit')}
        isLoading={isLoading}
        error={error}
      />
      <ConfirmDialog
        isOpen={pending?.kind === 'receive'}
        onClose={close}
        onConfirm={() => void confirmReceive()}
        title={t('holds.receiveTransit')}
        message={t('holds.receiveConfirm')}
        confirmLabel={t('holds.receiveTransit')}
        isLoading={isLoading}
        error={error}
      />
      <ConfirmDialog
        isOpen={pending?.kind === 'cancel'}
        onClose={close}
        onConfirm={() => void confirmCancel()}
        title={t('holds.cancelTransit')}
        message={t('holds.cancelTransitConfirm')}
        confirmLabel={t('holds.cancelTransit')}
        confirmVariant="danger"
        isLoading={isLoading}
        error={error}
      />
    </>
  );
}
