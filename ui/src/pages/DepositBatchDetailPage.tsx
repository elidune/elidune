import { useEffect, useRef, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Library } from 'lucide-react';
import { Badge, Button, Card, CardHeader, ConfirmDialog, Input, Modal, QueryErrorBanner, Table } from '@/components/common';
import ReturnToLenderNotice from '@/components/items/ReturnToLenderNotice';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import {
  canManageItems,
  type DepositBatchItem,
  type DepositCopyState,
  type Source,
} from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import {
  depositActiveLoanCopies,
  depositScanAlreadyReturned,
  formatCatalogDate,
  parseDepositCloseReport,
  returnToLenderNotice,
  type DepositCloseLine,
  type DepositCloseReport,
} from '@/utils/deposits';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const STATES: DepositCopyState[] = ['onShelf', 'onLoan', 'onHold', 'inTransit', 'returned', 'missing'];

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function printCloseReport(
  reference: string,
  report: DepositCloseReport,
  labels: { title: string; returned: string; missing: string; onLoan: string; barcode: string; document: string },
) {
  const popup = window.open('', '_blank', 'noopener,noreferrer');
  if (!popup) return;
  const section = (heading: string, lines: DepositCloseLine[]) => {
    const rows = lines
      .map((line) => `<tr><td>${escapeHtml(line.barcode ?? '')}</td><td>${escapeHtml(line.title ?? '')}</td></tr>`)
      .join('');
    return `<h2>${escapeHtml(heading)}</h2><table><thead><tr><th>${escapeHtml(labels.barcode)}</th><th>${escapeHtml(labels.document)}</th></tr></thead><tbody>${rows}</tbody></table>`;
  };
  popup.document.write(
    `<!DOCTYPE html><html><head><title>${escapeHtml(labels.title)}</title><style>body{font-family:sans-serif;padding:24px}table{border-collapse:collapse;width:100%;margin-bottom:24px}td,th{border:1px solid #ccc;padding:6px;text-align:left}</style></head><body><h1>${escapeHtml(reference)}</h1>${section(labels.returned, report.returned)}${section(labels.missing, report.missing)}${section(labels.onLoan, report.onLoan)}</body></html>`,
  );
  popup.document.close();
  popup.focus();
  popup.print();
}

