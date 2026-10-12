import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Badge, Button, ConfirmDialog } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import type { ItemWithdrawal } from '@/types';
import { canCancelWeeding, canManageWeeding } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import {
  canCancelWithdrawal,
  canCompleteLegacyWithdrawal,
  withdrawalActorName,
  withdrawalAgentName,
} from '@/utils/withdrawals';
import WithdrawalDialog from './WithdrawalDialog';

interface WithdrawalRecordProps {
  withdrawal: ItemWithdrawal;
  onChanged?: () => void;
}

function formatWhen(value: string | null | undefined, language: string): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(language);
}

function statusLine(
  t: (key: string, options?: Record<string, string>) => string,
  kind: 'withdrawn' | 'disposed',
  date: string,
  name: string | null,
  destination: string,
): string {
  if (kind === 'withdrawn') {
    return name
      ? t('weeding.withdrawnOnBy', { date, name, destination })
      : t('weeding.withdrawnOn', { date, destination });
  }
  return name
    ? t('weeding.disposedOnBy', { date, name, destination })
    : t('weeding.disposedOn', { date, destination });
}

export default function WithdrawalRecord({ withdrawal, onChanged }: WithdrawalRecordProps) {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canWrite = canManageWeeding(user, token);
  const canCancelRight = canCancelWeeding(user, token);
  const [completeOpen, setCompleteOpen] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const disposed = !!withdrawal.disposedAt;
  const cancelAllowed = canCancelRight && canCancelWithdrawal(withdrawal);
  const showComplete = canWrite && canCompleteLegacyWithdrawal(withdrawal);
  const destination = withdrawal.destination
    ? t(`weeding.destinations.${withdrawal.destination}`)
    : t('weeding.destinationUnset');

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ['item-withdrawal', withdrawal.itemId] });
    void queryClient.invalidateQueries({ queryKey: ['item-withdrawal-by-id', withdrawal.id] });
    void queryClient.invalidateQueries({ queryKey: ['item-withdrawals'] });
    onChanged?.();
  };

  const cancelMutation = useMutation({
    mutationFn: () => api.cancelItemWithdrawal(withdrawal.id),
    onSuccess: () => {
      setError(null);
      setConfirmCancel(false);
      refresh();
    },
    onError: (err) => setError(getApiErrorMessage(err, t)),
  });

  return (
    <article className="space-y-3">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="text-base font-semibold text-gray-900 dark:text-white">
          {withdrawal.biblioId ? (
            <Link className="hover:underline" to={`/biblios/${withdrawal.biblioId}`}>
              {withdrawal.biblioTitle || t('items.noTitle')}
            </Link>
          ) : (
            withdrawal.biblioTitle || t('weeding.detailTitle')
          )}
        </h2>
        {withdrawal.legacy ? <Badge size="sm">{t('weeding.legacyBadge')}</Badge> : null}
        {withdrawal.cancelledAt ? <Badge size="sm" variant="default">{t('weeding.cancelled')}</Badge> : null}
      </div>

      <p className="text-sm text-gray-600 dark:text-gray-300">
        {withdrawal.barcode || t('items.noBarcode')}
        {withdrawal.callNumber ? ` · ${withdrawal.callNumber}` : ''}
        {withdrawal.siteName ? ` · ${withdrawal.siteName}` : ''}
        {' · '}
        {t(`weeding.reasons.${withdrawal.reason}`)}
        {withdrawal.reasonDetail?.trim() ? ` — ${withdrawal.reasonDetail.trim()}` : ''}
      </p>

      <dl className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-sm">
        <div>
          <dt className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {t('weeding.columnWithdrawnAt')}
          </dt>
          <dd className="text-gray-800 dark:text-gray-100">
            {statusLine(
              t,
              'withdrawn',
              formatWhen(withdrawal.withdrawnAt, i18n.language),
              withdrawalAgentName(withdrawal),
              destination,
            )}
          </dd>
        </div>
        <div>
          <dt className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {t('weeding.columnDisposedAt')}
          </dt>
          <dd className="text-gray-800 dark:text-gray-100">
            {withdrawal.disposedAt
              ? statusLine(
                  t,
                  'disposed',
                  formatWhen(withdrawal.disposedAt, i18n.language),
                  withdrawalActorName(withdrawal),
                  destination,
                )
              : '—'}
          </dd>
        </div>
      </dl>

      {showComplete ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.legacyHint')}</p>
      ) : null}

      {showComplete || canCancelRight ? (
        <div className="flex flex-wrap gap-2">
          {showComplete ? (
            <Button size="sm" variant="secondary" onClick={() => setCompleteOpen(true)}>
              {t('weeding.complete')}
            </Button>
          ) : null}
          {canCancelRight ? (
            <Button
              size="sm"
              variant="secondary"
              disabled={!cancelAllowed || cancelMutation.isPending}
              isLoading={cancelMutation.isPending}
              title={disposed ? t('weeding.cancelDisabledDisposed') : undefined}
              onClick={() => {
                if (!cancelAllowed) return;
                setError(null);
                setConfirmCancel(true);
              }}
            >
              {t('weeding.cancel')}
            </Button>
          ) : null}
        </div>
      ) : null}
      {canCancelRight && disposed ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.cancelDisabledDisposed')}</p>
      ) : null}
      {error ? (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400 whitespace-pre-wrap">
          {error}
        </p>
      ) : null}

      <ConfirmDialog
        isOpen={confirmCancel}
        onClose={() => {
          if (cancelMutation.isPending) return;
          setConfirmCancel(false);
        }}
        onConfirm={() => cancelMutation.mutate()}
        title={t('weeding.cancel')}
        message={t('weeding.cancelConfirm')}
        confirmLabel={t('weeding.cancel')}
        confirmVariant="danger"
        isLoading={cancelMutation.isPending}
        error={error}
      />

      <WithdrawalDialog
        open={completeOpen}
        mode="complete"
        itemIds={[]}
        withdrawalId={withdrawal.id}
        initialReason={withdrawal.reason}
        initialDetail={withdrawal.reasonDetail ?? ''}
        initialDestination={withdrawal.destination ?? ''}
        onClose={() => setCompleteOpen(false)}
        onSuccess={() => {
          setCompleteOpen(false);
          setError(null);
          refresh();
        }}
      />
    </article>
  );
}
