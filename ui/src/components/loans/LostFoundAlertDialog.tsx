import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import Modal from '@/components/common/Modal';
import Button from '@/components/common/Button';
import type { LostFoundDisposition, LostReturnFields } from '@/types';
import { lostFoundChoiceHint } from '@/utils/lostFound';

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
  variant: 'primary' | 'secondary';
}> = [
  {
    disposition: 'circulate',
    labelKey: 'loans.lostFound.circulate',
    variant: 'primary',
  },
  {
    disposition: 'damaged',
    labelKey: 'loans.lostFound.damaged',
    variant: 'secondary',
  },
  {
    disposition: 'leave',
    labelKey: 'loans.lostFound.leave',
    variant: 'secondary',
  },
];

const FIRST_CHOICE = CHOICES[0]?.disposition ?? 'circulate';

/**
 * Desk alert when a return reports `wasLost`.
 * Rendered as `role="dialog"` with `aria-modal` and `aria-labelledby` (the title).
 * The shared modal focus trap moves focus into the first action and restores it on close.
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
  const translate = (key: string, values?: Record<string, unknown>) => t(key, values);

  return (
    <Modal
      isOpen={isOpen}
      onClose={() => {
        if (!busy) onDismiss();
      }}
      title={t('loans.lostFound.title')}
      role="dialog"
      descriptionId={descriptionId}
      size="md"
    >
      <div id={descriptionId} className="space-y-3 text-sm text-gray-700 dark:text-gray-300">
        <p>{t('loans.lostFound.checkCondition')}</p>
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
                autoFocus={isOpen && choice.disposition === FIRST_CHOICE}
                isLoading={pending === choice.disposition}
                disabled={!canResolve || (busy && pending !== choice.disposition)}
                onClick={() => onResolve(choice.disposition)}
              >
                {t(choice.labelKey)}
              </Button>
              <p id={hintId} className="mt-1 text-sm text-gray-600 dark:text-gray-400">
                {lostFoundChoiceHint(choice.disposition, fee, i18n.language, translate)}
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
