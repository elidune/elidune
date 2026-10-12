import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Badge, Button } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import type { ItemStaffView, ItemWeedingStatus, ItemWithdrawal } from '@/types';
import { canCancelWeeding, canManageWeeding, canViewWeeding } from '@/types';
import { getApiErrorMessage, getHttpStatus } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { ITEM_WEEDING_STATUSES, itemWeedingStatus } from '@/utils/weeding';
import {
  canCancelWithdrawal,
  canCompleteLegacyWithdrawal,
  withdrawalActorName,
  withdrawalAgentName,
} from '@/utils/withdrawals';
import WithdrawalDialog from './WithdrawalDialog';

interface WeedingStatusControlProps {
  item: ItemStaffView;
  onUpdated?: () => void;
}

const STATUS_VARIANT: Record<ItemWeedingStatus, 'success' | 'warning' | 'danger'> = {
  onShelf: 'success',
  candidate: 'warning',
  withdrawn: 'danger',
};

const SHELF_ACTION: Record<Exclude<ItemWeedingStatus, 'withdrawn'>, string> = {
  onShelf: 'weeding.actionOnShelf',
  candidate: 'weeding.actionCandidate',
};

export default function WeedingStatusControl({ item, onUpdated }: WeedingStatusControlProps) {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canWrite = canManageWeeding(user, token);
  const canRead = canViewWeeding(user, token);
  const canCancelRight = canCancelWeeding(user, token);
  const current = itemWeedingStatus(item);
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogMode, setDialogMode] = useState<'create' | 'complete'>('create');

  const withdrawalQuery = useQuery({
    queryKey: ['item-withdrawal', item.id],
    queryFn: () => api.getActiveItemWithdrawal(item.id!),
    enabled: current === 'withdrawn' && item.id != null && canRead,
    retry: (count, err) => getHttpStatus(err) !== 405 && count < 1,
  });

  const withdrawal = withdrawalQuery.data ?? null;
  const detailsUnavailable = withdrawalQuery.isError && getHttpStatus(withdrawalQuery.error) === 405;

  const refresh = () => {
    if (item.id != null) {
      void queryClient.invalidateQueries({ queryKey: ['item-withdrawal', item.id] });
    }
    void queryClient.invalidateQueries({ queryKey: ['item-withdrawals'] });
    onUpdated?.();
  };

  const statusMutation = useMutation({
    mutationFn: (status: Exclude<ItemWeedingStatus, 'withdrawn'>) => {
      if (item.id == null) return Promise.reject(new Error('missing item id'));
      return api.setItemWeeding(item.id, {
        status,
        reason: reason.trim() || undefined,
      });
    },
    onSuccess: () => {
      setReason('');
      setError(null);
      refresh();
    },
    onError: (err) => setError(getApiErrorMessage(err, t)),
  });

  const cancelMutation = useMutation({
    mutationFn: (row: ItemWithdrawal) => api.cancelItemWithdrawal(row.id),
    onSuccess: () => {
      setError(null);
      refresh();
    },
    onError: (err) => setError(getApiErrorMessage(err, t)),
  });

  const shelfActions = ITEM_WEEDING_STATUSES.filter(
    (status): status is Exclude<ItemWeedingStatus, 'withdrawn'> =>
      status !== 'withdrawn' && status !== current,
  );

  const disposed = !!withdrawal?.disposedAt;
  const cancelAllowed = canCancelRight && canCancelWithdrawal(withdrawal);
  const showComplete = canWrite && canCompleteLegacyWithdrawal(withdrawal);

  return (
    <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-600 space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          {t('weeding.label')}
        </p>
        <Badge variant={STATUS_VARIANT[current]} size="sm">
          {t(`weeding.status.${current}`)}
        </Badge>
      </div>

      {current === 'withdrawn' && withdrawal ? (
        <WithdrawalSummary withdrawal={withdrawal} language={i18n.language} />
      ) : null}

      {current === 'withdrawn' && !withdrawal && item.weedingReason?.trim() ? (
        <p className="text-xs text-gray-600 dark:text-gray-300">
          {t('weeding.reason')}: {item.weedingReason}
        </p>
      ) : null}

      {current !== 'withdrawn' ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.hint')}</p>
      ) : null}

      {detailsUnavailable ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.detailsUnavailable')}</p>
      ) : null}
      {withdrawalQuery.isError && !detailsUnavailable ? (
        <p role="alert" className="text-xs text-red-600 dark:text-red-400">
          {getApiErrorMessage(withdrawalQuery.error, t)}
        </p>
      ) : null}

      {canWrite && current !== 'withdrawn' ? (
        <>
          <div>
            <label htmlFor={`weeding-note-${item.id ?? 'new'}`} className={formLabelClass()}>
              {t('weeding.candidateNote')}
            </label>
            <input
              id={`weeding-note-${item.id ?? 'new'}`}
              className={formControlClass({ className: 'w-full' })}
              value={reason}
              maxLength={200}
              onChange={(e) => setReason(e.target.value.slice(0, 200))}
              placeholder={t('weeding.reasonPlaceholder')}
            />
          </div>
          <div className="flex flex-wrap gap-2">
            {shelfActions.map((status) => (
              <Button
                key={status}
                size="sm"
                variant="secondary"
                disabled={statusMutation.isPending}
                isLoading={statusMutation.isPending && statusMutation.variables === status}
                onClick={() => statusMutation.mutate(status)}
              >
                {t(SHELF_ACTION[status])}
              </Button>
            ))}
            <Button
              size="sm"
              variant="danger"
              disabled={item.id == null}
              onClick={() => {
                setDialogMode('create');
                setDialogOpen(true);
              }}
            >
              {t('weeding.actionWithdrawn')}
            </Button>
          </div>
        </>
      ) : null}

      {current === 'withdrawn' && withdrawal && (showComplete || canCancelRight) ? (
        <div className="flex flex-wrap gap-2">
          {showComplete ? (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                setDialogMode('complete');
                setDialogOpen(true);
              }}
            >
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
                if (withdrawal && cancelAllowed) cancelMutation.mutate(withdrawal);
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
        <p role="alert" className="text-xs text-red-600 dark:text-red-400">
          {error}
        </p>
      ) : null}

      {item.id != null ? (
        <WithdrawalDialog
          open={dialogOpen}
          mode={dialogMode}
          itemIds={[item.id]}
          withdrawalId={withdrawal?.id}
          initialReason={dialogMode === 'complete' ? (withdrawal?.reason ?? '') : ''}
          initialDetail={dialogMode === 'complete' ? (withdrawal?.reasonDetail ?? '') : ''}
          initialDestination={dialogMode === 'complete' ? (withdrawal?.destination ?? '') : ''}
          onClose={() => setDialogOpen(false)}
          onSuccess={refresh}
        />
      ) : null}
    </div>
  );
}

function WithdrawalSummary({
  withdrawal,
  language,
}: {
  withdrawal: ItemWithdrawal;
  language: string;
}) {
  const { t } = useTranslation();
  const destination = withdrawal.destination
    ? t(`weeding.destinations.${withdrawal.destination}`)
    : t('weeding.destinationUnset');
  const withdrawnBy = withdrawalAgentName(withdrawal);
  const disposedBy = withdrawalActorName(withdrawal);

  return (
    <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
      <p>
        {withdrawalStatusLine(t, 'withdrawn', formatWhen(withdrawal.withdrawnAt, language), withdrawnBy, destination)}
      </p>
      {withdrawal.disposedAt ? (
        <p>
          {withdrawalStatusLine(t, 'disposed', formatWhen(withdrawal.disposedAt, language), disposedBy, destination)}
        </p>
      ) : null}
      <p>
        {t('weeding.reason')}: {t(`weeding.reasons.${withdrawal.reason}`)}
        {withdrawal.reasonDetail?.trim() ? ` — ${withdrawal.reasonDetail.trim()}` : ''}
      </p>
    </div>
  );
}

function formatWhen(value: string | null | undefined, language: string): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(language);
}

function withdrawalStatusLine(
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
