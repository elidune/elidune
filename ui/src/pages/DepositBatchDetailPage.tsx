import { useEffect, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronDown, Library } from 'lucide-react';
import { Badge, Button, Card, CardHeader, ConfirmDialog, Input, Modal, QueryErrorBanner, Table } from '@/components/common';
import { MarcImportWorkflow } from '@/pages/ImportIsoPage';
import ReturnToLenderNotice from '@/components/items/ReturnToLenderNotice';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import {
  canManageItems,
  type DepositBatchItem,
  type DepositCopyState,
  type Site,
} from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import {
  activeLoanCopyLabel,
  depositActiveLoanCopies,
  depositScanAlreadyReturned,
  formatCatalogDate,
  parseDepositCloseReport,
  returnToLenderNotice,
  type DepositActiveLoanCopy,
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
  labels: { title: string; returned: string; missing: string; barcode: string; document: string },
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
    `<!DOCTYPE html><html><head><title>${escapeHtml(labels.title)}</title><style>body{font-family:sans-serif;padding:24px}table{border-collapse:collapse;width:100%;margin-bottom:24px}td,th{border:1px solid #ccc;padding:6px;text-align:left}</style></head><body><h1>${escapeHtml(reference)}</h1>${section(labels.returned, report.returned)}${section(labels.missing, report.missing)}</body></html>`,
  );
  popup.document.close();
  popup.focus();
  popup.print();
}

