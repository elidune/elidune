import { useState } from 'react';
import { SearchX, CircleAlert } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { ConfirmDialog, Input } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import type { ItemStaffView } from '@/types';
import { canManageLoans } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { CIRCULATION_STATUS } from '@/utils/circulationStatus';
import { isDepositCopy, replacementAmountFieldState } from '@/utils/deposits';
import { formChoiceLabelClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import { isItemWithdrawn } from '@/utils/weeding';

interface ItemCirculationActionsProps {
  item: ItemStaffView;
  onUpdated?: () => void;
}

type DialogKind = 'missing' | 'lost' | null;

function circulationCode(item: ItemStaffView): number | null {
  const value = item.circulationStatus as number | string | null | undefined;
  if (typeof value === 'number') return value;
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (/^\d+$/.test(trimmed)) return Number(trimmed);
  }
  return null;
}

export default function ItemCirculationActions({ item, onUpdated }: ItemCirculationActionsProps) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const canWrite = canManageLoans(user, api.getToken());
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [notes, setNotes] = useState('');
  const [bill, setBill] = useState(false);
  const [amount, setAmount] = useState('');
  const [error, setError] = useState<string | null>(null);

  const closeDialog = () => {
    setDialog(null);
    setNotes('');
    setBill(false);
    setAmount('');
    setError(null);
  };

  const mutation = useMutation({
    mutationFn: async () => {
      if (!item.id || !dialog) return;
      if (dialog === 'missing') {
        return api.markItemMissing(item.id, { notes: notes.trim() || undefined });
      }
      const deposit = isDepositCopy(item.deposit);
      const onLoan = Boolean(item.borrowed || item.loanId);
      const payload = {
        notes: notes.trim() || undefined,
        bill: deposit ? undefined : onLoan ? bill : false,
        amount: amount.trim() || undefined,
        useItemPrice: !deposit && onLoan && bill ? true : undefined,
      };
      return api.markItemLost(item.id, payload);
    },
    onSuccess: () => {
      closeDialog();
      onUpdated?.();
    },
    onError: (err: unknown) => {
      setError(getApiErrorMessage(err, t) || t('items.circulation.error'));
    },
  });

  if (!canWrite || !item.id || isItemWithdrawn(item)) {
    return null;
  }

  const code = circulationCode(item);
  const isLost = code === CIRCULATION_STATUS.LOST;
  const isMissing = code === CIRCULATION_STATUS.MISSING;
  const canDeclareMissing =
    code == null ||
    code === CIRCULATION_STATUS.AVAILABLE ||
    code === CIRCULATION_STATUS.TO_SEARCH;
  const canMarkLost = !isLost;
  if (!canDeclareMissing && !canMarkLost) {
    return null;
  }

  const deposit = isDepositCopy(item.deposit);
  const onLoan = Boolean(item.borrowed || item.loanId);
  const showBillToggle = dialog === 'lost' && !deposit && onLoan;
  const showAmount = dialog === 'lost' && (deposit || (showBillToggle && bill));

  const amountState = replacementAmountFieldState({
    kind: 'lost',
    bill: deposit || bill,
    amount,
    serverRequiresAmount: false,
  });

  const confirmDisabled = mutation.isPending || (dialog === 'lost' && amountState.invalid);

  return (
    <div className="mt-3 space-y-2 border-t border-gray-200 pt-3 dark:border-gray-600">
      <p className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
        {t('items.circulation.section')}
      </p>
      <div className="flex flex-wrap gap-2">
        {canDeclareMissing ? (
          <button
            type="button"
            disabled={mutation.isPending}
            onClick={() => {
              setError(null);
              setDialog('missing');
            }}
            className="inline-flex items-center gap-1.5 rounded-lg border border-gray-200 px-3 py-1.5 text-sm text-gray-800 hover:bg-gray-100 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-700/50"
          >
            <SearchX className="h-4 w-4" aria-hidden />
            {t('items.circulation.declareMissing')}
          </button>
        ) : null}
        {canMarkLost ? (
          <button
            type="button"
            disabled={mutation.isPending}
            onClick={() => {
              setError(null);
              setDialog('lost');
            }}
            className="inline-flex items-center gap-1.5 rounded-lg border border-red-200 px-3 py-1.5 text-sm text-red-700 hover:bg-red-50 dark:border-red-900/50 dark:text-red-300 dark:hover:bg-red-950/30"
          >
            <CircleAlert className="h-4 w-4" aria-hidden />
            {t('items.circulation.markLost')}
          </button>
        ) : null}
      </div>
      {isMissing ? (
        <p className="text-xs text-gray-600 dark:text-gray-400">{t('items.circulation.missingHint')}</p>
      ) : null}

      <ConfirmDialog
        isOpen={dialog != null}
        onClose={closeDialog}
        onConfirm={() => mutation.mutate()}
        title={
          dialog === 'missing'
            ? t('items.circulation.missingTitle')
            : t('items.circulation.lostTitle')
        }
        message={
          dialog === 'missing'
            ? t('items.circulation.missingMessage')
            : deposit
              ? t('items.circulation.lostDepositMessage')
              : onLoan
                ? t('items.circulation.lostOnLoanMessage')
                : t('items.circulation.lostShelfMessage')
        }
        confirmLabel={
          dialog === 'missing'
            ? t('items.circulation.declareMissing')
            : t('items.circulation.markLost')
        }
        confirmVariant={dialog === 'lost' ? 'danger' : 'primary'}
        isLoading={mutation.isPending}
        confirmDisabled={confirmDisabled}
      >
        {error ? (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400" role="alert">
            {error}
          </p>
        ) : null}

        {showBillToggle ? (
          <label className={`${formChoiceLabelClass()} mt-4`}>
            <input
              type="checkbox"
              checked={bill}
              onChange={(e) => setBill(e.target.checked)}
              disabled={mutation.isPending}
            />
            {t('loans.exceptions.billReplacement')}
          </label>
        ) : null}

        {showAmount ? (
          <div className="mt-4 space-y-1">
            <label className={formLabelClass()} htmlFor={`item-lost-amount-${item.id}`}>
              {t('loans.exceptions.amount')}
            </label>
            <Input
              id={`item-lost-amount-${item.id}`}
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              disabled={mutation.isPending}
              inputMode="decimal"
              placeholder="0.00"
              aria-invalid={amountState.invalid || undefined}
            />
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {deposit
                ? t('items.circulation.depositAmountHint')
                : t('loans.exceptions.amountHintLost')}
            </p>
          </div>
        ) : null}

        <div className="mt-4 space-y-1">
          <label className={formLabelClass()} htmlFor={`item-circ-notes-${item.id}`}>
            {t('loans.exceptions.notes')}
          </label>
          <textarea
            id={`item-circ-notes-${item.id}`}
            className={formTextareaClass()}
            rows={2}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            disabled={mutation.isPending}
          />
        </div>
      </ConfirmDialog>
    </div>
  );
}
