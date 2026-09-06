import { useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ChevronLeft, PackagePlus, Pencil, Plus, Trash2 } from 'lucide-react';
import {
  Button,
  Card,
  ConfirmDialog,
  Input,
  ListSkeleton,
  Modal,
  QueryErrorBanner,
  Table,
} from '@/components/common';
import AcquisitionsSubnav from '@/components/acquisitions/AcquisitionsSubnav';
import OrderAuditPanel from '@/components/acquisitions/OrderAuditPanel';
import OrderLineEditor from '@/components/acquisitions/OrderLineEditor';
import OrderStatusBadge from '@/components/acquisitions/OrderStatusBadge';
import ReceiveOrderDialog from '@/components/acquisitions/ReceiveOrderDialog';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import {
  ACQUISITIONS_FUNDS_KEY,
  ACQUISITIONS_ORDERS_KEY,
  orderDetailQueryKey,
} from '@/hooks/acquisitions/queryKeys';
import { useFundsPickerQuery } from '@/hooks/acquisitions/useFundsQuery';
import { useOrderDetailQuery } from '@/hooks/acquisitions/useOrdersQuery';
import { useActiveVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import {
  canManageAcquisitions,
  type CreateOrderLine,
  type PurchaseOrderLine,
  type ReceivePurchaseOrder,
} from '@/types';
import {
  canCancelOrder,
  canEditOrder,
  canReceiveOrder,
  canSubmitOrder,
  formatMoney,
  lineIntentLabel,
  lineUpdateNeedsRecreate,
  quantityRemaining,
} from '@/utils/acquisitionDisplay';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';

export default function AcquisitionsOrderDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageAcquisitions(user, api.getToken());

  const query = useOrderDetailQuery(id);
  const vendorsQuery = useActiveVendorsQuery(canWrite);
  const fundsQuery = useFundsPickerQuery(canWrite);
  const order = query.data?.order;
  const lines = query.data?.lines ?? [];
  const editable = Boolean(order && canEditOrder(order.status) && canWrite);

  const [headerVendorId, setHeaderVendorId] = useState<string | null>(null);
  const [headerFundId, setHeaderFundId] = useState<string | null>(null);
  const [headerNumber, setHeaderNumber] = useState<string | null>(null);
  const [headerNotes, setHeaderNotes] = useState<string | null>(null);
  const [headerError, setHeaderError] = useState<string | null>(null);

  const [lineEditor, setLineEditor] = useState<PurchaseOrderLine | null | undefined>(undefined);
  const [lineError, setLineError] = useState<string | null>(null);
  const [deleteLine, setDeleteLine] = useState<PurchaseOrderLine | null>(null);
  const [confirmSubmit, setConfirmSubmit] = useState(false);
  const [confirmCancel, setConfirmCancel] = useState(false);
  const [showReceive, setShowReceive] = useState(false);
  const [receiveError, setReceiveError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const invalidate = async () => {
    if (!id) return;
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: orderDetailQueryKey(id) }),
      queryClient.invalidateQueries({ queryKey: ACQUISITIONS_ORDERS_KEY }),
      queryClient.invalidateQueries({ queryKey: ACQUISITIONS_FUNDS_KEY }),
    ]);
  };

  const headerMutation = useMutation({
    mutationFn: () =>
      api.updatePurchaseOrder(id!, {
        vendorId: (headerVendorId ?? order?.vendorId) || undefined,
        fundId: headerFundId === '' ? null : (headerFundId ?? order?.fundId ?? null),
        orderNumber: (headerNumber ?? order?.orderNumber) || null,
        notes: headerNotes === null ? order?.notes ?? null : headerNotes.trim() || null,
      }),
    onSuccess: () => {
      setHeaderError(null);
      void invalidate();
      showToast({ message: t('acquisitions.orders.saved'), variant: 'success' });
    },
    onError: (err) => setHeaderError(getApiErrorMessage(err, t)),
  });

  const lineMutation = useMutation({
    mutationFn: async (data: CreateOrderLine) => {
      if (!lineEditor) return api.addPurchaseOrderLine(id!, data);
      if (lineUpdateNeedsRecreate(lineEditor, data)) {
        await api.deletePurchaseOrderLine(id!, lineEditor.id);
        return api.addPurchaseOrderLine(id!, data);
      }
      return api.updatePurchaseOrderLine(id!, lineEditor.id, data);
    },
    onSuccess: () => {
      setLineEditor(undefined);
      setLineError(null);
      void invalidate();
      showToast({ message: t('acquisitions.lines.saved'), variant: 'success' });
    },
    onError: (err) => setLineError(getApiErrorMessage(err, t)),
  });

  const deleteLineMutation = useMutation({
    mutationFn: (lineId: string) => api.deletePurchaseOrderLine(id!, lineId),
    onSuccess: () => {
      setDeleteLine(null);
      void invalidate();
      showToast({ message: t('acquisitions.lines.removed'), variant: 'success' });
    },
    onError: (err) => showToast({ message: getApiErrorMessage(err, t), variant: 'error' }),
  });

  const submitMutation = useMutation({
    mutationFn: () => api.submitPurchaseOrder(id!),
    onSuccess: () => {
      setConfirmSubmit(false);
      setActionError(null);
      void invalidate();
      showToast({ message: t('acquisitions.orders.submitted'), variant: 'success' });
    },
    onError: (err) => setActionError(getApiErrorMessage(err, t)),
  });

  const cancelMutation = useMutation({
    mutationFn: () => api.cancelPurchaseOrder(id!),
    onSuccess: () => {
      setConfirmCancel(false);
      setActionError(null);
      void invalidate();
      showToast({ message: t('acquisitions.orders.cancelled'), variant: 'success' });
    },
    onError: (err) => setActionError(getApiErrorMessage(err, t)),
  });

  const receiveMutation = useMutation({
    mutationFn: (data: ReceivePurchaseOrder) => api.receivePurchaseOrder(id!, data),
    onSuccess: (result) => {
      setShowReceive(false);
      setReceiveError(null);
      void invalidate();
      const count = result.lines.reduce((n, line) => n + (line.itemIds?.length ?? 0), 0);
      showToast({
        message: t('acquisitions.receive.success', { count }),
        variant: 'success',
      });
    },
    onError: (err) => setReceiveError(getApiErrorMessage(err, t)),
  });

  const busy =
    headerMutation.isPending ||
    lineMutation.isPending ||
    deleteLineMutation.isPending ||
    submitMutation.isPending ||
    cancelMutation.isPending ||
    receiveMutation.isPending;

  if (query.isLoading && query.data === undefined) {
    return (
      <div className="flex flex-col gap-4">
        <AcquisitionsSubnav />
        <ListSkeleton rows={6} />
      </div>
    );
  }

  if (query.isError || !order) {
    return (
      <div className="flex flex-col gap-4">
        <AcquisitionsSubnav />
        <QueryErrorBanner
          message={getApiErrorMessage(query.error, t) || t('acquisitions.orders.errorLoad')}
          onRetry={() => void query.refetch()}
        />
      </div>
    );
  }

  const vendorValue = headerVendorId ?? order.vendorId;
  const fundValue = headerFundId ?? order.fundId ?? '';
  const numberValue = headerNumber ?? order.orderNumber;
  const notesValue = headerNotes ?? order.notes ?? '';

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <Button type="button" variant="secondary" size="sm" leftIcon={<ChevronLeft className="h-4 w-4" />} onClick={() => navigate('/acquisitions/orders')}>
          {t('common.back')}
        </Button>
        <AcquisitionsSubnav />
      </div>

      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{order.orderNumber}</h1>
            <OrderStatusBadge status={order.status} />
          </div>
          <p className="mt-1 text-sm text-gray-500">
            {t('acquisitions.orders.detailSubtitle')}
          </p>
        </div>
        {canWrite ? (
          <div className="flex flex-wrap gap-2">
            {canSubmitOrder(order.status) ? (
              <Button type="button" disabled={busy || lines.length === 0} onClick={() => { setActionError(null); setConfirmSubmit(true); }}>
                {t('acquisitions.orders.submit')}
              </Button>
            ) : null}
            {canReceiveOrder(order.status) ? (
              <Button
                type="button"
                leftIcon={<PackagePlus className="h-4 w-4" />}
                disabled={busy}
                onClick={() => { setReceiveError(null); setShowReceive(true); }}
              >
                {t('acquisitions.receive.action')}
              </Button>
            ) : null}
            {canCancelOrder(order.status) ? (
              <Button type="button" variant="danger" disabled={busy} onClick={() => { setActionError(null); setConfirmCancel(true); }}>
                {t('acquisitions.orders.cancel')}
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>

      {actionError ? (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">{actionError}</p>
      ) : null}

      <Card className="p-4 space-y-3">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('acquisitions.orders.header')}</h2>
        {editable ? (
          <form
            className="grid gap-3 sm:grid-cols-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (!busy) headerMutation.mutate();
            }}
          >
            <div>
              <label className={formLabelClass()} htmlFor="order-vendor">{t('acquisitions.vendors.vendor')}</label>
              <select
                id="order-vendor"
                className={formControlClass()}
                value={vendorValue}
                onChange={(e) => setHeaderVendorId(e.target.value)}
              >
                {(vendorsQuery.data?.vendors ?? []).map((v) => (
                  <option key={v.id} value={v.id}>{v.name}</option>
                ))}
                {!vendorsQuery.data?.vendors.some((v) => v.id === order.vendorId) ? (
                  <option value={order.vendorId}>{order.vendorName || order.vendorId}</option>
                ) : null}
              </select>
            </div>
            <div>
              <label className={formLabelClass()} htmlFor="order-fund">{t('acquisitions.funds.fund')}</label>
              <select
                id="order-fund"
                className={formControlClass()}
                value={fundValue}
                onChange={(e) => setHeaderFundId(e.target.value)}
              >
                <option value="">{t('acquisitions.funds.none')}</option>
                {(fundsQuery.data?.funds ?? []).map((f) => (
                  <option key={f.id} value={f.id}>{f.code} · {f.fiscalYear} · {f.name}</option>
                ))}
                {order.fundId && !(fundsQuery.data?.funds ?? []).some((f) => f.id === order.fundId) ? (
                  <option value={order.fundId}>{order.fundCode || order.fundId}</option>
                ) : null}
              </select>
            </div>
            <Input
              label={t('acquisitions.orders.number')}
              value={numberValue}
              onChange={(e) => setHeaderNumber(e.target.value)}
            />
            <div className="sm:col-span-2">
              <label className={formLabelClass()} htmlFor="order-notes">{t('acquisitions.orders.notes')}</label>
              <textarea
                id="order-notes"
                className={formTextareaClass()}
                rows={2}
                value={notesValue}
                onChange={(e) => setHeaderNotes(e.target.value)}
              />
            </div>
            {headerError ? <p role="alert" className="sm:col-span-2 text-sm text-red-600">{headerError}</p> : null}
            <div className="sm:col-span-2">
              <Button type="submit" isLoading={headerMutation.isPending} disabled={busy}>
                {t('common.save')}
              </Button>
            </div>
          </form>
        ) : (
          <dl className="grid gap-3 text-sm sm:grid-cols-2">
            <div>
              <dt className="text-gray-500">{t('acquisitions.vendors.vendor')}</dt>
              <dd className="text-gray-900 dark:text-white">{order.vendorName || order.vendorId}</dd>
            </div>
            <div>
              <dt className="text-gray-500">{t('acquisitions.funds.fund')}</dt>
              <dd className="text-gray-900 dark:text-white">{order.fundCode || '—'}</dd>
            </div>
            <div className="sm:col-span-2">
              <dt className="text-gray-500">{t('acquisitions.orders.notes')}</dt>
              <dd className="text-gray-900 dark:text-white whitespace-pre-wrap">{order.notes || '—'}</dd>
            </div>
          </dl>
        )}
      </Card>

      <Card className="overflow-hidden">
        <div className="flex items-center justify-between gap-3 p-4">
          <div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('acquisitions.lines.heading')}</h2>
            <p className="text-xs text-gray-500">{t('acquisitions.lines.help')}</p>
          </div>
          {editable ? (
            <Button type="button" size="sm" leftIcon={<Plus className="h-4 w-4" />} disabled={busy} onClick={() => { setLineError(null); setLineEditor(null); }}>
              {t('acquisitions.lines.add')}
            </Button>
          ) : null}
        </div>
        <Table
          data={lines}
          keyExtractor={(line) => line.id}
          emptyMessage={t('acquisitions.lines.empty')}
          columns={[
            {
              key: 'intent',
              header: t('acquisitions.lines.item'),
              render: (line) => (
                <div>
                  <p>{lineIntentLabel(line)}</p>
                  <p className="text-xs text-gray-500">
                    {line.biblioId ? (
                      <Link className="text-amber-700 hover:underline dark:text-amber-400" to={`/biblios/${line.biblioId}`}>
                        {t('acquisitions.lines.openBiblio')}
                      </Link>
                    ) : line.isbn ? (
                      t('acquisitions.lines.isbnIntent')
                    ) : (
                      t('acquisitions.lines.titleIntent')
                    )}
                  </p>
                </div>
              ),
            },
            {
              key: 'qty',
              header: t('acquisitions.lines.qty'),
              render: (line) => `${line.quantityReceived}/${line.quantityOrdered}`,
            },
            {
              key: 'price',
              header: t('acquisitions.lines.unitPrice'),
              align: 'right',
              render: (line) => formatMoney(line.unitPrice, line.currency, i18n.language),
            },
            {
              key: 'left',
              header: t('acquisitions.receive.remainingShort'),
              render: (line) => String(quantityRemaining(line)),
            },
            ...(editable
              ? [{
                  key: 'actions',
                  header: t('common.actions'),
                  align: 'right' as const,
                  render: (line: PurchaseOrderLine) => (
                    <div className="flex justify-end gap-1">
                      <button
                        type="button"
                        className="rounded-lg p-1.5 text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800"
                        aria-label={t('common.edit')}
                        disabled={busy}
                        onClick={() => { setLineError(null); setLineEditor(line); }}
                      >
                        <Pencil className="h-4 w-4" />
                      </button>
                      <button
                        type="button"
                        className="rounded-lg p-1.5 text-gray-500 hover:bg-red-50 hover:text-red-600"
                        aria-label={t('common.delete')}
                        disabled={busy}
                        onClick={() => setDeleteLine(line)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  ),
                }]
              : []),
          ]}
        />
      </Card>

      <Card className="p-4">
        <h2 className="mb-2 text-lg font-semibold text-gray-900 dark:text-white">{t('acquisitions.audit.title')}</h2>
        <p className="mb-3 text-xs text-gray-500">{t('acquisitions.audit.help')}</p>
        <OrderAuditPanel orderId={order.id} />
      </Card>

      {lineEditor !== undefined ? (
        <Modal
          isOpen
          onClose={() => { if (!lineMutation.isPending) setLineEditor(undefined); }}
          title={lineEditor ? t('acquisitions.lines.editTitle') : t('acquisitions.lines.addTitle')}
          size="lg"
        >
          <OrderLineEditor
            key={lineEditor?.id ?? 'new'}
            funds={fundsQuery.data?.funds ?? []}
            defaultFundId={order.fundId}
            initial={lineEditor}
            isSaving={lineMutation.isPending}
            error={lineError}
            onCancel={() => { if (!lineMutation.isPending) setLineEditor(undefined); }}
            onSubmit={(data) => lineMutation.mutate(data)}
          />
        </Modal>
      ) : null}

      {showReceive ? (
        <ReceiveOrderDialog
          key={order.id}
          isOpen
          lines={lines}
          isSaving={receiveMutation.isPending}
          error={receiveError}
          locale={i18n.language}
          onClose={() => { if (!receiveMutation.isPending) setShowReceive(false); }}
          onSubmit={(data) => receiveMutation.mutate(data)}
        />
      ) : null}

      <ConfirmDialog
        isOpen={deleteLine != null}
        onClose={() => { if (!deleteLineMutation.isPending) setDeleteLine(null); }}
        onConfirm={() => deleteLine && deleteLineMutation.mutate(deleteLine.id)}
        title={t('acquisitions.lines.removeTitle')}
        message={t('acquisitions.lines.removeConfirm', { title: deleteLine ? lineIntentLabel(deleteLine) : '' })}
        confirmLabel={t('common.delete')}
        confirmVariant="danger"
        isLoading={deleteLineMutation.isPending}
      />
      <ConfirmDialog
        isOpen={confirmSubmit}
        onClose={() => { if (!submitMutation.isPending) setConfirmSubmit(false); }}
        onConfirm={() => submitMutation.mutate()}
        title={t('acquisitions.orders.submitTitle')}
        message={t('acquisitions.orders.submitConfirm')}
        confirmLabel={t('acquisitions.orders.submit')}
        isLoading={submitMutation.isPending}
        error={actionError}
      />
      <ConfirmDialog
        isOpen={confirmCancel}
        onClose={() => { if (!cancelMutation.isPending) setConfirmCancel(false); }}
        onConfirm={() => cancelMutation.mutate()}
        title={t('acquisitions.orders.cancelTitle')}
        message={t('acquisitions.orders.cancelConfirm')}
        confirmLabel={t('acquisitions.orders.cancel')}
        confirmVariant="danger"
        isLoading={cancelMutation.isPending}
        error={actionError}
      />
    </div>
  );
}
