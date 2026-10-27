import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Newspaper, Plus } from 'lucide-react';
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
import SerialsTabBar from '@/components/serials/SerialsTabBar';
import { SubscriptionStatusBadge } from '@/components/serials/IssueStatusBadge';
import SubscriptionFormModal from '@/components/serials/SubscriptionFormModal';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { useActiveVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import {
  canManageSerials,
  type SerialSubscription,
  type SerialSubscriptionStatus,
  type SerialSubscriptionWrite,
} from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const PER_PAGE = 20;

export default function SerialsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageSerials(user, api.getToken());
  const [q, setQ] = useState('');
  const [siteId, setSiteId] = useState('');
  const [vendorId, setVendorId] = useState('');
  const [status, setStatus] = useState<SerialSubscriptionStatus | ''>('');
  const [page, setPage] = useState(1);
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<unknown>(null);

  const sitesQuery = useQuery({
    queryKey: ['sites', 'active'],
    queryFn: () => api.getSites(false),
  });
  const vendorsQuery = useActiveVendorsQuery(true);
  const listQuery = useQuery({
    queryKey: ['serial-subscriptions', q, siteId, vendorId, status, page],
    queryFn: () =>
      api.listSerialSubscriptions({
        q: q.trim() || undefined,
        siteId: siteId || undefined,
        vendorId: vendorId || undefined,
        status: status || undefined,
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

  const rows = listQuery.data?.subscriptions ?? [];
  const total = listQuery.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));
  const siteName = (id: string) => sitesQuery.data?.find((site) => String(site.id) === id)?.name || id;
  const vendorName = (id: string | null | undefined) =>
    id ? vendorsQuery.data?.vendors.find((vendor) => vendor.id === id)?.name || id : '—';

  const columns = [
    {
      key: 'title',
      header: t('serials.column.title'),
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
      header: t('serials.column.vendor'),
      render: (row: SerialSubscription) => vendorName(row.vendorId),
    },
    {
      key: 'status',
      header: t('common.status'),
      render: (row: SerialSubscription) => <SubscriptionStatusBadge status={row.status} />,
    },
  ];

  return (
    <div className={`${APP_PAGE_CLASS} flex flex-col lg:min-h-0 lg:flex-1`}>
      <PageTabLeadGroup>
        <PageHeading icon={Newspaper} title={t('nav.serials')} subtitle={t('serials.pageSubtitle')} />
        <SerialsTabBar activeTab="subscriptions" onSelectTab={() => undefined} />
      </PageTabLeadGroup>
      <div role="tabpanel" id="section-panel-subscriptions" aria-labelledby="section-tab-subscriptions" className="flex min-h-0 flex-1 flex-col">
        <ListPageLayout>
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
            <p className="text-sm text-gray-600 dark:text-gray-300">{t('serials.list.help')}</p>
            {canWrite ? (
              <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => { setFormError(null); setCreating(true); }}>
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
                  <option value="expired">{t('serials.status.expired')}</option>
                  <option value="closed">{t('serials.status.closed')}</option>
                </select>
              </div>
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
                            {row.issns.join(', ') || '—'} · {siteName(row.siteId)}
                          </span>
                          <SubscriptionStatusBadge status={row.status} />
                        </button>
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
      {creating ? (
        <SubscriptionFormModal
          subscription={null}
          isOpen
          isSaving={createMutation.isPending}
          error={formError}
          onClose={() => setCreating(false)}
          onSubmit={(body) => createMutation.mutate(body)}
        />
      ) : null}
    </div>
  );
}
