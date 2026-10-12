import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import Modal from '@/components/common/Modal';
import Button from '@/components/common/Button';
import type { LostFoundDisposition, LostReturnFields } from '@/types';
import { lostFoundFeeMessage } from '@/utils/lostFound';

export interface LostFoundAlertDialogProps {
  isOpen: boolean;
  fee: LostReturnFields;
  canResolve: boolean;
  pending: LostFoundDisposition | null;
  error: string | null;
  onDismiss: () => void;
  onResolve: (disposition: LostFoundDisposition) => void;
}

const CHOICES: Array<{
  disposition: LostFoundDisposition;
  labelKey: string;
  hintKey: string;
  variant: 'primary' | 'secondary';
}> = [
  {
    disposition: 'circulate',
    labelKey: 'loans.lostFound.circulate',
    hintKey: 'loans.lostFound.circulateHint',
    variant: 'primary',
  },
  {
    disposition: 'damaged',
    labelKey: 'loans.lostFound.damaged',
    hintKey: 'loans.lostFound.damagedHint',
    variant: 'secondary',
  },
  {
    disposition: 'leave',
    labelKey: 'loans.lostFound.leave',
    hintKey: 'loans.lostFound.leaveHint',
    variant: 'secondary',
  },
];

/**
 * Desk alert when a return reports `wasLost`.
 * Escape, the close control, and the backdrop dismiss without calling the API:
 * the copy stays lost and fees are unchanged. Choosing "Laisser perdu" posts `leave`.
 */
export default function LostFoundAlertDialog({
  isOpen,
  fee,
  canResolve,
  pending,
  error,
  onDismiss,
  onResolve,
}: LostFoundAlertDialogProps) {
  const { t, i18n } = useTranslation();
  const descriptionId = useId();
  const busy = pending !== null;
  const feeText = lostFoundFeeMessage(fee, i18n.language, (key, values) => t(key, values));

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!busy) onDismiss();
      }}
      title={t('loans.lostFound.title')}
      role="alertdialog"
      descriptionId={descriptionId}
      size="md"
    >
      <div id={descriptionId} className="space-y-3 text-sm text-gray-700 dark:text-gray-300">
        <p>{t('loans.lostFound.checkCondition')}</p>
        <p>{feeText}</p>
        <p>{t('loans.lostFound.dismissHint')}</p>
      </div>
      <div className="mt-5 space-y-4" role="group" aria-label={t('loans.lostFound.checkCondition')}>
        {CHOICES.map((choice) => {
          const hintId = `${descriptionId}-${choice.disposition}`;
          return (
            <div key={choice.disposition}>
              <Button
                type="button"
                variant={choice.variant}
                className="w-full justify-start"
                aria-describedby={hintId}
                isLoading={pending === choice.disposition}
                disabled={!canResolve || (busy && pending !== choice.disposition)}
                onClick={() => onResolve(choice.disposition)}
              >
                {t(choice.labelKey)}
              </Button>
              <p id={hintId} className="mt-1 text-sm text-gray-600 dark:text-gray-400">
                {t(choice.hintKey)}
              </p>
            </div>
          );
        })}
      </div>
      {error ? (
        <p role="alert" className="mt-4 text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      ) : null}
    </Modal>
  );
}
