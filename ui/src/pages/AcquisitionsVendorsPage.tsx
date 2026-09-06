import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Archive, Plus, Store } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  ConfirmDialog,
  ListSkeleton,
  Pagination,
  QueryErrorBanner,
  ResponsiveRecordList,
  SearchInput,
  Table,
} from '@/components/common';
import AcquisitionsSubnav from '@/components/acquisitions/AcquisitionsSubnav';
import VendorFormModal from '@/components/acquisitions/VendorFormModal';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { ACQUISITIONS_VENDORS_KEY } from '@/hooks/acquisitions/queryKeys';
import { useVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import { canManageAcquisitions, type AcquisitionVendor, type CreateVendor, type UpdateVendor } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';

const PER_PAGE = 20;

export default function AcquisitionsVendorsPage() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageAcquisitions(user, api.getToken());

  const [q, setQ] = useState('');
  const [includeArchived, setIncludeArchived] = useState(false);
  const [page, setPage] = useState(1);
  const [formVendor, setFormVendor] = useState<AcquisitionVendor | null | undefined>(undefined);
  const [formError, setFormError] = useState<string | null>(null);
  const [archiveVendor, setArchiveVendor] = useState<AcquisitionVendor | null>(null);

  const query = useVendorsQuery({ q, includeArchived, page, perPage: PER_PAGE });
  const vendors = query.data?.vendors ?? [];
  const total = query.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  const saveMutation = useMutation({
    mutationFn: async (data: CreateVendor | UpdateVendor) => {
      if (formVendor) return api.updateVendor(formVendor.id, data);
      return api.createVendor(data as CreateVendor);
    },
    onSuccess: () => {
      setFormVendor(undefined);
      setFormError(null);
      void queryClient.invalidateQueries({ queryKey: ACQUISITIONS_VENDORS_KEY });
      showToast({ message: t('acquisitions.vendors.saved'), variant: 'success' });
    },
    onError: (err) => setFormError(getApiErrorMessage(err, t)),
  });

  const archiveMutation = useMutation({
    mutationFn: (id: string) => api.archiveVendor(id),
    onSuccess: () => {
      setArchiveVendor(null);
      void queryClient.invalidateQueries({ queryKey: ACQUISITIONS_VENDORS_KEY });
      showToast({ message: t('acquisitions.vendors.archived'), variant: 'success' });
    },
    onError: (err) => {
      showToast({ message: getApiErrorMessage(err, t), variant: 'error' });
    },
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('acquisitions.vendors.title')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('acquisitions.vendors.subtitle')}</p>
        </div>
        {canWrite ? (
          <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => { setFormError(null); setFormVendor(null); }}>
            {t('acquisitions.vendors.create')}
          </Button>
        ) : null}
      </div>
      <AcquisitionsSubnav />

      <Card className="overflow-hidden">
        <div className="flex flex-col gap-3 p-4 sm:flex-row sm:items-center">
          <div className="flex-1">
            <SearchInput
              value={q}
              onChange={(value) => {
                setQ(value);
                setPage(1);
              }}
              placeholder={t('acquisitions.vendors.search')}
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
            <input
              type="checkbox"
              checked={includeArchived}
              onChange={(e) => {
                setIncludeArchived(e.target.checked);
                setPage(1);
              }}
            />
            {t('acquisitions.vendors.includeArchived')}
          </label>
        </div>

        {query.isLoading && query.data === undefined ? (
          <ListSkeleton />
        ) : query.isError ? (
          <div className="p-4">
            <QueryErrorBanner
              message={getApiErrorMessage(query.error, t) || t('acquisitions.vendors.errorLoad')}
              onRetry={() => void query.refetch()}
            />
          </div>
        ) : (
          <>
            <ResponsiveRecordList
              desktop={
                <Table
                  data={vendors}
                  keyExtractor={(v) => v.id}
                  emptyMessage={t('acquisitions.vendors.empty')}
                  onRowClick={
                    canWrite
                      ? (v) => {
                          if (v.archivedAt) return;
                          setFormError(null);
                          setFormVendor(v);
                        }
                      : undefined
                  }
                  columns={[
                    { key: 'name', header: t('acquisitions.vendors.name'), render: (v) => v.name },
                    { key: 'code', header: t('acquisitions.vendors.code'), render: (v) => v.code || '—' },
                    { key: 'email', header: t('acquisitions.vendors.email'), render: (v) => v.email || '—' },
                    {
                      key: 'status',
                      header: t('common.status'),
                      render: (v) =>
                        v.archivedAt ? (
                          <Badge variant="default" size="sm">{t('acquisitions.vendors.archivedLabel')}</Badge>
                        ) : v.active ? (
                          <Badge variant="success" size="sm">{t('common.active')}</Badge>
                        ) : (
                          <Badge variant="warning" size="sm">{t('acquisitions.vendors.inactive')}</Badge>
                        ),
                    },
                    {
                      key: 'updated',
                      header: t('acquisitions.updatedAt'),
                      render: (v) => new Date(v.updatedAt).toLocaleDateString(i18n.language),
                    },
                    ...(canWrite
                      ? [{
                          key: 'actions',
                          header: t('common.actions'),
                          align: 'right' as const,
                          render: (v: AcquisitionVendor) =>
                            !v.archivedAt ? (
                              <button
                                type="button"
                                className="rounded-lg p-1.5 text-gray-500 hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950/40"
                                aria-label={t('acquisitions.vendors.archive')}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setArchiveVendor(v);
                                }}
                              >
                                <Archive className="h-4 w-4" />
                              </button>
                            ) : null,
                        }]
                      : []),
                  ]}
                />
              }
              mobile={
                vendors.length === 0 ? (
                  <p className="px-4 py-8 text-center text-sm text-gray-500">{t('acquisitions.vendors.empty')}</p>
                ) : (
                  <ul className="divide-y divide-gray-100 dark:divide-gray-800">
                    {vendors.map((v) => (
                      <li key={v.id} className="flex items-start gap-3 px-4 py-3">
                        <Store className="mt-0.5 h-5 w-5 shrink-0 text-amber-600" />
                        {canWrite && !v.archivedAt ? (
                          <button
                            type="button"
                            className="min-w-0 flex-1 text-left"
                            onClick={() => {
                              setFormError(null);
                              setFormVendor(v);
                            }}
                          >
                            <p className="font-medium text-gray-900 dark:text-white">{v.name}</p>
                            <p className="text-xs text-gray-500">{[v.code, v.email].filter(Boolean).join(' · ') || '—'}</p>
                          </button>
                        ) : (
                          <div className="min-w-0 flex-1">
                            <p className="font-medium text-gray-900 dark:text-white">{v.name}</p>
                            <p className="text-xs text-gray-500">{[v.code, v.email].filter(Boolean).join(' · ') || '—'}</p>
                          </div>
                        )}
                        {canWrite && !v.archivedAt ? (
                          <button
                            type="button"
                            className="rounded-lg p-1.5 text-gray-500"
                            aria-label={t('acquisitions.vendors.archive')}
                            onClick={() => setArchiveVendor(v)}
                          >
                            <Archive className="h-4 w-4" />
                          </button>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                )
              }
            />
            <div className="p-4">
              <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
            </div>
          </>
        )}
      </Card>

      {formVendor !== undefined ? (
        <VendorFormModal
          key={formVendor?.id ?? 'new'}
          vendor={formVendor}
          isOpen
          isSaving={saveMutation.isPending}
          error={formError}
          onClose={() => { if (!saveMutation.isPending) setFormVendor(undefined); }}
          onSubmit={(data) => saveMutation.mutate(data)}
        />
      ) : null}

      <ConfirmDialog
        isOpen={archiveVendor != null}
        onClose={() => { if (!archiveMutation.isPending) setArchiveVendor(null); }}
        onConfirm={() => archiveVendor && archiveMutation.mutate(archiveVendor.id)}
        title={t('acquisitions.vendors.archiveTitle')}
        message={t('acquisitions.vendors.archiveConfirm', { name: archiveVendor?.name ?? '' })}
        confirmLabel={t('acquisitions.vendors.archive')}
        confirmVariant="danger"
        isLoading={archiveMutation.isPending}
      />
    </div>
  );
}
