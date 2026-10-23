import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { AlertCircle, Truck } from 'lucide-react';
import {
  Button,
  Card,
  CardHeader,
  Pagination,
  ResponsiveRecordList,
  ScrollableListRegion,
  ListSkeleton,
  Table,
  PageHeading,
  ListResultCountBar,
} from '@/components/common';
import TransitActionModals from '@/components/holds/TransitActionModals';
import {
  TransitBarcodeCell,
  TransitReaderCell,
  TransitTitleCell,
} from '@/components/holds/TransitQueueCells';
import TransitStatusBadge from '@/components/holds/TransitStatusBadge';
import { useActiveTransitsQuery } from '@/hooks/holds/useActiveTransitsQuery';
import { useSitesQuery } from '@/hooks/holds/useSitesQuery';
import { useTransitActions } from '@/hooks/holds/useTransitActions';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { formChoiceLabelClass } from '@/utils/formControl';
import {
  canCancelTransit,
  sourceLabel,
  transitListBarcode,
  transitListPatron,
  transitListTitle,
} from '@/utils/transitDisplay';
import type { ItemTransit, TransitStatus } from '@/types';

type TransitFilter = 'active' | TransitStatus | 'all';

export default function TransitsPage() {
  const { t, i18n } = useTranslation();
  const [filter, setFilter] = useState<TransitFilter>('active');
  const [page, setPage] = useState(1);
  const [prevFilter, setPrevFilter] = useState(filter);
  if (filter !== prevFilter) {
    setPrevFilter(filter);
    setPage(1);
  }

  const actions = useTransitActions();
  const sitesQuery = useSitesQuery();
  const sites = sitesQuery.data ?? [];

  const activeQuery = useActiveTransitsQuery(filter === 'active');
  const pagedQuery = useQuery({
    queryKey: ['transits', filter, page],
    queryFn: () =>
      api.listTransits({
        status: filter === 'all' || filter === 'active' ? undefined : filter,
        page,
        perPage: 50,
      }),
    enabled: filter !== 'active',
    staleTime: 30 * 1000,
  });

  const listError = filter === 'active' ? activeQuery.error : pagedQuery.error;
  const isLoading = filter === 'active' ? activeQuery.isLoading : pagedQuery.isLoading;
  const rows =
    filter === 'active'
      ? (activeQuery.data ?? []).slice().sort((a, b) => b.createdAt.localeCompare(a.createdAt))
      : (pagedQuery.data?.items ?? []);
  const totalPages =
    filter === 'active' ? 1 : Math.max(1, pagedQuery.data?.pageCount ?? 1);
  const listTotal =
    filter === 'active' ? rows.length : (pagedQuery.data?.total ?? rows.length);

  const rowActions = (r: ItemTransit) => (
    <>
      {r.status === 'requested' && (
        <Button
          size="sm"
          variant="secondary"
          disabled={actions.isLoading}
          isLoading={actions.busyKey === `ship:${r.id}`}
          onClick={() => actions.open({ kind: 'ship', transit: r })}
        >
          {t('holds.shipTransit')}
        </Button>
      )}
      {r.status === 'inTransit' && (
        <Button
          size="sm"
          variant="primary"
          disabled={actions.isLoading}
          isLoading={actions.busyKey === `receive:${r.id}`}
          onClick={() => actions.open({ kind: 'receive', transit: r })}
        >
          {t('holds.receiveTransit')}
        </Button>
      )}
      {canCancelTransit(r) && (
        <Button
          size="sm"
          variant="secondary"
          disabled={actions.isLoading}
          isLoading={actions.busyKey === `cancel:${r.id}`}
          onClick={() => actions.open({ kind: 'cancel', transit: r })}
        >
          {t('holds.cancelTransit')}
        </Button>
      )}
    </>
  );

  const columns = [
    {
      key: 'barcode',
      header: t('transits.columnBarcode'),
      render: (r: ItemTransit) => <TransitBarcodeCell barcode={transitListBarcode(r)} />,
    },
    {
      key: 'title',
      header: t('transits.columnTitle'),
      render: (r: ItemTransit) => <TransitTitleCell title={transitListTitle(r)} />,
    },
    {
      key: 'reader',
      header: t('transits.columnReader'),
      // `GET /transits` item `patron` (`listTransits` → `normalizePaginatedResponse`). No hold join.
      render: (r: ItemTransit) => <TransitReaderCell patron={transitListPatron(r)} />,
    },
    {
      key: 'from',
      header: t('transits.columnFrom'),
      render: (r: ItemTransit) => sourceLabel(sites, r.fromSiteId) ?? r.fromSiteId,
    },
    {
      key: 'to',
      header: t('transits.columnTo'),
      render: (r: ItemTransit) => sourceLabel(sites, r.toSiteId) ?? r.toSiteId,
    },
    {
      key: 'status',
      header: t('transits.columnStatus'),
      render: (r: ItemTransit) => <TransitStatusBadge status={r.status} />,
    },
    {
      key: 'created',
      header: t('transits.columnCreated'),
      render: (r: ItemTransit) => new Date(r.createdAt).toLocaleString(i18n.language),
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: (r: ItemTransit) => <div className="flex flex-wrap justify-end gap-2">{rowActions(r)}</div>,
    },
  ];

  const emptyMessage = filter === 'all' || filter === 'active' ? t('transits.noTransits') : t('transits.noMatching');

  const filters: { value: TransitFilter; key: string }[] = [
    { value: 'active', key: 'transits.filterActive' },
    { value: 'requested', key: 'transits.filterRequested' },
    { value: 'inTransit', key: 'transits.filterInTransit' },
    { value: 'all', key: 'transits.filterAll' },
  ];

  return (
    <div className="space-y-6">
      <PageHeading icon={Truck} title={t('transits.pageTitle')} subtitle={t('transits.subtitle')} />

      <Card padding="none" className="flex flex-col min-h-0">
        {listError && (
          <div className="mx-4 mt-4 rounded-lg border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3">
            <div className="flex items-start gap-2 flex-1 min-w-0 text-sm text-red-800 dark:text-red-200">
              <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" aria-hidden />
              <span>{getApiErrorMessage(listError, t)}</span>
            </div>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => void (filter === 'active' ? activeQuery.refetch() : pagedQuery.refetch())}
            >
              {t('common.retry')}
            </Button>
          </div>
        )}
        <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
          <CardHeader title={t('transits.queueTitle')} subtitle={t('transits.subtitle')} />
        </div>
        <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-800 flex flex-wrap gap-4 flex-shrink-0">
          <fieldset className="flex flex-wrap gap-4">
            <legend className="sr-only">{t('transits.filterLegend')}</legend>
            {filters.map(({ value, key }) => (
              <label key={value} className={formChoiceLabelClass()}>
                <input
                  type="radio"
                  name="transit-filter"
                  className="text-indigo-600"
                  checked={filter === value}
                  onChange={() => setFilter(value)}
                />
                {t(key)}
              </label>
            ))}
          </fieldset>
        </div>
        {!isLoading || rows.length > 0 ? (
          <ListResultCountBar>{t('common.listCount', { count: listTotal })}</ListResultCountBar>
        ) : null}
        <ScrollableListRegion aria-label={t('transits.queueTitle')}>
          {isLoading && rows.length === 0 ? (
            <ListSkeleton rows={8} />
          ) : (
            <ResponsiveRecordList
              desktop={
                <Table
                  columns={columns}
                  data={rows}
                  emptyMessage={emptyMessage}
                  keyExtractor={(r) => r.id}
                  isLoading={false}
                />
              }
              mobile={
                rows.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                    {emptyMessage}
                  </div>
                ) : (
                  <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                    {rows.map((r) => {
                      const reader = transitListPatron(r);
                      return (
                        <div
                          key={r.id}
                          className="p-4 border-b border-gray-100 dark:border-gray-800 last:border-b-0 space-y-2"
                        >
                          <div className="flex items-start justify-between gap-2">
                            <div className="min-w-0 space-y-1">
                              <TransitTitleCell title={transitListTitle(r)} />
                              <div className="text-xs text-gray-500 dark:text-gray-400">
                                {t('transits.columnBarcode')}: <TransitBarcodeCell barcode={transitListBarcode(r)} />
                              </div>
                              {reader ? (
                                <div className="text-xs text-gray-600 dark:text-gray-300">
                                  {t('transits.columnReader')}: <TransitReaderCell patron={reader} />
                                </div>
                              ) : null}
                            </div>
                            <TransitStatusBadge status={r.status} />
                          </div>
                          <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                            <div>
                              {t('transits.columnFrom')}: {sourceLabel(sites, r.fromSiteId) ?? r.fromSiteId}
                            </div>
                            <div>
                              {t('transits.columnTo')}: {sourceLabel(sites, r.toSiteId) ?? r.toSiteId}
                            </div>
                          </div>
                          <div className="flex flex-wrap gap-2">{rowActions(r)}</div>
                        </div>
                      );
                    })}
                  </div>
                )
              }
            />
          )}
        </ScrollableListRegion>
        {filter !== 'active' && (pagedQuery.data?.total ?? 0) > 0 && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-800 flex-shrink-0">
            <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
          </div>
        )}
      </Card>

      <TransitActionModals actions={actions} />
    </div>
  );
}
