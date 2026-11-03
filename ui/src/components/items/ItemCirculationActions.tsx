import { useState } from 'react';
import { SearchX, CircleAlert, SearchCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ConfirmDialog, Input } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { invalidateCirculationCaches } from '@/hooks/loans/invalidateCirculationCaches';
import api from '@/services/api';
import type { ItemStaffView, LostFoundDisposition } from '@/types';
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

type DialogKind = 'missing' | 'lost' | 'found' | null;

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
  const queryClient = useQueryClient();
  const canWrite = canManageLoans(user, api.getToken());
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [notes, setNotes] = useState('');
  const [bill, setBill] = useState(false);
  const [amount, setAmount] = useState('');
  const [foundDisposition, setFoundDisposition] = useState<LostFoundDisposition>('circulate');
  const [error, setError] = useState<string | null>(null);

  const closeDialog = () => {
    setDialog(null);
    setNotes('');
    setBill(false);
    setAmount('');
    setFoundDisposition('circulate');
    setError(null);
  };

  const mutation = useMutation({
    mutationFn: async () => {
      if (!item.id || !dialog) return;
      if (dialog === 'missing') {
        return api.markItemMissing(item.id, { notes: notes.trim() || undefined });
      }
      if (dialog === 'found') {
        return api.markItemFound(item.id, {
          notes: notes.trim() || undefined,
          disposition: foundDisposition,
        });
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
    onSuccess: async () => {
      closeDialog();
      await invalidateCirculationCaches(queryClient, undefined, { biblioId: item.biblioId });
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
  const isToSearch = code === CIRCULATION_STATUS.TO_SEARCH;
  const canDeclareMissing =
    code == null ||
    code === CIRCULATION_STATUS.AVAILABLE ||
    code === CIRCULATION_STATUS.TO_SEARCH;
  const canMarkLost = !isLost;
  const canMarkFound = isLost || isMissing || isToSearch;
  if (!canDeclareMissing && !canMarkLost && !canMarkFound) {
    return null;
  }

  const deposit = isDepositCopy(item.deposit);
  const onLoan = Boolean(item.borrowed || item.loanId);
  const showBillToggle = dialog === 'lost' && !deposit && onLoan;
  const showAmount = dialog === 'lost' && (deposit || (showBillToggle && bill));
  const showFoundDisposition = dialog === 'found' && isLost;

  const amountState = replacementAmountFieldState({
    kind: 'lost',
    bill: deposit || bill,
    amount,
    serverRequiresAmount: false,
  });

  const confirmDisabled = mutation.isPending || (dialog === 'lost' && amountState.invalid);

  const dialogTitle =
    dialog === 'missing'
      ? t('items.circulation.missingTitle')
      : dialog === 'found'
        ? t('items.circulation.foundTitle')
        : t('items.circulation.lostTitle');

  const dialogMessage =
    dialog === 'missing'
      ? t('items.circulation.missingMessage')
      : dialog === 'found'
        ? isLost
          ? t('items.circulation.foundLostMessage')
          : t('items.circulation.foundMissingMessage')
        : deposit
          ? t('items.circulation.lostDepositMessage')
          : onLoan
            ? t('items.circulation.lostOnLoanMessage')
            : t('items.circulation.lostShelfMessage');

  const confirmLabel =
    dialog === 'missing'
      ? t('items.circulation.declareMissing')
      : dialog === 'found'
        ? t('items.circulation.markFound')
        : t('items.circulation.markLost');

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
        {canMarkFound ? (
          <button
            type="button"
            disabled={mutation.isPending}
            onClick={() => {
              setError(null);
              setFoundDisposition('circulate');
              setDialog('found');
            }}
            className="inline-flex items-center gap-1.5 rounded-lg border border-emerald-200 px-3 py-1.5 text-sm text-emerald-800 hover:bg-emerald-50 dark:border-emerald-900/50 dark:text-emerald-200 dark:hover:bg-emerald-950/30"
          >
            <SearchCheck className="h-4 w-4" aria-hidden />
            {t('items.circulation.markFound')}
          </button>
        ) : null}
      </div>
      {isMissing ? (
        <p className="text-xs text-gray-600 dark:text-gray-400">{t('items.circulation.missingHint')}</p>
      ) : null}
      {isLost ? (
        <p className="text-xs text-gray-600 dark:text-gray-400">{t('items.circulation.lostHint')}</p>
      ) : null}

      <ConfirmDialog
        isOpen={dialog != null}
        onClose={closeDialog}
        onConfirm={() => mutation.mutate()}
        title={dialogTitle}
        message={dialogMessage}
        confirmLabel={confirmLabel}
        confirmVariant={dialog === 'lost' ? 'danger' : 'primary'}
        isLoading={mutation.isPending}
        confirmDisabled={confirmDisabled}
      >
        {error ? (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400" role="alert">
            {error}
          </p>
        ) : null}

        {showFoundDisposition ? (
          <fieldset className="mt-4 space-y-2">
            <legend className={formLabelClass()}>{t('items.circulation.foundDisposition')}</legend>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name="found-disposition"
                value="circulate"
                checked={foundDisposition === 'circulate'}
                onChange={() => setFoundDisposition('circulate')}
                disabled={mutation.isPending}
              />
              {t('loans.lostFound.circulate')}
            </label>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name="found-disposition"
                value="damaged"
                checked={foundDisposition === 'damaged'}
                onChange={() => setFoundDisposition('damaged')}
                disabled={mutation.isPending}
              />
              {t('loans.lostFound.damaged')}
            </label>
          </fieldset>
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
