import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Wallet } from 'lucide-react';
import {
  Button,
  Card,
  Input,
  ListSkeleton,
  Pagination,
  QueryErrorBanner,
  ResponsiveRecordList,
  SearchInput,
  Table,
} from '@/components/common';
import AcquisitionsSubnav from '@/components/acquisitions/AcquisitionsSubnav';
import FundFormModal from '@/components/acquisitions/FundFormModal';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { ACQUISITIONS_FUNDS_KEY } from '@/hooks/acquisitions/queryKeys';
import { useFundsQuery } from '@/hooks/acquisitions/useFundsQuery';
import api from '@/services/api';
import { canManageAcquisitions, type AcquisitionFund, type CreateFund, type UpdateFund } from '@/types';
import { formatMoney } from '@/utils/acquisitionDisplay';
import { formControlClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';

const PER_PAGE = 20;

export default function AcquisitionsFundsPage() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageAcquisitions(user, api.getToken());

  const [q, setQ] = useState('');
  const [year, setYear] = useState('');
  const [page, setPage] = useState(1);
  const [formFund, setFormFund] = useState<AcquisitionFund | null | undefined>(undefined);
  const [formError, setFormError] = useState<string | null>(null);

  const fiscalYear = year.trim() ? Number(year) : undefined;
  const query = useFundsQuery({
    q,
    fiscalYear: Number.isInteger(fiscalYear) ? fiscalYear : undefined,
    page,
    perPage: PER_PAGE,
  });
  const funds = query.data?.funds ?? [];
  const total = query.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  const saveMutation = useMutation({
    mutationFn: async (data: CreateFund | UpdateFund) => {
      if (formFund) return api.updateFund(formFund.id, data);
      return api.createFund(data as CreateFund);
    },
    onSuccess: () => {
      setFormFund(undefined);
      setFormError(null);
      void queryClient.invalidateQueries({ queryKey: ACQUISITIONS_FUNDS_KEY });
      showToast({ message: t('acquisitions.funds.saved'), variant: 'success' });
    },
    onError: (err) => setFormError(getApiErrorMessage(err, t)),
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('acquisitions.funds.title')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('acquisitions.funds.subtitle')}</p>
        </div>
        {canWrite ? (
          <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => { setFormError(null); setFormFund(null); }}>
            {t('acquisitions.funds.create')}
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
              placeholder={t('acquisitions.funds.search')}
            />
          </div>
          <Input
            type="number"
            value={year}
            onChange={(e) => {
              setYear(e.target.value);
              setPage(1);
            }}
            placeholder={t('acquisitions.funds.fiscalYear')}
            className={formControlClass({ className: 'sm:w-36' })}
          />
        </div>

        {query.isLoading && query.data === undefined ? (
          <ListSkeleton />
        ) : query.isError ? (
          <div className="p-4">
            <QueryErrorBanner
              message={getApiErrorMessage(query.error, t) || t('acquisitions.funds.errorLoad')}
              onRetry={() => void query.refetch()}
            />
          </div>
        ) : (
          <>
            <ResponsiveRecordList
              desktop={
                <Table
                  data={funds}
                  keyExtractor={(f) => f.id}
                  emptyMessage={t('acquisitions.funds.empty')}
                  onRowClick={canWrite ? (f) => { setFormError(null); setFormFund(f); } : undefined}
                  columns={[
                    { key: 'code', header: t('acquisitions.funds.code'), render: (f) => f.code },
                    { key: 'name', header: t('acquisitions.funds.name'), render: (f) => f.name },
                    { key: 'year', header: t('acquisitions.funds.fiscalYear'), render: (f) => String(f.fiscalYear) },
                    {
                      key: 'allocated',
                      header: t('acquisitions.funds.allocated'),
                      align: 'right',
                      render: (f) => formatMoney(f.allocatedAmount, f.currency, i18n.language),
                    },
                    {
                      key: 'committed',
                      header: t('acquisitions.funds.committed'),
                      align: 'right',
                      render: (f) => formatMoney(f.committed, f.currency, i18n.language),
                    },
                    {
                      key: 'spent',
                      header: t('acquisitions.funds.spent'),
                      align: 'right',
                      render: (f) => formatMoney(f.spent, f.currency, i18n.language),
                    },
                    {
                      key: 'available',
                      header: t('acquisitions.funds.available'),
                      align: 'right',
                      render: (f) => formatMoney(f.available, f.currency, i18n.language),
                    },
                  ]}
                />
              }
              mobile={
                funds.length === 0 ? (
                  <p className="px-4 py-8 text-center text-sm text-gray-500">{t('acquisitions.funds.empty')}</p>
                ) : (
                  <ul className="divide-y divide-gray-100 dark:divide-gray-800">
                    {funds.map((f) => (
                      <li key={f.id}>
                        <button
                          type="button"
                          className="flex w-full items-start gap-3 px-4 py-3 text-left"
                          onClick={canWrite ? () => { setFormError(null); setFormFund(f); } : undefined}
                        >
                          <Wallet className="mt-0.5 h-5 w-5 shrink-0 text-amber-600" />
                          <div className="min-w-0 flex-1">
                            <p className="font-medium text-gray-900 dark:text-white">
                              {f.code} · {f.name}
                            </p>
                            <p className="text-xs text-gray-500">
                              {f.fiscalYear} · {t('acquisitions.funds.available')}:{' '}
                              {formatMoney(f.available, f.currency, i18n.language)}
                            </p>
                          </div>
                        </button>
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

      {formFund !== undefined ? (
        <FundFormModal
          key={formFund?.id ?? 'new'}
          fund={formFund}
          isOpen
          isSaving={saveMutation.isPending}
          error={formError}
          onClose={() => { if (!saveMutation.isPending) setFormFund(undefined); }}
          onSubmit={(data) => saveMutation.mutate(data)}
        />
      ) : null}
    </div>
  );
}
