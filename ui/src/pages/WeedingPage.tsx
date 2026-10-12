import { useMemo, useRef, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Download, Leaf } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ConfirmDialog,
  Pagination,
} from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import api from '@/services/api';
import { canManageWeeding } from '@/types';
import type { Collection, ItemWithdrawal } from '@/types';
import { disposeBatchErrorMessage, getApiErrorMessage } from '@/utils/apiError';
import WithdrawalDialog from '@/components/items/WithdrawalDialog';
import { formChoiceControlClass, formChoiceLabelClass, formControlClass, formLabelClass } from '@/utils/formControl';
import {
  WITHDRAWAL_DESTINATIONS,
  WITHDRAWAL_REASONS,
  addScannedWithdrawalCopy,
  canMarkDisposed,
  csvExportLanguage,
  emptyWithdrawalFilters,
  exportPeriodError,
  toWithdrawalListParams,
  withdrawalActorName,
  withdrawalAgentName,
  withdrawalExportFilename,
  type ScannedWithdrawalCopy,
  type WithdrawalListFilters,
} from '@/utils/withdrawals';

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function formatWhen(value: string | null | undefined, language: string): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(language);
}

export default function WeedingPage() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const canDispose = canManageWeeding(user, api.getToken());
  const [filters, setFilters] = useState<WithdrawalListFilters>(emptyWithdrawalFilters);
  const [selected, setSelected] = useState<string[]>([]);
  const [confirmDispose, setConfirmDispose] = useState(false);
  const [disposeError, setDisposeError] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [collectionSearch, setCollectionSearch] = useState('');
  const [scanList, setScanList] = useState<ScannedWithdrawalCopy[]>([]);
  const scanListRef = useRef<ScannedWithdrawalCopy[]>([]);
  const [barcode, setBarcode] = useState('');
  const [scanError, setScanError] = useState<string | null>(null);
  const [batchOpen, setBatchOpen] = useState(false);

  const params = useMemo(() => toWithdrawalListParams(filters), [filters]);
  const listQuery = useQuery({
    queryKey: ['item-withdrawals', params],
    queryFn: () => api.listItemWithdrawals(params),
  });
  const sourcesQuery = useSourcesQuery();
  const collectionsQuery = useQuery({
    queryKey: ['collections', 'weeding', collectionSearch],
    queryFn: () => api.getCollections({ name: collectionSearch.trim() || undefined, page: 1, perPage: 50 }),
  });

  const rows = listQuery.data?.items ?? [];
  const eligibleOnPage = rows.filter((row) => canMarkDisposed(row));
  const periodError = exportPeriodError(filters.from, filters.to);

  const patchFilters = (patch: Partial<WithdrawalListFilters>) => {
    setFilters((prev) => ({ ...prev, ...patch, page: patch.page ?? 1 }));
    setSelected([]);
  };

  const exportMutation = useMutation({
    mutationFn: async () => {
      const lang = csvExportLanguage(i18n.language);
      return api.exportItemWithdrawalsCsv(
        toWithdrawalListParams(filters, { forExport: true, lang }),
      );
    },
    onSuccess: ({ blob }) => {
      setExportError(null);
      downloadBlob(blob, withdrawalExportFilename(i18n.language, filters.from, filters.to));
    },
    onError: (err) => setExportError(getApiErrorMessage(err, t)),
  });

  const scanMutation = useMutation({
    mutationFn: (code: string) => api.getItemByBarcode(code),
    onSuccess: (notice, code) => {
      const copies = notice.items ?? [];
      const item = copies.find(
        (copy) => (copy.barcode ?? '').trim() === code && copy.id != null,
      ) ?? copies.find((copy) => copy.id != null);
      if (!item?.id || !notice.id) {
        setScanError(t('weeding.scanNotFound'));
        return;
      }
      if ('weedingStatus' in item && item.weedingStatus === 'withdrawn') {
        setScanError(t('weeding.scanAlreadyWithdrawn'));
        return;
      }
      const next = addScannedWithdrawalCopy(scanListRef.current, {
        itemId: item.id,
        biblioId: notice.id,
        barcode: (item.barcode ?? code).trim(),
        title: (notice.title ?? '').trim() || t('items.noTitle'),
      });
      scanListRef.current = next.list;
      setScanList(next.list);
      setScanError(next.duplicate ? t('weeding.scanDuplicate') : null);
      if (!next.duplicate) setBarcode('');
    },
    onError: (err) => setScanError(getApiErrorMessage(err, t)),
  });

  const disposeMutation = useMutation({
    mutationFn: (ids: string[]) => api.disposeItemWithdrawals(ids),
    onSuccess: () => {
      setConfirmDispose(false);
      setDisposeError(null);
      setSelected([]);
      void queryClient.invalidateQueries({ queryKey: ['item-withdrawals'] });
    },
    onError: (err) => setDisposeError(disposeBatchErrorMessage(err, t)),
  });

  const total = listQuery.data?.total ?? 0;
  const perPage = listQuery.data?.perPage || filters.perPage;
  const totalPages = Math.max(1, Math.ceil(total / perPage));
  const collections = collectionsQuery.data?.items ?? [];

  const toggle = (id: string) => {
    setSelected((prev) => (prev.includes(id) ? prev.filter((value) => value !== id) : [...prev, id]));
  };

  const togglePage = () => {
    const ids = eligibleOnPage.map((row) => row.id);
    const allSelected = ids.length > 0 && ids.every((id) => selected.includes(id));
    setSelected((prev) => {
      if (allSelected) return prev.filter((id) => !ids.includes(id));
      return [...new Set([...prev, ...ids])];
    });
  };

  return (
    <div className="max-w-6xl mx-auto space-y-6">
      <div className="flex items-start gap-3">
        <div className="h-11 w-11 rounded-xl bg-amber-100 dark:bg-amber-900/40 flex items-center justify-center shrink-0">
          <Leaf className="h-6 w-6 text-amber-700 dark:text-amber-300" aria-hidden />
        </div>
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('weeding.pageTitle')}</h1>
          <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">{t('weeding.pageSubtitle')}</p>
        </div>
      </div>

      <Card>
        <CardHeader title={t('weeding.filtersTitle')} />
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          <FilterField label={t('weeding.filterFrom')} id="weeding-from">
            <input
              id="weeding-from"
              type="date"
              className={formControlClass({ className: 'w-full' })}
              value={filters.from}
              onChange={(e) => patchFilters({ from: e.target.value })}
            />
          </FilterField>
          <FilterField label={t('weeding.filterTo')} id="weeding-to">
            <input
              id="weeding-to"
              type="date"
              className={formControlClass({ className: 'w-full' })}
              value={filters.to}
              onChange={(e) => patchFilters({ to: e.target.value })}
            />
          </FilterField>
          <FilterField label={t('weeding.filterSite')} id="weeding-site">
            <select
              id="weeding-site"
              className={formControlClass({ className: 'w-full' })}
              value={filters.siteId}
              onChange={(e) => patchFilters({ siteId: e.target.value })}
            >
              <option value="">{t('common.all')}</option>
              {(sourcesQuery.data ?? []).map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name || source.id}
                </option>
              ))}
            </select>
          </FilterField>
          <FilterField label={t('weeding.reason')} id="weeding-reason">
            <select
              id="weeding-reason"
              className={formControlClass({ className: 'w-full' })}
              value={filters.reason}
              onChange={(e) => patchFilters({ reason: e.target.value as WithdrawalListFilters['reason'] })}
            >
              <option value="">{t('common.all')}</option>
              {WITHDRAWAL_REASONS.map((reason) => (
                <option key={reason} value={reason}>
                  {t(`weeding.reasons.${reason}`)}
                </option>
              ))}
            </select>
          </FilterField>
          <FilterField label={t('weeding.destinationLabel')} id="weeding-destination">
            <select
              id="weeding-destination"
              className={formControlClass({ className: 'w-full' })}
              value={filters.destination}
              onChange={(e) =>
                patchFilters({ destination: e.target.value as WithdrawalListFilters['destination'] })
              }
            >
              <option value="">{t('common.all')}</option>
              {WITHDRAWAL_DESTINATIONS.map((destination) => (
                <option key={destination} value={destination}>
                  {t(`weeding.destinations.${destination}`)}
                </option>
              ))}
              <option value="unset">{t('weeding.destinationUnset')}</option>
            </select>
          </FilterField>
          <FilterField label={t('weeding.filterDisposed')} id="weeding-disposed">
            <select
              id="weeding-disposed"
              className={formControlClass({ className: 'w-full' })}
              value={filters.disposed}
              onChange={(e) => patchFilters({ disposed: e.target.value as WithdrawalListFilters['disposed'] })}
            >
              <option value="all">{t('common.all')}</option>
              <option value="yes">{t('weeding.disposedYes')}</option>
              <option value="no">{t('weeding.disposedNo')}</option>
            </select>
          </FilterField>
          <FilterField label={t('weeding.filterCollectionSearch')} id="weeding-collection-search">
            <input
              id="weeding-collection-search"
              className={formControlClass({ className: 'w-full' })}
              value={collectionSearch}
              onChange={(e) => setCollectionSearch(e.target.value)}
              placeholder={t('weeding.filterCollectionSearch')}
            />
          </FilterField>
          <FilterField label={t('weeding.filterCollection')} id="weeding-collection">
            <select
              id="weeding-collection"
              className={formControlClass({ className: 'w-full' })}
              value={filters.collectionId}
              onChange={(e) => patchFilters({ collectionId: e.target.value })}
            >
              <option value="">{t('common.all')}</option>
              {collections.flatMap((collection: Collection) =>
                collection.id
                  ? [
                      <option key={collection.id} value={collection.id}>
                        {collection.name || collection.id}
                      </option>,
                    ]
                  : [],
              )}
            </select>
          </FilterField>
          <FilterField label={t('weeding.filterCallNumber')} id="weeding-call">
            <input
              id="weeding-call"
              className={formControlClass({ className: 'w-full' })}
              value={filters.callNumber}
              onChange={(e) => patchFilters({ callNumber: e.target.value })}
            />
          </FilterField>
        </div>
        <label className={`${formChoiceLabelClass('mt-3')} `}>
          <input
            type="checkbox"
            className={formChoiceControlClass()}
            checked={filters.includeCancelled}
            onChange={(e) => patchFilters({ includeCancelled: e.target.checked })}
          />
          {t('weeding.includeCancelled')}
        </label>
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <Button
            variant="secondary"
            leftIcon={<Download className="h-4 w-4" />}
            disabled={periodError !== null || exportMutation.isPending}
            isLoading={exportMutation.isPending}
            title={periodError === 'missing' ? t('weeding.exportNeedsPeriod') : undefined}
            onClick={() => {
              if (periodError === 'missing') {
                setExportError(t('weeding.exportNeedsPeriod'));
                return;
              }
              if (periodError === 'order') {
                setExportError(t('weeding.exportPeriodOrder'));
                return;
              }
              exportMutation.mutate();
            }}
          >
            {t('weeding.exportCsv')}
          </Button>
          {periodError === 'missing' ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.exportNeedsPeriod')}</p>
          ) : null}
          {periodError === 'order' ? (
            <p className="text-xs text-red-600 dark:text-red-400">{t('weeding.exportPeriodOrder')}</p>
          ) : null}
          {canDispose ? (
            <Button
              variant={selected.length === 0 ? 'secondary' : 'danger'}
              disabled={selected.length === 0 || disposeMutation.isPending}
              onClick={() => {
                setDisposeError(null);
                setConfirmDispose(true);
              }}
            >
              {t('weeding.markDisposed')}
              {selected.length > 0 ? ` (${selected.length})` : ''}
            </Button>
          ) : null}
          <Button
            variant="ghost"
            onClick={() => {
              setFilters(emptyWithdrawalFilters());
              setSelected([]);
              setCollectionSearch('');
              setExportError(null);
            }}
          >
            {t('common.reset')}
          </Button>
        </div>
        {exportError ? (
          <p role="alert" className="mt-3 text-sm text-red-600 dark:text-red-400">
            {exportError}
          </p>
        ) : null}
      </Card>

      {canDispose ? (
        <Card>
          <CardHeader title={t('weeding.scanTitle')} subtitle={t('weeding.scanHint')} />
          <form
            className="flex flex-wrap gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              const code = barcode.trim();
              if (!code || scanMutation.isPending) return;
              setScanError(null);
              scanMutation.mutate(code);
            }}
          >
            <input
              className={formControlClass({ className: 'min-w-[16rem] flex-1' })}
              value={barcode}
              onChange={(event) => setBarcode(event.target.value)}
              placeholder={t('weeding.scanPlaceholder')}
              aria-label={t('weeding.scanPlaceholder')}
            />
            <Button type="submit" variant="secondary" isLoading={scanMutation.isPending}>
              {t('weeding.scanAdd')}
            </Button>
            <Button
              type="button"
              variant={scanList.length === 0 ? 'secondary' : 'danger'}
              disabled={scanList.length === 0}
              onClick={() => setBatchOpen(true)}
            >
              {t('weeding.withdrawSelected', { count: scanList.length })}
            </Button>
          </form>
          {scanError ? (
            <p role="alert" className="mt-3 text-sm text-red-600 dark:text-red-400">{scanError}</p>
          ) : null}
          {scanList.length === 0 ? (
            <p className="mt-3 text-sm text-gray-500 dark:text-gray-400">{t('weeding.scanEmpty')}</p>
          ) : (
            <ul className="mt-3 space-y-2">
              {scanList.map((copy) => (
                <li key={copy.itemId} className="flex items-center justify-between gap-3 text-sm">
                  <span className="min-w-0 truncate text-gray-800 dark:text-gray-100">
                    {copy.barcode} · {copy.title}
                  </span>
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      const next = scanListRef.current.filter((row) => row.itemId !== copy.itemId);
                      scanListRef.current = next;
                      setScanList(next);
                    }}
                  >
                    {t('weeding.scanRemove')}
                  </Button>
                </li>
              ))}
            </ul>
          )}
          <WithdrawalDialog
            open={batchOpen}
            mode="create"
            itemIds={scanList.map((copy) => copy.itemId)}
            onClose={() => setBatchOpen(false)}
            onSuccess={() => {
              setBatchOpen(false);
              scanListRef.current = [];
              setScanList([]);
              setScanError(null);
              void queryClient.invalidateQueries({ queryKey: ['item-withdrawals'] });
            }}
          />
        </Card>
      ) : null}

      <Totals
        reasons={listQuery.data?.totalsByReason ?? []}
        destinations={listQuery.data?.totalsByDestination ?? []}
      />

      <Card>
        <CardHeader
          title={t('weeding.listTitle')}
          subtitle={t('weeding.listCount', { count: total })}
          action={
            canDispose && eligibleOnPage.length > 0 ? (
              <label className={formChoiceLabelClass()}>
                <input
                  type="checkbox"
                  className={formChoiceControlClass()}
                  checked={eligibleOnPage.every((row) => selected.includes(row.id))}
                  onChange={togglePage}
                />
                {t('weeding.selectPage')}
              </label>
            ) : null
          }
        />
        {listQuery.isLoading ? (
          <div className="flex justify-center py-10">
            <div className="h-8 w-8 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
          </div>
        ) : null}
        {listQuery.isError ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {getApiErrorMessage(listQuery.error, t)}
          </p>
        ) : null}
        {!listQuery.isLoading && !listQuery.isError && rows.length === 0 ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('weeding.empty')}</p>
        ) : null}
        <ul className="space-y-3">
          {rows.map((row) => (
            <WithdrawalRow
              key={row.id}
              row={row}
              language={i18n.language}
              selectable={canDispose && canMarkDisposed(row)}
              selected={selected.includes(row.id)}
              onToggle={() => toggle(row.id)}
            />
          ))}
        </ul>
        <div className="mt-4">
          <Pagination
            currentPage={filters.page}
            totalPages={totalPages}
            onPageChange={(page) => setFilters((prev) => ({ ...prev, page }))}
          />
        </div>
      </Card>

      <ConfirmDialog
        isOpen={confirmDispose}
        onClose={() => {
          if (disposeMutation.isPending) return;
          setConfirmDispose(false);
          setDisposeError(null);
        }}
        onConfirm={() => disposeMutation.mutate(selected)}
        title={t('weeding.markDisposed')}
        message={t('weeding.markDisposedConfirm', { count: selected.length })}
        confirmLabel={t('weeding.markDisposed')}
        confirmVariant="danger"
        isLoading={disposeMutation.isPending}
        error={disposeError}
      />
    </div>
  );
}