export default function DepositBatchDetailPage() {
  const { id = '' } = useParams();
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageItems(user, api.getToken());
  const scanRef = useRef<HTMLInputElement>(null);
  const [state, setState] = useState<DepositCopyState>('onShelf');
  const [editing, setEditing] = useState(false);
  const [returnDueOn, setReturnDueOn] = useState('');
  const [siteId, setSiteId] = useState('');
  const [notes, setNotes] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const [barcode, setBarcode] = useState('');
  const [scanNotice, setScanNotice] = useState<string | null>(null);
  const [confirmClose, setConfirmClose] = useState(false);
  const [closeReport, setCloseReport] = useState<DepositCloseReport | null>(null);
  const [closeRefusal, setCloseRefusal] = useState<{ message: string; copies: DepositCloseLine[] } | null>(null);
  const [returnToLenderMessage, setReturnToLenderMessage] = useState<string | null>(null);
  const [returnedBarcodes, setReturnedBarcodes] = useState<Set<string>>(new Set());

  const detailQuery = useQuery({
    queryKey: ['deposit-batch', id],
    queryFn: () => api.getDepositBatch(id),
    enabled: id !== '',
  });
  const itemsQuery = useQuery({
    queryKey: ['deposit-batch', id, 'items', state],
    queryFn: () => api.listDepositBatchItems(id, state),
    enabled: id !== '',
  });
  const lendersQuery = useQuery({
    queryKey: ['lenders', 'names'],
    queryFn: () => api.listLenders(true),
  });
  const sourcesQuery = useQuery({
    queryKey: ['sources', 'active'],
    queryFn: () => api.getSources(false),
  });
  const onLoanQuery = useQuery({
    queryKey: ['deposit-batch', id, 'items', 'onLoan'],
    queryFn: () => api.listDepositBatchItems(id, 'onLoan'),
    enabled: id !== '' && detailQuery.data?.status === 'returning',
  });
  const onHoldQuery = useQuery({
    queryKey: ['deposit-batch', id, 'items', 'onHold'],
    queryFn: () => api.listDepositBatchItems(id, 'onHold'),
    enabled: id !== '' && detailQuery.data?.status === 'returning',
  });
  const returnedQuery = useQuery({
    queryKey: ['deposit-batch', id, 'items', 'returned'],
    queryFn: () => api.listDepositBatchItems(id, 'returned'),
    enabled: id !== '' && detailQuery.data?.status === 'returning',
  });

  const batch = detailQuery.data;
  const lenderRecordName = lendersQuery.data?.find((lender) => lender.id === batch?.lenderId)?.name ?? null;
  const lenderName = lenderRecordName ?? batch?.lenderId;
  const siteName = (site: string | undefined) =>
    sourcesQuery.data?.find((source) => String(source.id) === site)?.name ?? site ?? '—';

  useEffect(() => {
    if (batch?.status === 'returning') {
      scanRef.current?.focus();
    }
  }, [batch?.status]);

  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ['deposit-batch', id] });
  };

  const updateMutation = useMutation({
    mutationFn: () =>
      api.updateDepositBatch(id, {
        returnDueOn,
        siteId,
        notes: notes.trim() || null,
      }),
    onSuccess: () => {
      setEditing(false);
      void refresh();
      showToast({ variant: 'success', message: t('deposits.batches.saved') });
    },
    onError: (error: unknown) => setFormError(getApiErrorMessage(error, t)),
  });

  const startReturnMutation = useMutation({
    mutationFn: () => api.startDepositReturn(id),
    onSuccess: () => {
      void refresh();
      showToast({ variant: 'success', message: t('deposits.batch.returnStarted') });
      requestAnimationFrame(() => scanRef.current?.focus());
    },
    onError: (error: unknown) => showToast({ variant: 'error', message: getApiErrorMessage(error, t) }),
  });

  const scanMutation = useMutation({
    mutationFn: async (code: string) => {
      const listed = returnedBarcodes.has(code)
        || (returnedQuery.data ?? []).some((item) => (item.barcode ?? '').trim() === code);
      const body = await api.scanOutDepositCopy(id, code);
      return { body, already: depositScanAlreadyReturned(body, listed) };
    },
    onSuccess: ({ already, body }, code) => {
      setReturnedBarcodes((current) => new Set(current).add(code));
      const scanned = already
        ? t('deposits.batch.alreadyScanned', { barcode: code })
        : t('deposits.batch.scanned', { barcode: code });
      const lenderNotice = returnToLenderNotice(body, t, lenderRecordName);
      setReturnToLenderMessage(lenderNotice);
      setScanNotice(lenderNotice ? `${scanned} ${lenderNotice}` : scanned);
      setBarcode('');
      void refresh();
      requestAnimationFrame(() => scanRef.current?.focus());
    },
    onError: (error: unknown) => {
      setScanNotice(getApiErrorMessage(error, t));
      requestAnimationFrame(() => {
        scanRef.current?.focus();
        scanRef.current?.select();
      });
    },
  });

  const closeMutation = useMutation({
    mutationFn: () => api.closeDepositBatch(id),
    onSuccess: (body) => {
      setConfirmClose(false);
      setCloseRefusal(null);
      setCloseReport(parseDepositCloseReport(body));
      void refresh();
      showToast({ variant: 'success', message: t('deposits.batch.closed') });
    },
    onError: (error: unknown) => {
      const message = getApiErrorMessage(error, t);
      const fromError = depositActiveLoanCopies(error);
      const copies = fromError == null
        ? []
        : fromError.length > 0
          ? fromError
          : (onLoanQuery.data ?? []).map((item) => ({
              barcode: item.barcode?.trim() || null,
              title: item.title?.trim() || null,
            }));
      setConfirmClose(false);
      setCloseReport(null);
      setCloseRefusal({ message, copies });
      showToast({ variant: 'error', message });
    },
  });

  const openEdit = () => {
    if (!batch) return;
    setReturnDueOn(batch.returnDueOn);
    setSiteId(batch.siteId);
    setNotes(batch.notes ?? '');
    setFormError(null);
    setEditing(true);
  };

  const counts = batch?.counts;

  return (
    <div className={APP_PAGE_CLASS}>
      <div className="flex items-start gap-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-gray-100 dark:bg-gray-800">
          <Library className="h-6 w-6 text-gray-700 dark:text-gray-200" aria-hidden />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm">
            <Link to="/deposits" className="text-indigo-700 hover:underline dark:text-indigo-300">
              {t('deposits.batches.title')}
            </Link>
          </p>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            {batch?.reference ?? t('deposits.batches.title')}
          </h1>
          {batch ? (
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
              {lenderName} · {siteName(batch.siteId)} · {t('deposits.returnDueOn')}{' '}
              {formatCatalogDate(batch.returnDueOn, i18n.language)}
            </p>
          ) : null}
        </div>
        {batch ? (
          <Badge variant={batch.status === 'active' ? 'success' : batch.status === 'returning' ? 'warning' : 'default'}>
            {t(`deposits.status.${batch.status}`)}
          </Badge>
        ) : null}
      </div>

      {detailQuery.isError ? (
        <QueryErrorBanner message={getApiErrorMessage(detailQuery.error, t)} onRetry={() => void detailQuery.refetch()} />
      ) : null}

      {batch && counts ? (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
          {([
            ['total', counts.total],
            ['onShelf', counts.onShelf],
            ['onLoan', counts.onLoan],
            ['onHold', counts.onHold],
            ['inTransit', counts.inTransit],
            ['returned', counts.returned],
          ] as const).map(([key, value]) => (
            <Card key={key}>
              <p className="text-xs text-gray-500 dark:text-gray-400">{t(`deposits.counts.${key}`)}</p>
              <p className="mt-1 text-2xl font-semibold text-gray-900 dark:text-white">{value}</p>
            </Card>
          ))}
        </div>
      ) : null}

      {batch?.notes ? (
        <Card>
          <CardHeader title={t('deposits.notes')} />
          <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{batch.notes}</p>
        </Card>
      ) : null}

      {canWrite && batch?.status === 'active' ? (
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="secondary" onClick={openEdit}>{t('common.edit')}</Button>
          <Button type="button" onClick={() => startReturnMutation.mutate()} isLoading={startReturnMutation.isPending}>
            {t('deposits.batch.startReturn')}
          </Button>
        </div>
      ) : null}

      {canWrite && batch?.status === 'returning' ? (
        <Card>
          <CardHeader title={t('deposits.batch.scanTitle')} subtitle={t('deposits.batch.scanHint')} />
          <form
            className="flex flex-col gap-3 sm:flex-row sm:items-end"
            onSubmit={(event) => {
              event.preventDefault();
              const code = barcode.trim();
              if (!code || scanMutation.isPending) return;
              scanMutation.mutate(code);
            }}
          >
            <div className="min-w-0 flex-1">
              <Input
                ref={scanRef}
                label={t('items.barcode')}
                value={barcode}
                autoFocus
                onChange={(event) => setBarcode(event.target.value)}
              />
            </div>
            <Button type="submit" isLoading={scanMutation.isPending} disabled={!barcode.trim()}>
              {t('deposits.batch.scanOut')}
            </Button>
          </form>
          {returnToLenderMessage ? (
            <div className="mt-3">
              <ReturnToLenderNotice message={returnToLenderMessage} />
            </div>
          ) : null}
          {scanNotice ? <p className="mt-3 text-sm text-gray-700 dark:text-gray-300">{scanNotice}</p> : null}
          <div className="mt-6 grid gap-4 lg:grid-cols-2">
            <CopyList title={t('deposits.batch.stillOnLoan')} items={onLoanQuery.data ?? []} empty={t('deposits.batch.none')} />
            <CopyList title={t('deposits.batch.stillOnHold')} items={onHoldQuery.data ?? []} empty={t('deposits.batch.none')} />
          </div>
          {counts && counts.onLoan > 0 ? (
            <p className="mt-4 text-sm font-medium text-amber-800 dark:text-amber-200" role="status">
              {t('deposits.batch.closeWithLoans', { count: counts.onLoan })}
            </p>
          ) : null}
          {closeRefusal ? (
            <div className="mt-4 rounded-lg border border-red-300 bg-red-50 p-4 text-red-900 dark:border-red-800 dark:bg-red-950/40 dark:text-red-100" role="alert">
              <p className="text-sm font-semibold">{closeRefusal.message}</p>
              {closeRefusal.copies.length > 0 ? (
                <ul className="mt-2 space-y-1 text-sm">
                  {closeRefusal.copies.map((line) => (
                    <li key={`${line.barcode ?? ''}:${line.title ?? ''}`}>
                      <span className="font-mono">{line.barcode || '—'}</span>
                      {line.title ? ` · ${line.title}` : ''}
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
          <div className="mt-4">
            <Button type="button" variant="danger" onClick={() => setConfirmClose(true)}>
              {t('deposits.batch.close')}
            </Button>
          </div>
        </Card>
      ) : null}

      {closeReport ? (
        <Card>
          <CardHeader
            title={t('deposits.batch.reportTitle')}
            action={
              <Button
                type="button"
                variant="secondary"
                onClick={() =>
                  printCloseReport(batch?.reference ?? '', closeReport, {
                    title: t('deposits.batch.reportTitle'),
                    returned: t('deposits.batch.returnedCopies'),
                    missing: t('deposits.batch.missingCopies'),
                    onLoan: t('deposits.batch.stillOnLoan'),
                    barcode: t('items.barcode'),
                    document: t('loans.document'),
                  })
                }
              >
                {t('deposits.batch.print')}
              </Button>
            }
          />
          <ReportTable title={t('deposits.batch.returnedCopies')} lines={closeReport.returned} />
          <ReportTable title={t('deposits.batch.missingCopies')} lines={closeReport.missing} />
          <ReportTable title={t('deposits.batch.stillOnLoan')} lines={closeReport.onLoan} />
        </Card>
      ) : null}

      <Card className="overflow-hidden">
        <div className="flex flex-wrap gap-2 px-4 py-3">
          {STATES.map((value) => (
            <Button
              key={value}
              type="button"
              size="sm"
              variant={state === value ? 'primary' : 'secondary'}
              onClick={() => setState(value)}
            >
              {t(`deposits.copyState.${value}`)}
            </Button>
          ))}
        </div>
        {itemsQuery.isError ? (
          <QueryErrorBanner message={getApiErrorMessage(itemsQuery.error, t)} onRetry={() => void itemsQuery.refetch()} />
        ) : null}
        <Table
          columns={[
            { key: 'barcode', header: t('items.barcode'), render: (item: DepositBatchItem) => item.barcode || '—' },
            { key: 'title', header: t('loans.document'), render: (item: DepositBatchItem) => item.title || '—' },
            {
              key: 'record',
              header: '',
              align: 'right',
              render: (item: DepositBatchItem) => (
                <Link to={`/biblios/${item.biblioId}`} className="text-sm text-indigo-700 hover:underline dark:text-indigo-300" data-row-action>
                  {t('deposits.batch.openRecord')}
                </Link>
              ),
            },
          ]}
          data={itemsQuery.data ?? []}
          keyExtractor={(item) => item.itemId}
          isLoading={itemsQuery.isLoading}
          emptyMessage={t('deposits.batch.none')}
        />
      </Card>

      <Modal
        isOpen={editing}
        onClose={() => setEditing(false)}
        title={t('deposits.batches.edit')}
        footer={
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => setEditing(false)}>{t('common.cancel')}</Button>
            <Button type="button" isLoading={updateMutation.isPending} onClick={() => updateMutation.mutate()}>
              {t('common.save')}
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <p className="text-sm text-gray-600 dark:text-gray-300">{t('deposits.batches.siteDoesNotMoveCopies')}</p>
          <div>
            <label htmlFor="edit-site" className={formLabelClass()}>{t('deposits.site')}</label>
            <select id="edit-site" className={formControlClass()} value={siteId} onChange={(event) => setSiteId(event.target.value)}>
              {(sourcesQuery.data ?? []).map((source: Source) => (
                <option key={source.id} value={String(source.id)}>{source.name || source.id}</option>
              ))}
            </select>
          </div>
          <Input label={t('deposits.returnDueOn')} type="date" value={returnDueOn} onChange={(event) => setReturnDueOn(event.target.value)} />
          <Input label={t('deposits.notes')} value={notes} onChange={(event) => setNotes(event.target.value)} />
          {formError ? <p className="text-sm text-red-600 dark:text-red-400">{formError}</p> : null}
        </div>
      </Modal>

      <ConfirmDialog
        isOpen={confirmClose}
        onClose={() => setConfirmClose(false)}
        onConfirm={() => closeMutation.mutate()}
        title={t('deposits.batch.close')}
        message={
          counts && counts.onLoan > 0
            ? t('deposits.batch.closeWithLoans', { count: counts.onLoan })
            : t('deposits.batch.closeConfirm')
        }
        confirmLabel={t('deposits.batch.close')}
        confirmVariant="danger"
        isLoading={closeMutation.isPending}
      />
    </div>
  );
}

function CopyList({ title, items, empty }: { title: string; items: DepositBatchItem[]; empty: string }) {
  return (
    <div>
      <h2 className="text-sm font-semibold text-gray-900 dark:text-white">{title}</h2>
      {items.length === 0 ? (
        <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">{empty}</p>
      ) : (
        <ul className="mt-2 space-y-1 text-sm text-gray-700 dark:text-gray-300">
          {items.map((item) => (
            <li key={item.itemId}>
              <span className="font-mono">{item.barcode || '—'}</span>
              {item.title ? ` · ${item.title}` : ''}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ReportTable({ title, lines }: { title: string; lines: DepositCloseReport['returned'] }) {
  const { t } = useTranslation();
  return (
    <div className="mt-4">
      <h2 className="mb-2 text-sm font-semibold text-gray-900 dark:text-white">{title}</h2>
      <Table
        columns={[
          { key: 'barcode', header: t('items.barcode'), render: (line) => line.barcode || '—' },
          { key: 'title', header: t('loans.document'), render: (line) => line.title || '—' },
        ]}
        data={lines}
        keyExtractor={(line) => `${line.barcode ?? ''}:${line.title ?? ''}`}
        emptyMessage={t('deposits.batch.none')}
      />
    </div>
  );
}
