import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Printer, SearchX } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ConfirmDialog,
  QueryErrorBanner,
  Table,
} from '@/components/common';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { defaultPickupSiteId } from '@/utils/transitDisplay';
import type { HoldPickListRow } from '@/types';
import { useToast } from '@/contexts/ToastContext';

const ALL_SITES = '';

export default function PickListPage() {
  const { t, i18n } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const sourcesQuery = useSourcesQuery();
  const sources = sourcesQuery.data ?? [];
  const [siteOverride, setSiteOverride] = useState<string | null>(null);
  const [pending, setPending] = useState<HoldPickListRow | null>(null);
  const siteId =
    siteOverride ??
    (sourcesQuery.isFetched
      ? sources.length > 0
        ? defaultPickupSiteId(sources)
        : ALL_SITES
      : null);

  const pickQuery = useQuery({
    queryKey: ['pickList', siteId],
    queryFn: () => api.getHoldPickList(siteId || undefined),
    enabled: siteId !== null,
  });

  const notFoundMutation = useMutation({
    mutationFn: (holdId: string) => api.markPickNotFound(holdId),
    onSuccess: async () => {
      setPending(null);
      showToast({ variant: 'success', message: t('pickList.notFoundSuccess') });
      await queryClient.invalidateQueries({ queryKey: ['pickList'] });
    },
    onError: (error: unknown) => {
      showToast({
        variant: 'error',
        message: getApiErrorMessage(error, t) || t('pickList.notFoundError'),
      });
    },
  });

  const rows = pickQuery.data ?? [];
  const formatDue = (iso: string | null) => {
    if (!iso) return t('pickList.noDue');
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return t('pickList.noDue');
    return date.toLocaleString(i18n.language);
  };

  const columns = [
    {
      key: 'barcode',
      header: t('pickList.columnBarcode'),
      render: (row: HoldPickListRow) => (
        <span className="font-mono text-sm">{row.barcode || '—'}</span>
      ),
    },
    {
      key: 'callNumber',
      header: t('pickList.columnCallNumber'),
      render: (row: HoldPickListRow) => (
        <span className="font-mono text-sm">{row.callNumber || '—'}</span>
      ),
    },
    {
      key: 'title',
      header: t('pickList.columnTitle'),
      render: (row: HoldPickListRow) => (
        <span className="font-medium text-gray-900 dark:text-white">
          {row.title?.trim() || t('pickList.noTitle')}
        </span>
      ),
    },
    {
      key: 'patron',
      header: t('pickList.columnPatron'),
      render: (row: HoldPickListRow) => (
        <Link
          className="text-indigo-600 dark:text-indigo-400 hover:underline"
          to={`/users/${row.userId}`}
        >
          {row.userName}
        </Link>
      ),
    },
    {
      key: 'pickup',
      header: t('pickList.columnPickup'),
      render: (row: HoldPickListRow) => row.pickupSiteName?.trim() || '—',
    },
    {
      key: 'due',
      header: t('pickList.columnDue'),
      render: (row: HoldPickListRow) => (
        <div className="flex items-center gap-2">
          <span className="tabular-nums">{formatDue(row.pickDueAt)}</span>
          {row.overdue ? <Badge variant="danger">{t('pickList.overdue')}</Badge> : null}
        </div>
      ),
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      className: 'print:hidden',
      render: (row: HoldPickListRow) => (
        <Button
          size="sm"
          variant="secondary"
          className="print:hidden"
          leftIcon={<SearchX className="h-4 w-4" />}
          isLoading={notFoundMutation.isPending && notFoundMutation.variables === row.holdId}
          onClick={() => setPending(row)}
        >
          {t('pickList.notFound')}
        </Button>
      ),
    },
  ];

  return (
    <div className="space-y-6 print:space-y-3">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between print:hidden">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('pickList.pageTitle')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('pickList.subtitle')}</p>
        </div>
        <Button
          type="button"
          variant="secondary"
          leftIcon={<Printer className="h-4 w-4" />}
          onClick={() => window.print()}
        >
          {t('pickList.print')}
        </Button>
      </div>

      <h1 className="hidden print:block text-xl font-bold">{t('pickList.pageTitle')}</h1>

      <Card padding="none" className="overflow-hidden">
        <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800 space-y-4 print:border-0 print:p-0">
          <div className="print:hidden">
            <CardHeader
              title={t('pickList.pageTitle')}
              subtitle={pickQuery.isSuccess ? t('pickList.count', { count: rows.length }) : undefined}
            />
          </div>
          <div className="max-w-xs print:hidden">
            <label className={formLabelClass()} htmlFor="pick-list-site">
              {t('pickList.siteLabel')}
            </label>
            <select
              id="pick-list-site"
              className={formControlClass({ className: 'w-full' })}
              value={siteId ?? ''}
              disabled={sourcesQuery.isLoading || siteId === null}
              onChange={(event) => setSiteOverride(event.target.value)}
            >
              <option value={ALL_SITES}>{t('pickList.allSites')}</option>
              {sources.map((site) => (
                <option key={site.id} value={site.id}>
                  {site.name?.trim() || site.id}
                </option>
              ))}
            </select>
          </div>
        </div>

        {pickQuery.isError ? (
          <div className="p-4 print:hidden">
            <QueryErrorBanner
              message={getApiErrorMessage(pickQuery.error, t) || t('pickList.loadError')}
              onRetry={() => void pickQuery.refetch()}
            />
          </div>
        ) : (
          <Table
            columns={columns}
            data={rows}
            keyExtractor={(row) => row.holdId}
            isLoading={pickQuery.isLoading || siteId === null}
            emptyMessage={t('pickList.empty')}
          />
        )}
      </Card>

      <ConfirmDialog
        isOpen={pending !== null}
        onClose={() => {
          if (!notFoundMutation.isPending) setPending(null);
        }}
        onConfirm={() => {
          if (pending) notFoundMutation.mutate(pending.holdId);
        }}
        title={t('pickList.notFoundTitle')}
        message={t('pickList.notFoundConfirm', {
          title: pending?.title?.trim() || t('pickList.noTitle'),
          barcode: pending?.barcode || '—',
        })}
        confirmLabel={t('pickList.notFound')}
        confirmVariant="danger"
        isLoading={notFoundMutation.isPending}
      />
    </div>
  );
}
