import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Badge, Button, Card, ListResultCountBar, QueryErrorBanner, ResponsiveRecordList, Table } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import { canManageItems, type DepositLoss, type DepositLossKind } from '@/types';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { formatReplacementRateCents } from '@/utils/deposits';
import { formatLocaleDate } from '@/utils/localeDate';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const LOSSES_KEY = ['deposit-losses'] as const;

function todayIsoDate(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${now.getFullYear()}-${month}-${day}`;
}

function kindLabel(kind: DepositLossKind, t: (key: string) => string): string {
  return kind === 'damaged' ? t('deposits.losses.kindDamaged') : t('deposits.losses.kindLost');
}

export function DepositLossesPanel() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageItems(user, api.getToken());
  const [notified, setNotified] = useState(false);
  const [lenderId, setLenderId] = useState('');
  const [dates, setDates] = useState<Record<string, string>>({});

  const query = useQuery({
    queryKey: [...LOSSES_KEY, notified, lenderId],
    queryFn: () => api.listDepositLosses({
      notified,
      ...(lenderId ? { lenderId } : {}),
    }),
  });
  const lendersQuery = useQuery({
    queryKey: ['lenders', 'names'],
    queryFn: () => api.listLenders(true),
  });

  const notifyMutation = useMutation({
    mutationFn: (input: { itemId: string; notifiedOn: string }) =>
      api.markDepositLossNotified(input.itemId, input.notifiedOn),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: LOSSES_KEY });
      showToast({ variant: 'success', message: t('deposits.losses.saved') });
    },
    onError: (error: unknown) => {
      if (getApiErrorCode(error) === 'deposit_loss_already_notified') {
        void queryClient.invalidateQueries({ queryKey: LOSSES_KEY });
      }
      showToast({ variant: 'error', message: getApiErrorMessage(error, t) });
    },
  });

  const rows = query.data ?? [];
  const emptyMessage = notified ? t('deposits.losses.emptyNotified') : t('deposits.losses.empty');

  const notifyControl = (row: DepositLoss) => {
    if (row.depositLossNotifiedAt || notified) {
      const iso = row.depositLossNotifiedAt;
      return iso
        ? t('deposits.losses.notifiedOn', { date: formatLocaleDate(iso, i18n.language) })
        : '—';
    }
    if (!canWrite) return '—';
    const value = dates[row.itemId] || todayIsoDate();
    const pending = notifyMutation.isPending && notifyMutation.variables?.itemId === row.itemId;
    return (
      <div className="flex flex-wrap items-center justify-end gap-2" data-row-action>
        <input
          type="date"
          aria-label={t('deposits.losses.notifyDate')}
          className={formControlClass({ className: 'w-auto' })}
          value={value}
          onChange={(event) => setDates((current) => ({ ...current, [row.itemId]: event.target.value }))}
        />
        <Button
          type="button"
          size="sm"
          className="min-h-[2.75rem]"
          isLoading={pending}
          disabled={!value || notifyMutation.isPending}
          onClick={() => notifyMutation.mutate({ itemId: row.itemId, notifiedOn: value })}
        >
          {t('deposits.losses.notify')}
        </Button>
      </div>
    );
  };

  const columns = [
    {
      key: 'barcode',
      header: t('items.barcode'),
      render: (row: DepositLoss) => (
        <span className="font-mono text-gray-900 dark:text-white">{row.barcode || '—'}</span>
      ),
    },
    {
      key: 'title',
      header: t('deposits.losses.titleColumn'),
      render: (row: DepositLoss) => row.title || '—',
    },
    {
      key: 'lender',
      header: t('deposits.lender'),
      render: (row: DepositLoss) => (
        <div>
          <p className="font-medium text-gray-900 dark:text-white">{row.lenderName}</p>
          <Link
            to={`/deposits/${row.batchId}`}
            className="text-sm text-blue-700 underline dark:text-blue-300"
            data-row-action
          >
            {row.reference}
          </Link>
        </div>
      ),
    },
    {
      key: 'kind',
      header: t('deposits.losses.kind'),
      render: (row: DepositLoss) => (
        <Badge variant={row.kind === 'damaged' ? 'warning' : 'danger'} size="sm">
          {kindLabel(row.kind, t)}
        </Badge>
      ),
    },
    {
      key: 'amount',
      header: t('deposits.losses.amount'),
      render: (row: DepositLoss) =>
        formatReplacementRateCents(row.amountCents, i18n.language) ?? '—',
    },
    {
      key: 'notice',
      header: notified ? t('deposits.losses.notified') : '',
      align: 'right' as const,
      render: (row: DepositLoss) => notifyControl(row),
    },
  ];

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('deposits.losses.title')}</h2>
        <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
          {notified ? t('deposits.losses.subtitleNotified') : t('deposits.losses.subtitle')}
        </p>
      </div>

      <Card padding="none" className="overflow-hidden">
        <div className="grid gap-3 border-b border-gray-200 px-4 py-3 dark:border-gray-800 sm:grid-cols-2 sm:max-w-2xl">
          <div className="flex flex-col gap-1.5">
            <label htmlFor="deposit-loss-notified" className={formLabelClass({ marginBottom: false })}>
              {t('common.status')}
            </label>
            <select
              id="deposit-loss-notified"
              className={formControlClass()}
              value={notified ? 'true' : 'false'}
              onChange={(event) => setNotified(event.target.value === 'true')}
            >
              <option value="false">{t('deposits.losses.toNotify')}</option>
              <option value="true">{t('deposits.losses.notified')}</option>
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="deposit-loss-lender" className={formLabelClass({ marginBottom: false })}>
              {t('deposits.lender')}
            </label>
            <select
              id="deposit-loss-lender"
              className={formControlClass()}
              value={lenderId}
              onChange={(event) => setLenderId(event.target.value)}
            >
              <option value="">{t('deposits.filter.anyLender')}</option>
              {(lendersQuery.data ?? []).map((lender) => (
                <option key={lender.id} value={lender.id}>{lender.name}</option>
              ))}
            </select>
          </div>
        </div>
        {query.isError ? (
          <QueryErrorBanner message={getApiErrorMessage(query.error, t)} onRetry={() => void query.refetch()} />
        ) : null}
        {!query.isLoading || rows.length > 0 ? (
          <ListResultCountBar>{t('common.listCount', { count: rows.length })}</ListResultCountBar>
        ) : null}
        <ResponsiveRecordList
          desktop={
            <Table
              columns={columns}
              data={rows}
              keyExtractor={(row) => row.itemId}
              isLoading={query.isLoading}
              emptyMessage={emptyMessage}
            />
          }
          mobile={
            query.isLoading && rows.length === 0 ? (
              <Table columns={columns} data={[]} keyExtractor={(row) => row.itemId} isLoading emptyMessage={emptyMessage} />
            ) : rows.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                {emptyMessage}
              </div>
            ) : (
              <div className="mx-2 mb-2 overflow-hidden rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-900 sm:mx-4">
                {rows.map((row) => (
                  <div key={row.itemId} className="space-y-2 border-b border-gray-100 p-4 last:border-b-0 dark:border-gray-800">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <p className="font-mono font-medium text-gray-900 dark:text-white">{row.barcode || '—'}</p>
                      <Badge variant={row.kind === 'damaged' ? 'warning' : 'danger'} size="sm">
                        {kindLabel(row.kind, t)}
                      </Badge>
                    </div>
                    <p className="text-sm text-gray-900 dark:text-white">{row.title || '—'}</p>
                    <p className="text-sm text-gray-600 dark:text-gray-300">
                      {row.lenderName}
                      {' · '}
                      <Link to={`/deposits/${row.batchId}`} className="text-blue-700 underline dark:text-blue-300">
                        {row.reference}
                      </Link>
                    </p>
                    <p className="text-sm text-gray-600 dark:text-gray-300">
                      {t('deposits.losses.amount')}: {formatReplacementRateCents(row.amountCents, i18n.language) ?? '—'}
                    </p>
                    <div>{notifyControl(row)}</div>
                  </div>
                ))}
              </div>
            )
          }
        />
      </Card>
    </div>
  );
}
