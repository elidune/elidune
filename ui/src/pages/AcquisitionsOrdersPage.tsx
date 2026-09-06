import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, ShoppingCart } from 'lucide-react';
import {
  Button,
  Card,
  Input,
  ListSkeleton,
  Modal,
  Pagination,
  QueryErrorBanner,
  ResponsiveRecordList,
  SearchInput,
  Table,
} from '@/components/common';
import AcquisitionsSubnav from '@/components/acquisitions/AcquisitionsSubnav';
import OrderStatusBadge from '@/components/acquisitions/OrderStatusBadge';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { ACQUISITIONS_ORDERS_KEY } from '@/hooks/acquisitions/queryKeys';
import { useFundsPickerQuery } from '@/hooks/acquisitions/useFundsQuery';
import { useOrdersQuery } from '@/hooks/acquisitions/useOrdersQuery';
import { useActiveVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import {
  canManageAcquisitions,
  type PurchaseOrder,
  type PurchaseOrderStatus,
} from '@/types';
import { ORDER_STATUSES } from '@/utils/acquisitionDisplay';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';

const PER_PAGE = 20;

export default function AcquisitionsOrdersPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageAcquisitions(user, api.getToken());

  const [q, setQ] = useState('');
  const [status, setStatus] = useState<PurchaseOrderStatus | ''>('');
  const [vendorId, setVendorId] = useState('');
  const [fundId, setFundId] = useState('');
  const [page, setPage] = useState(1);
  const [showCreate, setShowCreate] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createVendorId, setCreateVendorId] = useState('');
  const [createFundId, setCreateFundId] = useState('');
  const [createOrderNumber, setCreateOrderNumber] = useState('');
  const [createNotes, setCreateNotes] = useState('');

  const vendorsQuery = useActiveVendorsQuery(true);
  const fundsQuery = useFundsPickerQuery(true);
  const query = useOrdersQuery({
    q,
    status: status || undefined,
    vendorId,
    fundId,
    page,
    perPage: PER_PAGE,
  });
  const orders = query.data?.orders ?? [];
  const total = query.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  const createMutation = useMutation({
    mutationFn: () =>
      api.createPurchaseOrder({
        vendorId: createVendorId,
        fundId: createFundId || null,
        orderNumber: createOrderNumber.trim() || null,
        notes: createNotes.trim() || null,
      }),
    onSuccess: (detail) => {
      setShowCreate(false);
      setCreateError(null);
      void queryClient.invalidateQueries({ queryKey: ACQUISITIONS_ORDERS_KEY });
      showToast({ message: t('acquisitions.orders.created'), variant: 'success' });
      navigate(`/acquisitions/orders/${detail.order.id}`);
    },
    onError: (err) => setCreateError(getApiErrorMessage(err, t)),
  });

  const openCreate = () => {
    setCreateError(null);
    setCreateVendorId('');
    setCreateFundId('');
    setCreateOrderNumber('');
    setCreateNotes('');
    setShowCreate(true);
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('acquisitions.orders.title')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('acquisitions.orders.subtitle')}</p>
        </div>
        {canWrite ? (
          <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={openCreate}>
            {t('acquisitions.orders.create')}
          </Button>
        ) : null}
      </div>
      <AcquisitionsSubnav />

      <Card className="overflow-hidden">
        <div className="grid gap-3 p-4 md:grid-cols-4">
          <div className="md:col-span-2">
            <SearchInput
              value={q}
              onChange={(value) => {
                setQ(value);
                setPage(1);
              }}
              placeholder={t('acquisitions.orders.search')}
            />
          </div>
          <select
            className={formControlClass()}
            value={status}
            onChange={(e) => {
              setStatus(e.target.value as PurchaseOrderStatus | '');
              setPage(1);
            }}
            aria-label={t('common.status')}
          >
            <option value="">{t('acquisitions.orders.allStatuses')}</option>
            {ORDER_STATUSES.map((s) => (
              <option key={s} value={s}>
                {t(`acquisitions.statuses.${s}`)}
              </option>
            ))}
          </select>
          <select
            className={formControlClass()}
            value={vendorId}
            onChange={(e) => {
              setVendorId(e.target.value);
              setPage(1);
            }}
            aria-label={t('acquisitions.vendors.vendor')}
          >
            <option value="">{t('acquisitions.vendors.all')}</option>
            {(vendorsQuery.data?.vendors ?? []).map((v) => (
              <option key={v.id} value={v.id}>
                {v.name}
              </option>
            ))}
          </select>
          <select
            className={`${formControlClass()} md:col-span-2`}
            value={fundId}
            onChange={(e) => {
              setFundId(e.target.value);
              setPage(1);
            }}
            aria-label={t('acquisitions.funds.fund')}
          >
            <option value="">{t('acquisitions.funds.all')}</option>
            {(fundsQuery.data?.funds ?? []).map((f) => (
              <option key={f.id} value={f.id}>
                {f.code} · {f.fiscalYear}
              </option>
            ))}
          </select>
        </div>

        {query.isLoading && query.data === undefined ? (
          <ListSkeleton />
        ) : query.isError ? (
          <div className="p-4">
            <QueryErrorBanner
              message={getApiErrorMessage(query.error, t) || t('acquisitions.orders.errorLoad')}
              onRetry={() => void query.refetch()}
            />
          </div>
        ) : (
          <>
            <ResponsiveRecordList
              desktop={
                <Table
                  data={orders}
                  keyExtractor={(o) => o.id}
                  emptyMessage={t('acquisitions.orders.empty')}
                  onRowClick={(o) => navigate(`/acquisitions/orders/${o.id}`)}
                  columns={[
                    { key: 'number', header: t('acquisitions.orders.number'), render: (o) => o.orderNumber },
                    { key: 'vendor', header: t('acquisitions.vendors.vendor'), render: (o) => o.vendorName || o.vendorId },
                    { key: 'fund', header: t('acquisitions.funds.fund'), render: (o) => o.fundCode || '—' },
                    {
                      key: 'status',
                      header: t('common.status'),
                      render: (o) => <OrderStatusBadge status={o.status} />,
                    },
                    {
                      key: 'updated',
                      header: t('acquisitions.updatedAt'),
                      render: (o) => new Date(o.updatedAt).toLocaleDateString(i18n.language),
                    },
                  ]}
                />
              }
              mobile={
                orders.length === 0 ? (
                  <p className="px-4 py-8 text-center text-sm text-gray-500">{t('acquisitions.orders.empty')}</p>
                ) : (
                  <ul className="divide-y divide-gray-100 dark:divide-gray-800">
                    {orders.map((o: PurchaseOrder) => (
                      <li key={o.id}>
                        <button
                          type="button"
                          className="flex w-full items-start gap-3 px-4 py-3 text-left"
                          onClick={() => navigate(`/acquisitions/orders/${o.id}`)}
                        >
                          <ShoppingCart className="mt-0.5 h-5 w-5 shrink-0 text-amber-600" />
                          <div className="min-w-0 flex-1">
                            <p className="font-medium text-gray-900 dark:text-white">{o.orderNumber}</p>
                            <p className="text-xs text-gray-500">
                              {o.vendorName || o.vendorId}
                              {o.fundCode ? ` · ${o.fundCode}` : ''}
                            </p>
                          </div>
                          <OrderStatusBadge status={o.status} />
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

      <Modal
        isOpen={showCreate}
        onClose={() => { if (!createMutation.isPending) setShowCreate(false); }}
        title={t('acquisitions.orders.createTitle')}
        size="md"
        footer={
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={() => setShowCreate(false)} disabled={createMutation.isPending}>
              {t('common.cancel')}
            </Button>
            <Button
              type="submit"
              form="create-order-form"
              isLoading={createMutation.isPending}
              disabled={!createVendorId}
            >
              {t('acquisitions.orders.create')}
            </Button>
          </div>
        }
      >
        <form
          id="create-order-form"
          className="space-y-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (!createVendorId || createMutation.isPending) return;
            createMutation.mutate();
          }}
        >
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('acquisitions.orders.createHelp')}</p>
          <div>
            <label htmlFor="create-vendor" className={formLabelClass()}>
              {t('acquisitions.vendors.vendor')}
            </label>
            <select
              id="create-vendor"
              className={formControlClass()}
              value={createVendorId}
              onChange={(e) => setCreateVendorId(e.target.value)}
              required
            >
              <option value="">{t('common.select')}</option>
              {(vendorsQuery.data?.vendors ?? []).map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label htmlFor="create-fund" className={formLabelClass()}>
              {t('acquisitions.funds.fund')}
            </label>
            <select
              id="create-fund"
              className={formControlClass()}
              value={createFundId}
              onChange={(e) => setCreateFundId(e.target.value)}
            >
              <option value="">{t('acquisitions.funds.none')}</option>
              {(fundsQuery.data?.funds ?? []).map((f) => (
                <option key={f.id} value={f.id}>
                  {f.code} · {f.fiscalYear} · {f.name}
                </option>
              ))}
            </select>
          </div>
          <Input
            label={t('acquisitions.orders.number')}
            value={createOrderNumber}
            onChange={(e) => setCreateOrderNumber(e.target.value)}
            placeholder={t('acquisitions.orders.numberPlaceholder')}
          />
          <div>
            <label htmlFor="create-notes" className={formLabelClass()}>
              {t('acquisitions.orders.notes')}
            </label>
            <textarea
              id="create-notes"
              className={formTextareaClass()}
              rows={2}
              value={createNotes}
              onChange={(e) => setCreateNotes(e.target.value)}
            />
          </div>
          {createError ? (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {createError}
            </p>
          ) : null}
        </form>
      </Modal>
    </div>
  );
}
