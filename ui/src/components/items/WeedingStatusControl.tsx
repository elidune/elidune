import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Badge, Button } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import type { ItemStaffView, ItemWeedingStatus } from '@/types';
import { canManageWeeding, canViewWeeding } from '@/types';
import { getApiErrorMessage, getHttpStatus } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { ITEM_WEEDING_STATUSES, itemWeedingStatus } from '@/utils/weeding';
import WithdrawalDialog from './WithdrawalDialog';
import WithdrawalRecord from './WithdrawalRecord';

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
  const { t } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canWrite = canManageWeeding(user, token);
  const canRead = canViewWeeding(user, token);
  const current = itemWeedingStatus(item);
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  const withdrawalQuery = useQuery({
    queryKey: ['item-withdrawal', item.id],
    queryFn: () => api.getActiveItemWithdrawal(item.id!),
    enabled: current === 'withdrawn' && item.id != null && canRead,
    retry: (count, err) => getHttpStatus(err) !== 405 && count < 1,
  });

  const withdrawal = withdrawalQuery.data ?? null;

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

  const shelfActions = ITEM_WEEDING_STATUSES.filter(
    (status): status is Exclude<ItemWeedingStatus, 'withdrawn'> =>
      status !== 'withdrawn' && status !== current,
  );

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

      {current === 'withdrawn' && withdrawalQuery.isLoading ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('common.loading')}</p>
      ) : null}

      {current === 'withdrawn' && withdrawal ? (
        <WithdrawalRecord withdrawal={withdrawal} onChanged={refresh} />
      ) : null}

      {current === 'withdrawn' && !withdrawal && item.weedingReason?.trim() ? (
        <p className="text-xs text-gray-600 dark:text-gray-300">
          {t('weeding.reason')}: {item.weedingReason}
        </p>
      ) : null}

      {current !== 'withdrawn' ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.hint')}</p>
      ) : null}

      {withdrawalQuery.isError ? (
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
              onClick={() => setDialogOpen(true)}
            >
              {t('weeding.actionWithdrawn')}
            </Button>
          </div>
        </>
      ) : null}

      {error ? (
        <p role="alert" className="text-xs text-red-600 dark:text-red-400">
          {error}
        </p>
      ) : null}

      {item.id != null ? (
        <WithdrawalDialog
          open={dialogOpen}
          mode="create"
          itemIds={[item.id]}
          onClose={() => setDialogOpen(false)}
          onSuccess={refresh}
        />
      ) : null}
    </div>
  );
}