function FilterField({
  id,
  label,
  children,
}: {
  id: string;
  label: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label htmlFor={id} className={formLabelClass()}>
        {label}
      </label>
      {children}
    </div>
  );
}

function Totals({
  reasons,
  destinations,
}: {
  reasons: { reason: string; count: number }[];
  destinations: { destination: string | null; count: number }[];
}) {
  const { t } = useTranslation();
  if (reasons.length === 0 && destinations.length === 0) return null;
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      <Card>
        <CardHeader title={t('weeding.totalsByReason')} />
        <div className="flex flex-wrap gap-2">
          {reasons.map((row) => (
            <Badge key={row.reason} variant="default">
              {t(`weeding.reasons.${row.reason}`)} · {row.count}
            </Badge>
          ))}
        </div>
      </Card>
      <Card>
        <CardHeader title={t('weeding.totalsByDestination')} />
        <div className="flex flex-wrap gap-2">
          {destinations.map((row) => (
            <Badge key={row.destination ?? 'unset'} variant="default">
              {row.destination ? t(`weeding.destinations.${row.destination}`) : t('weeding.destinationUnset')}
              {' · '}
              {row.count}
            </Badge>
          ))}
        </div>
      </Card>
    </div>
  );
}

function WithdrawalRow({
  row,
  language,
  selectable,
  selected,
  onToggle,
}: {
  row: ItemWithdrawal;
  language: string;
  selectable: boolean;
  selected: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  const disposed = !!row.disposedAt;
  const destination = row.destination
    ? t(`weeding.destinations.${row.destination}`)
    : t('weeding.destinationUnset');
  const withdrawnLine = statusLine(
    t,
    'withdrawn',
    formatWhen(row.withdrawnAt, language),
    withdrawalAgentName(row),
    destination,
  );
  const disposedLine = disposed
    ? statusLine(t, 'disposed', formatWhen(row.disposedAt, language), withdrawalActorName(row), destination)
    : null;

  return (
    <li className="rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/40">
      <div className="flex items-start gap-3 p-3">
        {selectable ? (
          <input
            type="checkbox"
            className={`${formChoiceControlClass()} mt-1`}
            checked={selected}
            onClick={(event) => event.stopPropagation()}
            onChange={onToggle}
            aria-label={t('weeding.selectRow', { barcode: row.barcode || row.id })}
          />
        ) : (
          <span className="w-4 shrink-0" aria-hidden />
        )}
        <Link
          to={`/weeding/${row.id}`}
          className="group min-w-0 flex-1 space-y-2 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500"
        >
          <div className="flex flex-wrap items-center gap-2">
            <p className="font-medium text-gray-900 underline-offset-2 group-hover:underline dark:text-white">
              {row.biblioTitle || t('items.noTitle')}
            </p>
            {row.cancelledAt ? <Badge size="sm" variant="default">{t('weeding.cancelled')}</Badge> : null}
            {row.legacy ? <Badge size="sm">{t('weeding.legacyBadge')}</Badge> : null}
          </div>
          <p className="text-sm text-gray-600 dark:text-gray-300">
            {row.barcode || t('items.noBarcode')}
            {row.callNumber ? ` · ${row.callNumber}` : ''}
            {row.siteName ? ` · ${row.siteName}` : ''}
            {' · '}
            {t(`weeding.reasons.${row.reason}`)}
            {row.reasonDetail?.trim() ? ` — ${row.reasonDetail.trim()}` : ''}
          </p>
          <dl className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-sm">
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('weeding.columnWithdrawnAt')}
              </dt>
              <dd className="text-gray-800 dark:text-gray-100">{withdrawnLine}</dd>
            </div>
            <div>
              <dt className="text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {t('weeding.columnDisposedAt')}
              </dt>
              <dd className="text-gray-800 dark:text-gray-100">{disposedLine ?? '—'}</dd>
            </div>
          </dl>
          {!selectable && !row.cancelledAt && disposed ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('weeding.alreadyDisposed')}</p>
          ) : null}
        </Link>
      </div>
    </li>
  );
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