export default function DepositBatchDetailPage() {
  const { id = '' } = useParams();
  const navigate = useNavigate();
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
  const [closeRefusal, setCloseRefusal] = useState<{ message: string; copies: DepositActiveLoanCopy[] } | null>(null);
  const [returnToLenderMessage, setReturnToLenderMessage] = useState<string | null>(null);
  const [returnedBarcodes, setReturnedBarcodes] = useState<Set<string>>(new Set());
  const [marcImportOpen, setMarcImportOpen] = useState(false);
  const marcImportPanelId = `deposit-marc-import-${id}`;

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
  const sitesQuery = useQuery({
    queryKey: ['sites', 'active'],
    queryFn: () => api.getSites(false),
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
    sitesQuery.data?.find((source) => String(source.id) === site)?.name ?? site ?? '—';

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
        ...(returnDueOn.trim() ? { returnDueOn: returnDueOn.trim() } : {}),
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

  const [confirmCancelReturn, setConfirmCancelReturn] = useState(false);

  const cancelReturnMutation = useMutation({
    mutationFn: () => api.cancelDepositReturn(id),
    onSuccess: () => {
      setConfirmCancelReturn(false);
      setScanNotice(null);
      setReturnToLenderMessage(null);
      setReturnedBarcodes(new Set());
      void refresh();
      void queryClient.invalidateQueries({ queryKey: ['deposit-batch', id, 'items'] });
      showToast({ variant: 'success', message: t('deposits.batch.returnCancelled') });
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
      const copies = depositActiveLoanCopies(error) ?? [];
      setConfirmClose(false);
      setCloseReport(null);
      setCloseRefusal({ message, copies });
      showToast({ variant: 'error', message });
    },
  });

  const openEdit = () => {
    if (!batch) return;
    setReturnDueOn(batch.returnDueOn ?? '');
    setSiteId(batch.siteId);
    setNotes(batch.notes ?? '');
    setFormError(null);
    setEditing(true);
  };

  const counts = batch?.counts;

  return (
    <div className={APP_PAGE_CLASS}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-gray-100 dark:bg-gray-800">
            <Library className="h-6 w-6 text-gray-700 dark:text-gray-200" aria-hidden />
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm">
              <Link to="/deposits" className="text-indigo-700 hover:underline dark:text-indigo-300">
                {t('deposits.batches.title')}
              </Link>
            </p>
            <div className="flex flex-wrap items-center gap-2">
              <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
                {batch?.reference ?? t('deposits.batches.title')}
              </h1>
              {batch ? (
                <Badge variant={batch.status === 'active' ? 'success' : batch.status === 'returning' ? 'warning' : 'default'}>
                  {t(`deposits.status.${batch.status}`)}
                </Badge>
              ) : null}
            </div>
            {batch ? (
              <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
                {lenderName} · {siteName(batch.siteId)} · {t('deposits.returnDueOn')}{' '}
                {batch.returnDueOn
                  ? formatCatalogDate(batch.returnDueOn, i18n.language)
                  : t('deposits.returnDueOpen')}
              </p>
            ) : null}
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-2">
          {canWrite && batch?.status === 'active' ? (
            <>
              <Button type="button" variant="secondary" onClick={openEdit}>
                {t('common.edit')}
              </Button>
              <Button
                type="button"
                variant={marcImportOpen ? 'primary' : 'secondary'}
                aria-expanded={marcImportOpen}
                aria-pressed={marcImportOpen}
                aria-controls={marcImportPanelId}
                rightIcon={
                  <ChevronDown
                    className={`h-4 w-4 transition-transform duration-200 ${marcImportOpen ? 'rotate-180' : ''}`}
                    aria-hidden
                  />
                }
                onClick={() => setMarcImportOpen((open) => !open)}
              >
                {t('deposits.batch.importNotices')}
              </Button>
              <Button type="button" onClick={() => startReturnMutation.mutate()} isLoading={startReturnMutation.isPending}>
                {t('deposits.batch.startReturn')}
              </Button>
            </>
          ) : null}
        </div>
      </div>

      {canWrite && batch?.status === 'active' && marcImportOpen ? (
        <div
          id={marcImportPanelId}
          className="w-full rounded-lg border border-amber-200/80 bg-white p-4 shadow-sm dark:border-amber-900/50 dark:bg-gray-900"
        >
          <p className="mb-3 text-sm text-gray-600 dark:text-gray-300">{t('deposits.batch.marcImportSubtitle')}</p>
          <MarcImportWorkflow
            layout="embedded"
            showDepositHeader={false}
            depositContext={{
              batchId: batch.id,
              siteId: batch.siteId,
              reference: batch.reference,
            }}
            onDepositItemsChanged={() => {
              void queryClient.invalidateQueries({ queryKey: ['deposit-batch', id] });
              void queryClient.invalidateQueries({ queryKey: ['deposit-batch', id, 'items'] });
            }}
          />
        </div>
      ) : null}

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
                  {closeRefusal.copies.map((line) => {
                    const known = activeLoanCopyLabel(line, onLoanQuery.data);
                    return (
                      <li key={line.itemId}>
                        <span className="font-mono">{line.barcode || '—'}</span>
                        {known.title ? ` · ${known.title}` : ''}
                        {' · '}
                        <ItemRecordLink
                          itemId={line.itemId}
                          biblioId={known.biblioId}
                          label={t('deposits.batch.openRecord')}
                          onOpen={(biblioId) => navigate(`/biblios/${biblioId}`)}
                          onError={(error) => showToast({ variant: 'error', message: getApiErrorMessage(error, t) })}
                        />
                      </li>
                    );
                  })}
                </ul>
              ) : null}
            </div>
          ) : null}
          <div className="mt-4 flex flex-wrap gap-2">
            <Button
              type="button"
              variant="secondary"
              isLoading={cancelReturnMutation.isPending}
              onClick={() => setConfirmCancelReturn(true)}
            >
              {t('deposits.batch.cancelReturn')}
            </Button>
            <Button
              type="button"
              variant="danger"
              disabled={(counts?.onLoan ?? 0) > 0}
              onClick={() => setConfirmClose(true)}
            >
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
              {(sitesQuery.data ?? []).map((source: Site) => (
                <option key={source.id} value={String(source.id)}>{source.name || source.id}</option>
              ))}
            </select>
          </div>
          <Input
            label={t('deposits.returnDueOn')}
            type="date"
            value={returnDueOn}
            onChange={(event) => setReturnDueOn(event.target.value)}
            hint={t('deposits.returnDueOnOptionalHint')}
          />
          <Input label={t('deposits.notes')} value={notes} onChange={(event) => setNotes(event.target.value)} />
          {formError ? <p className="text-sm text-red-600 dark:text-red-400">{formError}</p> : null}
        </div>
      </Modal>

      <ConfirmDialog
        isOpen={confirmClose}
        onClose={() => setConfirmClose(false)}
        onConfirm={() => closeMutation.mutate()}
        title={t('deposits.batch.close')}
        message={t('deposits.batch.closeConfirm')}
        confirmLabel={t('deposits.batch.close')}
        confirmVariant="danger"
        isLoading={closeMutation.isPending}
      />

      <ConfirmDialog
        isOpen={confirmCancelReturn}
        onClose={() => setConfirmCancelReturn(false)}
        onConfirm={() => cancelReturnMutation.mutate()}
        title={t('deposits.batch.cancelReturn')}
        message={t('deposits.batch.cancelReturnConfirm')}
        confirmLabel={t('deposits.batch.cancelReturn')}
        isLoading={cancelReturnMutation.isPending}
      />
    </div>
  );
}

function ItemRecordLink({
  itemId,
  biblioId,
  label,
  onOpen,
  onError,
}: {
  itemId: string;
  biblioId: string | null;
  label: string;
  onOpen: (biblioId: string) => void;
  onError: (error: unknown) => void;
}) {
  const [pending, setPending] = useState(false);
  const className = 'text-indigo-800 underline hover:no-underline dark:text-indigo-200';
  if (biblioId) {
    return <Link to={`/biblios/${biblioId}`} className={className}>{label}</Link>;
  }
  return (
    <button
      type="button"
      className={className}
      disabled={pending}
      onClick={() => {
        setPending(true);
        void api.getItem(itemId).then((notice) => {
          if (notice.id) onOpen(String(notice.id));
        }).catch((error: unknown) => {
          onError(error);
        }).finally(() => setPending(false));
      }}
    >
      {label}
    </button>
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
