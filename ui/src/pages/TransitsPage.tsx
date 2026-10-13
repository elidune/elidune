import { useMemo, useState } from 'react';
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
} from '@/components/common';
import HoldPickupCell from '@/components/holds/HoldPickupCell';
import HoldTransitActions from '@/components/holds/HoldTransitActions';
import TransitActionModals from '@/components/holds/TransitActionModals';
import {
  TransitBarcodeCell,
  TransitReaderCell,
  TransitTitleCell,
} from '@/components/holds/TransitQueueCells';
import TransitStatusBadge from '@/components/holds/TransitStatusBadge';
import { useActiveTransitsQuery } from '@/hooks/holds/useActiveTransitsQuery';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import { useTransitActions } from '@/hooks/holds/useTransitActions';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { formChoiceLabelClass } from '@/utils/formControl';
import { canReadUsers } from '@/types';
import { useAuth } from '@/contexts/AuthContext';
import { isHoldDetailsStaff } from '@/utils/catalogView';
import { STAFF_HOLDS_FETCH_CAP } from '@/utils/holdDisplay';
import {
  canCancelTransit,
  sourceLabel,
  transitListBarcode,
  transitListReader,
  transitListTitle,
} from '@/utils/transitDisplay';
import type { Hold, ItemTransit, TransitStatus } from '@/types';

type TransitFilter = 'active' | TransitStatus | 'all';

export default function TransitsPage() {
  const { t, i18n } = useTranslation();
  const { user: authUser } = useAuth();
  const canOpenReader = canReadUsers(authUser, api.getToken());
  const [filter, setFilter] = useState<TransitFilter>('active');
  const [page, setPage] = useState(1);
  const [prevFilter, setPrevFilter] = useState(filter);
  if (filter !== prevFilter) {
    setPrevFilter(filter);
    setPage(1);
  }

  const actions = useTransitActions();
  const sourcesQuery = useSourcesQuery();
  const sources = sourcesQuery.data ?? [];

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

  const holdsQuery = useQuery({
    queryKey: ['activeHolds'],
    queryFn: () => api.getHolds({ page: 1, perPage: STAFF_HOLDS_FETCH_CAP, activeOnly: true }),
    staleTime: 30 * 1000,
  });

  const holdById = useMemo(() => {
    const map = new Map<string, Hold>();
    for (const hold of holdsQuery.data?.items ?? []) {
      if (!isHoldDetailsStaff(hold)) continue;
      map.set(hold.id, hold);
    }
    return map;
  }, [holdsQuery.data?.items]);

  const listError = filter === 'active' ? activeQuery.error : pagedQuery.error;
  const isLoading = filter === 'active' ? activeQuery.isLoading : pagedQuery.isLoading;
  const rows =
    filter === 'active'
      ? (activeQuery.data ?? []).slice().sort((a, b) => b.createdAt.localeCompare(a.createdAt))
      : (pagedQuery.data?.items ?? []);
  const totalPages =
    filter === 'active' ? 1 : Math.max(1, pagedQuery.data?.pageCount ?? 1);

  const holdFor = (transit: ItemTransit) =>
    transit.holdId ? holdById.get(transit.holdId) : undefined;

  const columns = [
    {
      key: 'barcode',
      header: t('transits.columnBarcode'),
      render: (r: ItemTransit) => (
        <TransitBarcodeCell barcode={transitListBarcode(holdFor(r))} />
      ),
    },
    {
      key: 'title',
      header: t('transits.columnTitle'),
      render: (r: ItemTransit) => {
        const hold = holdFor(r);
        return <TransitTitleCell title={transitListTitle(hold)} biblioId={hold?.biblioId} />;
      },
    },
    {
      key: 'reader',
      header: t('transits.columnReader'),
      render: (r: ItemTransit) => (
        <TransitReaderCell reader={transitListReader(holdFor(r))} linked={canOpenReader} />
      ),
    },
    {
      key: 'from',
      header: t('transits.columnFrom'),
      render: (r: ItemTransit) => sourceLabel(sources, r.fromSourceId) ?? r.fromSourceId,
    },
    {
      key: 'to',
      header: t('transits.columnTo'),
      render: (r: ItemTransit) => sourceLabel(sources, r.toSourceId) ?? r.toSourceId,
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
      render: (r: ItemTransit) => {
        const hold = holdFor(r);
        return (
          <div className="flex flex-wrap justify-end gap-2">
            {hold && (
              <HoldTransitActions hold={hold} transit={r} actions={actions} sources={sources} />
            )}
            {!hold && r.status === 'requested' && (
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
            {!hold && r.status === 'inTransit' && (
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
          </div>
        );
      },
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
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
          <Truck className="h-7 w-7 text-amber-600" />
          {t('transits.pageTitle')}
        </h1>
        <p className="text-gray-500 dark:text-gray-400 mt-1">{t('transits.subtitle')}</p>
      </div>

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
                      const hold = holdFor(r);
                      return (
                        <div
                          key={r.id}
                          className="p-4 border-b border-gray-100 dark:border-gray-800 last:border-b-0 space-y-2"
                        >
                          <div className="flex items-start justify-between gap-2">
                            <div className="min-w-0 space-y-1">
                              <TransitTitleCell title={transitListTitle(hold)} biblioId={hold?.biblioId} />
                              <div className="text-xs text-gray-500 dark:text-gray-400">
                                {t('transits.columnBarcode')}: <TransitBarcodeCell barcode={transitListBarcode(hold)} />
                              </div>
                              <div className="text-xs text-gray-600 dark:text-gray-300">
                                {t('transits.columnReader')}:{' '}
                                <TransitReaderCell reader={transitListReader(hold)} linked={canOpenReader} />
                              </div>
                            </div>
                            <TransitStatusBadge status={r.status} />
                          </div>
                          <div className="text-xs text-gray-600 dark:text-gray-300 space-y-1">
                            <div>
                              {t('transits.columnFrom')}: {sourceLabel(sources, r.fromSourceId) ?? r.fromSourceId}
                            </div>
                            <div>
                              {t('transits.columnTo')}: {sourceLabel(sources, r.toSourceId) ?? r.toSourceId}
                            </div>
                            {hold && (
                              <HoldPickupCell hold={hold} sources={sources} transit={r} showTransit={false} />
                            )}
                          </div>
                          <div className="flex flex-wrap gap-2">
                            {hold && (
                              <HoldTransitActions hold={hold} transit={r} actions={actions} sources={sources} />
                            )}
                            {!hold && r.status === 'requested' && (
                              <Button
                                size="sm"
                                variant="secondary"
                                onClick={() => actions.open({ kind: 'ship', transit: r })}
                              >
                                {t('holds.shipTransit')}
                              </Button>
                            )}
                            {!hold && r.status === 'inTransit' && (
                              <Button
                                size="sm"
                                variant="primary"
                                onClick={() => actions.open({ kind: 'receive', transit: r })}
                              >
                                {t('holds.receiveTransit')}
                              </Button>
                            )}
                            {canCancelTransit(r) && (
                              <Button
                                size="sm"
                                variant="secondary"
                                onClick={() => actions.open({ kind: 'cancel', transit: r })}
                              >
                                {t('holds.cancelTransit')}
                              </Button>
                            )}
                          </div>
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

      <TransitActionModals actions={actions} sources={sources} />
    </div>
  );
}
