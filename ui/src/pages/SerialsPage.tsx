import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowDown, ArrowUp, ArrowUpDown, Newspaper, Plus } from 'lucide-react';
import {
  Button,
  Card,
  ListPageLayout,
  ListResultCountBar,
  PageHeading,
  PageTabLeadGroup,
  Pagination,
  QueryErrorBanner,
  ResponsiveRecordList,
  SearchInput,
  Table,
} from '@/components/common';
import SerialsTabBar, { isSerialsTab, type SerialsTab } from '@/components/serials/SerialsTabBar';
import SerialGiftsPanel from '@/components/serials/SerialGiftsPanel';
import { SubscriptionStatusBadge } from '@/components/serials/IssueStatusBadge';
import SubscriptionFormModal from '@/components/serials/SubscriptionFormModal';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { useActiveVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import {
  canManageItems,
  canManageSerials,
  type SerialSubscription,
  type SerialSubscriptionStatus,
  type SerialSubscriptionWrite,
} from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const PER_PAGE = 20;

type SerialListSort = 'title' | 'vendor' | 'itemCount';
type SerialListDirection = 'asc' | 'desc';

export default function SerialsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const tabParam = searchParams.get('tab');
  const activeTab: SerialsTab = isSerialsTab(tabParam) ? tabParam : 'subscriptions';
  const giftBiblioId = searchParams.get('biblioId') ?? undefined;
  const giftBiblioTitle = searchParams.get('biblioTitle') ?? undefined;
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canWrite = canManageSerials(user, token);
  const canEditItems = canManageItems(user, token);

  const selectTab = (tab: SerialsTab) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('tab', tab);
        if (tab !== 'gifts') {
          next.delete('biblioId');
          next.delete('biblioTitle');
        }
        return next;
      },
      { replace: true },
    );
  };
  const [q, setQ] = useState('');
  const [siteId, setSiteId] = useState('');
  const [vendorId, setVendorId] = useState('');
  const [status, setStatus] = useState<SerialSubscriptionStatus | ''>('');
  const [sort, setSort] = useState<SerialListSort>('title');
  const [direction, setDirection] = useState<SerialListDirection>('asc');
  const [page, setPage] = useState(1);
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<SerialSubscription | null>(null);
  const [formError, setFormError] = useState<unknown>(null);

  const sitesQuery = useQuery({
    queryKey: ['sites', 'active'],
    queryFn: () => api.getSites(false),
  });
  const vendorsQuery = useActiveVendorsQuery(true);
  const listQuery = useQuery({
    queryKey: ['serial-subscriptions', q, siteId, vendorId, status, sort, direction, page],
    queryFn: () =>
      api.listSerialSubscriptions({
        q: q.trim() || undefined,
        siteId: siteId || undefined,
        vendorId: vendorId || undefined,
        status: status || undefined,
        sort,
        direction,
        page,
        perPage: PER_PAGE,
      }),
  });

  const createMutation = useMutation({
    mutationFn: (body: SerialSubscriptionWrite) => api.createSerialSubscription(body),
    onSuccess: async (created) => {
      setCreating(false);
      setFormError(null);
      await queryClient.invalidateQueries({ queryKey: ['serial-subscriptions'] });
      showToast({ variant: 'success', message: t('serials.saved') });
      navigate(`/serials/${created.id}`);
    },
    onError: (error: unknown) => setFormError(error),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, body }: { id: string; body: SerialSubscriptionWrite }) =>
      api.updateSerialSubscription(id, body),
    onSuccess: async () => {
      setEditing(null);
      setFormError(null);
      await queryClient.invalidateQueries({ queryKey: ['serial-subscriptions'] });
      showToast({ variant: 'success', message: t('serials.saved') });
    },
    onError: (error: unknown) => setFormError(error),
  });

  const onSort = (column: SerialListSort) => {
    if (sort === column) {
      setDirection((current) => (current === 'asc' ? 'desc' : 'asc'));
    } else {
      setSort(column);
      setDirection(column === 'itemCount' ? 'desc' : 'asc');
    }
    setPage(1);
  };

  const rows = listQuery.data?.subscriptions ?? [];
  const total = listQuery.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));
  const siteName = (id: string) => sitesQuery.data?.find((site) => String(site.id) === id)?.name || id;
  const vendorName = (id: string | null | undefined) =>
    id ? vendorsQuery.data?.vendors.find((vendor) => vendor.id === id)?.name || id : '—';

  const sortHeader = (column: SerialListSort, label: string) => (
    <SortHeader
      label={label}
      pressedLabel={
        sort === column
          ? t(direction === 'asc' ? 'serials.list.sortAscending' : 'serials.list.sortDescending')
          : t('serials.list.sort')
      }
      active={sort === column}
      direction={direction}
      onClick={() => onSort(column)}
    />
  );

  const columns = [
    {
      key: 'title',
      header: sortHeader('title', t('serials.column.title')),
      render: (row: SerialSubscription) => (
        <span className="font-medium text-gray-900 dark:text-white">{row.biblioTitle?.trim() || row.biblioId}</span>
      ),
    },
    {
      key: 'issn',
      header: t('serials.column.issn'),
      render: (row: SerialSubscription) => row.issns.join(', ') || '—',
    },
    {
      key: 'site',
      header: t('serials.column.site'),
      render: (row: SerialSubscription) => siteName(row.siteId),
    },
    {
      key: 'vendor',
      header: sortHeader('vendor', t('serials.column.vendor')),
      render: (row: SerialSubscription) => vendorName(row.vendorId),
    },
    {
      key: 'items',
      header: sortHeader('itemCount', t('serials.column.items')),
      align: 'right' as const,
      render: (row: SerialSubscription) => row.itemCount,
    },
    {
      key: 'status',
      header: t('common.status'),
      render: (row: SerialSubscription) => <SubscriptionStatusBadge status={row.status} />,
    },
    {
      key: 'actions',
      header: t('common.actions'),
      render: (row: SerialSubscription) =>
        canWrite || canEditItems ? (
          <SubscriptionRowActions
            row={row}
            canWrite={canWrite}
            canEditItems={canEditItems}
            onEditSubscription={(subscription) => {
              setFormError(null);
              setCreating(false);
              setEditing(subscription);
            }}
            onEditNotice={() => navigate(`/biblios/${row.biblioId}/edit`)}
            onEditCopy={
              row.latestItemId
                ? () => navigate(`/biblios/${row.biblioId}?completeItem=${row.latestItemId}`)
                : undefined
            }
          />
        ) : null,
    },
  ].filter((column) => column.key !== 'actions' || canWrite || canEditItems);

  return (
    <div className={`${APP_PAGE_CLASS} flex flex-col lg:min-h-0 lg:flex-1`}>
      <PageTabLeadGroup>
        <PageHeading icon={Newspaper} title={t('nav.serials')} subtitle={t('serials.pageSubtitle')} />
        <SerialsTabBar activeTab={activeTab} onSelectTab={selectTab} />
      </PageTabLeadGroup>
      {activeTab === 'gifts' ? (
        <SerialGiftsPanel
          initialBiblioId={giftBiblioId}
          initialBiblioTitle={giftBiblioTitle ?? undefined}
          openCreateOnMount={Boolean(giftBiblioId)}
        />
      ) : null}
      {activeTab === 'subscriptions' ? (
      <>
      <div role="tabpanel" id="section-panel-subscriptions" aria-labelledby="section-tab-subscriptions" className="flex min-h-0 flex-1 flex-col">
        <ListPageLayout>
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
            <p className="text-sm text-gray-600 dark:text-gray-300">{t('serials.list.help')}</p>
            {canWrite ? (
              <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => { setFormError(null); setEditing(null); setCreating(true); }}>
                {t('serials.list.create')}
              </Button>
            ) : null}
          </div>
          <Card padding="none" className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="grid shrink-0 gap-3 p-4 md:grid-cols-4">
              <div className="md:col-span-2">
                <SearchInput
                  value={q}
                  onChange={(value) => {
                    setQ(value);
                    setPage(1);
                  }}
                  placeholder={t('serials.list.search')}
                />
              </div>
              <div>
                <label htmlFor="serial-filter-site" className={formLabelClass({ marginBottom: false })}>
                  {t('serials.column.site')}
                </label>
                <select
                  id="serial-filter-site"
                  className={formControlClass()}
                  value={siteId}
                  onChange={(event) => {
                    setSiteId(event.target.value);
                    setPage(1);
                  }}
                >
                  <option value="">{t('serials.list.allSites')}</option>
                  {(sitesQuery.data ?? []).map((site) => (
                    <option key={site.id} value={String(site.id)}>
                      {site.name || site.id}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label htmlFor="serial-filter-vendor" className={formLabelClass({ marginBottom: false })}>
                  {t('serials.column.vendor')}
                </label>
                <select
                  id="serial-filter-vendor"
                  className={formControlClass()}
                  value={vendorId}
                  onChange={(event) => {
                    setVendorId(event.target.value);
                    setPage(1);
                  }}
                >
                  <option value="">{t('serials.list.allVendors')}</option>
                  {(vendorsQuery.data?.vendors ?? []).filter((vendor) => vendor.active).map((vendor) => (
                    <option key={vendor.id} value={vendor.id}>
                      {vendor.name}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label htmlFor="serial-filter-status" className={formLabelClass({ marginBottom: false })}>
                  {t('common.status')}
                </label>
                <select
                  id="serial-filter-status"
                  className={formControlClass()}
                  value={status}
                  onChange={(event) => {
                    setStatus(event.target.value as SerialSubscriptionStatus | '');
                    setPage(1);
                  }}
                >
                  <option value="">{t('serials.list.allStatuses')}</option>
                  <option value="active">{t('serials.status.active')}</option>
                  <option value="suspended">{t('serials.status.suspended')}</option>
                  <option value="expired">{t('serials.status.expired')}</option>
                  <option value="closed">{t('serials.status.closed')}</option>
                </select>
              </div>
            </div>
            <div className="mt-3 flex items-end gap-2 px-4 md:hidden">
              <div className="min-w-0 flex-1">
                <label htmlFor="serial-sort" className={formLabelClass({ marginBottom: false })}>
                  {t('serials.list.sort')}
                </label>
                <select
                  id="serial-sort"
                  className={formControlClass({ className: 'w-full' })}
                  value={sort}
                  onChange={(event) => {
                    const next = event.target.value as SerialListSort;
                    setSort(next);
                    setDirection(next === 'itemCount' ? 'desc' : 'asc');
                    setPage(1);
                  }}
                >
                  <option value="title">{t('serials.column.title')}</option>
                  <option value="vendor">{t('serials.column.vendor')}</option>
                  <option value="itemCount">{t('serials.column.items')}</option>
                </select>
              </div>
              <button
                type="button"
                className={formControlClass({ className: 'inline-flex w-10 items-center justify-center px-0' })}
                aria-label={t(direction === 'asc' ? 'serials.list.sortAscending' : 'serials.list.sortDescending')}
                onClick={() => {
                  setDirection((current) => (current === 'asc' ? 'desc' : 'asc'));
                  setPage(1);
                }}
              >
                {direction === 'asc' ? <ArrowUp className="h-4 w-4" aria-hidden /> : <ArrowDown className="h-4 w-4" aria-hidden />}
              </button>
            </div>
            {listQuery.isError ? (
              <QueryErrorBanner message={getApiErrorMessage(listQuery.error, t)} onRetry={() => void listQuery.refetch()} />
            ) : null}
            <ListResultCountBar>{t('common.listCount', { count: total })}</ListResultCountBar>
            <ResponsiveRecordList
              desktop={
                <Table
                  columns={columns}
                  data={rows}
                  keyExtractor={(row) => row.id}
                  onRowClick={(row) => navigate(`/serials/${row.id}`)}
                  isLoading={listQuery.isLoading}
                  emptyMessage={t('serials.list.empty')}
                />
              }
              mobile={
                listQuery.isLoading ? (
                  <p className="px-4 py-6 text-sm text-gray-500 dark:text-gray-400">{t('common.loading')}</p>
                ) : rows.length === 0 ? (
                  <p className="px-4 py-6 text-sm text-gray-500 dark:text-gray-400">{t('serials.list.empty')}</p>
                ) : (
                  <ul className="divide-y divide-gray-200 dark:divide-gray-800">
                    {rows.map((row) => (
                      <li key={row.id}>
                        <button
                          type="button"
                          className="w-full space-y-1 px-4 py-3 text-left hover:bg-amber-50 dark:hover:bg-gray-800"
                          onClick={() => navigate(`/serials/${row.id}`)}
                        >
                          <span className="block font-medium text-gray-900 dark:text-white">
                            {row.biblioTitle?.trim() || row.biblioId}
                          </span>
                          <span className="block text-sm text-gray-600 dark:text-gray-300">
                            {row.issns.join(', ') || '—'} · {siteName(row.siteId)} · {vendorName(row.vendorId)}
                          </span>
                          <span className="block text-sm text-gray-600 dark:text-gray-300">
                            {t('serials.column.items')}: {row.itemCount}
                          </span>
                          <SubscriptionStatusBadge status={row.status} />
                        </button>
                        <div className="px-4 pb-3">
                          <SubscriptionRowActions
                            row={row}
                            canWrite={canWrite}
                            canEditItems={canEditItems}
                            onEditSubscription={(subscription) => {
                              setFormError(null);
                              setCreating(false);
                              setEditing(subscription);
                            }}
                            onEditNotice={() => navigate(`/biblios/${row.biblioId}/edit`)}
                            onEditCopy={
                              row.latestItemId
                                ? () => navigate(`/biblios/${row.biblioId}?completeItem=${row.latestItemId}`)
                                : undefined
                            }
                          />
                        </div>
                      </li>
                    ))}
                  </ul>
                )
              }
            />
            <div className="shrink-0 p-3">
              <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
            </div>
          </Card>
        </ListPageLayout>
      </div>
      {creating || editing ? (
        <SubscriptionFormModal
          key={editing?.id ?? 'new'}
          subscription={editing}
          isOpen
          isSaving={createMutation.isPending || updateMutation.isPending}
          error={formError}
          onClose={() => {
            setCreating(false);
            setEditing(null);
          }}
          onSubmit={(body) => {
            if (editing) updateMutation.mutate({ id: editing.id, body });
            else createMutation.mutate(body);
          }}
        />
      ) : null}
      </>
      ) : null}
    </div>
  );
}

function SortHeader({
  label,
  pressedLabel,
  active,
  direction,
  onClick,
}: {
  label: string;
  pressedLabel: string;
  active: boolean;
  direction: SerialListDirection;
  onClick: () => void;
}) {
  const Icon = active ? (direction === 'asc' ? ArrowUp : ArrowDown) : ArrowUpDown;
  return (
    <button
      type="button"
      className="inline-flex items-center gap-1 uppercase tracking-wider hover:text-gray-800 dark:hover:text-gray-200"
      aria-label={`${label}, ${pressedLabel}`}
      onClick={onClick}
    >
      {label}
      <Icon className={`h-3.5 w-3.5 ${active ? '' : 'opacity-40'}`} aria-hidden />
    </button>
  );
}

function SubscriptionRowActions({
  row,
  canWrite,
  canEditItems,
  onEditSubscription,
  onEditNotice,
  onEditCopy,
}: {
  row: SerialSubscription;
  canWrite: boolean;
  canEditItems: boolean;
  onEditSubscription: (row: SerialSubscription) => void;
  onEditNotice: () => void;
  onEditCopy?: () => void;
}) {
  const { t } = useTranslation();
  const title = row.biblioTitle?.trim() || row.biblioId;
  if (!canWrite && !canEditItems) return null;
  return (
    <div className="flex flex-wrap gap-1">
      {canWrite ? (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          aria-label={t('serials.list.editSubscriptionLabel', { title })}
          onClick={() => onEditSubscription(row)}
        >
          {t('serials.list.editSubscription')}
        </Button>
      ) : null}
      {canEditItems ? (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          aria-label={t('serials.list.editNoticeLabel', { title })}
          onClick={onEditNotice}
        >
          {t('serials.list.editNotice')}
        </Button>
      ) : null}
      {canEditItems && onEditCopy ? (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          aria-label={t('serials.list.editCopyLabel', { title })}
          onClick={onEditCopy}
        >
          {t('serials.list.editCopy')}
        </Button>
      ) : null}
    </div>
  );
}
