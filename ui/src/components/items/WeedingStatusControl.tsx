import { useEffect, useId, useState } from 'react';
import { ChevronDown, Flag, PackageMinus, RotateCcw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Badge } from '@/components/common';
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

const SHELF_ACTION_DETAIL: Record<Exclude<ItemWeedingStatus, 'withdrawn'>, string> = {
  onShelf: 'weeding.actionOnShelfDetail',
  candidate: 'weeding.actionCandidateDetail',
};

function WeedingActionRow({
  title,
  description,
  icon,
  tone,
  disabled,
  isLoading,
  onClick,
}: {
  title: string;
  description: string;
  icon: React.ReactNode;
  tone: 'neutral' | 'danger';
  disabled?: boolean;
  isLoading?: boolean;
  onClick: () => void;
}) {
  const toneClass =
    tone === 'danger'
      ? 'border-red-200 dark:border-red-900/50 hover:bg-red-50 dark:hover:bg-red-950/30 focus-visible:ring-red-500 disabled:hover:bg-transparent dark:disabled:hover:bg-transparent'
      : 'border-gray-200 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700/50 focus-visible:ring-gray-500 disabled:hover:bg-transparent dark:disabled:hover:bg-transparent';
  const iconClass =
    tone === 'danger'
      ? 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300'
      : 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-200';

  return (
    <button
      type="button"
      disabled={disabled || isLoading}
      onClick={onClick}
      className={`w-full rounded-lg border p-3 text-left transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-gray-50 dark:focus-visible:ring-offset-gray-800/50 disabled:cursor-not-allowed disabled:opacity-50 ${toneClass}`}
    >
      <span className="flex items-start gap-3">
        <span
          className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${iconClass}`}
          aria-hidden
        >
          {icon}
        </span>
        <span className="min-w-0 space-y-0.5">
          <span className="block text-sm font-medium text-gray-900 dark:text-white">{title}</span>
          <span className="block text-xs leading-snug text-gray-600 dark:text-gray-400">{description}</span>
        </span>
      </span>
    </button>
  );
}

export default function WeedingStatusControl({ item, onUpdated }: WeedingStatusControlProps) {
  const { t } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canWrite = canManageWeeding(user, token);
  const canRead = canViewWeeding(user, token);
  const current = itemWeedingStatus(item);
  const panelId = useId();
  const triggerId = useId();
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [expanded, setExpanded] = useState(() => current !== 'onShelf');

  useEffect(() => {
    if (current === 'candidate' || current === 'withdrawn') {
      setExpanded(true);
    }
  }, [current]);

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

  if (!canRead && !canWrite) {
    return null;
  }

  return (
    <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-600">
      <div className="flex items-center gap-2 min-w-0">
        <button
          type="button"
          id={triggerId}
          aria-expanded={expanded}
          aria-controls={panelId}
          onClick={() => setExpanded((open) => !open)}
          className="flex min-w-0 flex-1 items-center justify-between gap-2 rounded-md text-left -m-1 p-1 hover:bg-gray-200/60 dark:hover:bg-gray-700/40"
          title={expanded ? t('common.collapseSection') : t('common.expandSection')}
        >
          <span className="flex min-w-0 flex-wrap items-center gap-2">
            <span className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
              {t('weeding.label')}
            </span>
            <Badge variant={STATUS_VARIANT[current]} size="sm">
              {t(`weeding.status.${current}`)}
            </Badge>
          </span>
          <ChevronDown
            className={`h-4 w-4 shrink-0 text-gray-500 transition-transform ${expanded ? 'rotate-180' : ''}`}
            aria-hidden
          />
        </button>
      </div>

      {expanded ? (
        <div id={panelId} role="region" aria-labelledby={triggerId} className="mt-2 space-y-3">
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

              <div className="space-y-2">
                {shelfActions.map((status) => (
                  <WeedingActionRow
                    key={status}
                    tone="neutral"
                    title={t(SHELF_ACTION[status])}
                    description={t(SHELF_ACTION_DETAIL[status])}
                    icon={
                      status === 'candidate' ? (
                        <Flag className="h-4 w-4" />
                      ) : (
                        <RotateCcw className="h-4 w-4" />
                      )
                    }
                    disabled={statusMutation.isPending}
                    isLoading={statusMutation.isPending && statusMutation.variables === status}
                    onClick={() => statusMutation.mutate(status)}
                  />
                ))}

                <div className="pt-1">
                  <p className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                    {t('weeding.withdrawSectionTitle')}
                  </p>
                  <WeedingActionRow
                    tone="danger"
                    title={t('weeding.actionWithdrawn')}
                    description={t('weeding.actionWithdrawDetail')}
                    icon={<PackageMinus className="h-4 w-4" />}
                    disabled={item.id == null}
                    onClick={() => setDialogOpen(true)}
                  />
                </div>
              </div>
            </>
          ) : null}

          {canWrite && current === 'withdrawn' ? null : !canWrite && current !== 'withdrawn' ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.hint')}</p>
          ) : null}

          {error ? (
            <p role="alert" className="text-xs text-red-600 dark:text-red-400">
              {error}
            </p>
          ) : null}
        </div>
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
