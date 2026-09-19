import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Badge, Button, Input } from '@/components/common';
import api from '@/services/api';
import type { Item, ItemWeedingStatus } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { ITEM_WEEDING_STATUSES, itemWeedingStatus } from '@/utils/weeding';

interface WeedingStatusControlProps {
  item: Item;
  canManage: boolean;
  onUpdated?: () => void;
}

const STATUS_VARIANT: Record<ItemWeedingStatus, 'success' | 'warning' | 'danger'> = {
  onShelf: 'success',
  candidate: 'warning',
  withdrawn: 'danger',
};

const ACTION_KEY: Record<ItemWeedingStatus, string> = {
  onShelf: 'weeding.actionOnShelf',
  candidate: 'weeding.actionCandidate',
  withdrawn: 'weeding.actionWithdrawn',
};

export default function WeedingStatusControl({ item, canManage, onUpdated }: WeedingStatusControlProps) {
  const { t } = useTranslation();
  const current = itemWeedingStatus(item);
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: (status: ItemWeedingStatus) =>
      api.setItemWeeding(item.id, {
        status,
        reason: reason.trim() || undefined,
      }),
    onSuccess: () => {
      setReason('');
      setError(null);
      onUpdated?.();
    },
    onError: (err) => {
      setError(getApiErrorMessage(err, t));
    },
  });

  const alternatives = ITEM_WEEDING_STATUSES.filter((status) => status !== current);

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
      {item.weedingReason?.trim() ? (
        <p className="text-xs text-gray-600 dark:text-gray-300">
          {t('weeding.reason')}: {item.weedingReason}
        </p>
      ) : null}
      <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.hint')}</p>
      {canManage ? (
        <>
          <Input
            label={t('weeding.reason')}
            value={reason}
            maxLength={200}
            onChange={(e) => setReason(e.target.value.slice(0, 200))}
            placeholder={t('weeding.reasonPlaceholder')}
          />
          <div className="flex flex-wrap gap-2">
            {alternatives.map((status) => (
              <Button
                key={status}
                size="sm"
                variant={status === 'withdrawn' ? 'danger' : 'secondary'}
                disabled={mutation.isPending}
                isLoading={mutation.isPending && mutation.variables === status}
                onClick={() => mutation.mutate(status)}
              >
                {t(ACTION_KEY[status])}
              </Button>
            ))}
          </div>
          {error ? (
            <p role="alert" className="text-xs text-red-600 dark:text-red-400">
              {error}
            </p>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
