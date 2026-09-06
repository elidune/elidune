import { useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Ban, Bookmark, Plus, Search, AlertCircle, Truck } from 'lucide-react';
import { Card, CardHeader, Button, Badge, Table, Input, Pagination, Modal, ConfirmDialog, ScrollableListRegion, ResponsiveRecordList, ListSkeleton, BarcodeScanField, Typeahead } from '@/components/common';
import HoldMobileCard from '@/components/holds/HoldMobileCard';
import HoldDocumentCell from '@/components/holds/HoldDocumentCell';
import HoldExpiresCell from '@/components/holds/HoldExpiresCell';
import HoldPickupCell from '@/components/holds/HoldPickupCell';
import HoldTransitActions from '@/components/holds/HoldTransitActions';
import PickupSiteSelect from '@/components/holds/PickupSiteSelect';
import TransitActionModals from '@/components/holds/TransitActionModals';
import { useActiveTransitsQuery } from '@/hooks/holds/useActiveTransitsQuery';
import { usePickupSiteDraft } from '@/hooks/holds/usePickupSiteDraft';
import { useTransitActions } from '@/hooks/holds/useTransitActions';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import type { Biblio, BiblioShort, Hold, UserShort } from '@/types';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import { formatUserShortName } from '@/utils/userDisplay';
import { useToast } from '@/contexts/ToastContext';
import { formChoiceLabelClass, formControlClass, formLabelClass } from '@/utils/formControl';
import {
  applyStaffHoldList,
  buildCreateHold,
  STAFF_HOLDS_FETCH_CAP,
  type HoldPlacementScope,
  type StaffHoldStatusFilter,
} from '@/utils/holdDisplay';
import HoldScopeBadge from '@/components/holds/HoldScopeBadge';
import { indexTransitsByHoldId } from '@/utils/transitDisplay';

function statusBadge(t: (k: string) => string, status: Hold['status']) {
  return (
    <Badge variant={status === 'ready' ? 'success' : 'default'}>{t(`holds.statuses.${status}`)}</Badge>
  );
}

export default function HoldsPage() {
  const { t, i18n } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [statusFilter, setStatusFilter] = useState<StaffHoldStatusFilter>('all');
  const [copyBarcode, setCopyBarcode] = useState('');
  const [barcodeLookupError, setBarcodeLookupError] = useState<string | null>(null);
  const barcodeLookupInFlightRef = useRef(false);

  const [biblioDraft, setBiblioDraft] = useState('');
  const [biblioResults, setBiblioResults] = useState<BiblioShort[]>([]);
  const [biblioSearching, setBiblioSearching] = useState(false);
  const biblioSeqRef = useRef(0);
  const [selectedBiblio, setSelectedBiblio] = useState<Biblio | null>(null);
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [holdScope, setHoldScope] = useState<HoldPlacementScope>('title');
  const [createNotes, setCreateNotes] = useState('');
  const { sources, pickupSiteId, setPickupSiteId, missing: pickupMissing } = usePickupSiteDraft(showCreateModal);
  const transitActions = useTransitActions();
  const activeTransitsQuery = useActiveTransitsQuery();
  const transitByHoldId = useMemo(
    () => indexTransitsByHoldId(activeTransitsQuery.data ?? []),
    [activeTransitsQuery.data],
  );

  const [createUserDraft, setCreateUserDraft] = useState('');
  const [createUserResults, setCreateUserResults] = useState<UserShort[]>([]);
  const [createUserSearching, setCreateUserSearching] = useState(false);
  const createUserSeqRef = useRef(0);
  const [selectedUserForCreate, setSelectedUserForCreate] = useState<UserShort | null>(null);

  const [listPage, setListPage] = useState(1);
  const [listPerPage, setListPerPage] = useState(50);
  const [prevHoldListControls, setPrevHoldListControls] = useState({ statusFilter, listPerPage });
  const [cancelHoldId, setCancelHoldId] = useState<string | null>(null);
  const [biblioPickError, setBiblioPickError] = useState<string | null>(null);

  if (statusFilter !== prevHoldListControls.statusFilter || listPerPage !== prevHoldListControls.listPerPage) {
    setPrevHoldListControls({ statusFilter, listPerPage });
    setListPage(1);
  }

  const biblioQuery = biblioDraft.trim();
  const visibleBiblioResults = biblioQuery ? biblioResults : [];
  const visibleBiblioSearching = biblioQuery ? biblioSearching : false;

  useEffect(() => {
    const q = biblioDraft.trim();
    if (!q) {
      biblioSeqRef.current += 1;
      return;
    }
    const timer = window.setTimeout(() => {
      const seq = ++biblioSeqRef.current;
      setBiblioSearching(true);
      void (async () => {
        try {
          const res = await api.getBiblios({ freesearch: q, perPage: 15, page: 1 });
          if (seq !== biblioSeqRef.current) return;
          setBiblioResults(res.items);
        } catch {
          if (seq !== biblioSeqRef.current) return;
          setBiblioResults([]);
        } finally {
          if (seq === biblioSeqRef.current) setBiblioSearching(false);
        }
      })();
    }, 350);
    return () => window.clearTimeout(timer);
  }, [biblioDraft]);

  const createUserQuery = createUserDraft.trim();
  const visibleCreateUserResults = createUserQuery ? createUserResults : [];
  const visibleCreateUserSearching = createUserQuery ? createUserSearching : false;

  useEffect(() => {
    const q = createUserDraft.trim();
    if (!q) {
      createUserSeqRef.current += 1;
      return;
    }
    const timer = window.setTimeout(() => {
      const seq = ++createUserSeqRef.current;
      setCreateUserSearching(true);
      void (async () => {
        try {
          const res = await api.getUsers({ name: q, perPage: 10 });
          if (seq !== createUserSeqRef.current) return;
          setCreateUserResults(res.items);
        } catch {
          if (seq !== createUserSeqRef.current) return;
          setCreateUserResults([]);
        } finally {
          if (seq === createUserSeqRef.current) setCreateUserSearching(false);
        }
      })();
    }, 300);
    return () => window.clearTimeout(timer);
  }, [createUserDraft]);

  const resetCreateForm = () => {
    setBiblioDraft('');
    setBiblioResults([]);
    setSelectedBiblio(null);
    setSelectedItemId(null);
    setHoldScope('title');
    setCreateNotes('');
    setCreateUserDraft('');
    setCreateUserResults([]);
    setSelectedUserForCreate(null);
    setBiblioPickError(null);
    setCopyBarcode('');
    setBarcodeLookupError(null);
    setPickupSiteId('');
  };

  const resolveCopyBarcode = async (raw: string) => {
    const trimmed = raw.trim();
    if (!trimmed || barcodeLookupInFlightRef.current) return;
    barcodeLookupInFlightRef.current = true;
    setBarcodeLookupError(null);
    setBiblioPickError(null);
    try {
      const biblio = await api.getItemByBarcode(trimmed);
      const specimen =
        biblio.items?.find((s) => (s.barcode ?? '').trim() === trimmed && s.id != null) ??
        biblio.items?.find((s) => s.id != null);
      if (!specimen?.id) {
        setBarcodeLookupError(t('holds.copyBarcodeNotFound', { barcode: trimmed }));
        return;
      }
      setSelectedBiblio(biblio);
      setSelectedItemId(specimen.id);
      setHoldScope('title');
      setCopyBarcode('');
      setBiblioDraft('');
      setBiblioResults([]);
    } catch (e: unknown) {
      setBarcodeLookupError(getApiErrorMessage(e, t) || t('holds.copyBarcodeNotFound', { barcode: trimmed }));
    } finally {
      barcodeLookupInFlightRef.current = false;
    }
  };

  const loadBiblio = async (b: BiblioShort) => {
    setBiblioPickError(null);
    try {
      const full = await api.getBiblio(b.id);
      setSelectedBiblio(full);
      setSelectedItemId(null);
      setHoldScope('title');
      setBiblioDraft('');
      setBiblioResults([]);
    } catch (e: unknown) {
      setBiblioPickError(getApiErrorMessage(e, t));
    }
  };

  const activeHoldsQuery = useQuery({
    queryKey: ['activeHolds'],
    queryFn: () =>
      api.getHolds({
        page: 1,
        perPage: STAFF_HOLDS_FETCH_CAP,
        activeOnly: true,
      }),
    staleTime: 30 * 1000,
  });

  const selectedBiblioId = selectedBiblio?.id ?? null;
  const pinningCopy = holdScope === 'copy';
  const canSubmitCreate =
    !!selectedUserForCreate &&
    !!selectedBiblioId &&
    (!pinningCopy || !!selectedItemId);

  const biblioQueueQuery = useQuery({
    queryKey: ['biblioHolds', selectedBiblioId],
    queryFn: () => api.getBiblioHolds(selectedBiblioId!),
    enabled: showCreateModal && !!selectedBiblioId,
    staleTime: 30 * 1000,
  });

  const createQuotaQuery = useQuery({
    queryKey: ['holdsQuota', selectedUserForCreate?.id],
    queryFn: () => api.getHoldQuota(selectedUserForCreate!.id),
    enabled: showCreateModal && !!selectedUserForCreate?.id,
    staleTime: 15 * 1000,
  });

  const createMutation = useMutation({
    mutationFn: async () => {
      if (!selectedUserForCreate) throw new Error(t('holds.selectUser'));
      if (!selectedBiblioId) throw new Error(t('holds.selectBiblio'));
      if (pinningCopy && !selectedItemId) throw new Error(t('holds.selectCopy'));
      if (pickupMissing) throw new Error(t('holds.pickupSiteRequired'));
      return api.createHold(
        buildCreateHold({
          userId: selectedUserForCreate.id,
          biblioId: selectedBiblioId,
          scope: holdScope,
          itemId: selectedItemId,
          notes: createNotes,
          pickupSiteId,
        }),
      );
    },
    onSuccess: () => {
      showToast({
        variant: 'success',
        message: pinningCopy ? t('holds.createSuccessPinned') : t('holds.createSuccessTitle'),
      });
      resetCreateForm();
      setShowCreateModal(false);
      void queryClient.invalidateQueries({ queryKey: ['activeHolds'] });
      void queryClient.invalidateQueries({ queryKey: ['biblioHolds'] });
      void queryClient.invalidateQueries({ queryKey: ['holdsQuota'] });
    },
  });

  const cancelMutation = useMutation({
    mutationFn: (id: string) => api.cancelHold(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['activeHolds'] });
    },
  });

  const cancelCell = (r: Hold) =>
    r.status === 'pending' || r.status === 'ready' ? (
      <Button
        size="sm"
        variant="secondary"
        leftIcon={<Ban className="h-4 w-4" />}
        isLoading={cancelMutation.isPending && cancelMutation.variables === r.id}
        onClick={() => setCancelHoldId(r.id)}
      >
        {t('holds.cancelHold')}
      </Button>
    ) : null;

  const actionCell = (r: Hold) => (
    <div className="flex flex-wrap justify-end gap-2">
      <HoldTransitActions hold={r} transit={transitByHoldId.get(r.id)} actions={transitActions} />
      {cancelCell(r)}
    </div>
  );

  const columns = [
    {
      key: 'user',
      header: t('holds.columnUser'),
      render: (r: Hold) => {
        const label = formatUserShortName(r.user) || r.userId;
        return (
          <Link className="text-indigo-600 dark:text-indigo-400 hover:underline" to={`/users/${r.userId}`}>
            {label}
          </Link>
        );
      },
    },
    {
      key: 'item',
      header: t('holds.columnDocument'),
      render: (r: Hold) => <HoldDocumentCell hold={r} />,
    },
    {
      key: 'status',
      header: t('holds.status'),
      render: (r: Hold) => statusBadge(t, r.status),
    },
    {
      key: 'pickup',
      header: t('holds.columnPickup'),
      render: (r: Hold) => (
        <HoldPickupCell hold={r} sources={sources} transit={transitByHoldId.get(r.id)} />
      ),
    },
    {
      key: 'position',
      header: t('holds.position'),
      render: (r: Hold) => t('holds.queuePosition', { position: r.position }),
    },
    {
      key: 'created',
      header: t('holds.createdAt'),
      render: (r: Hold) => new Date(r.createdAt).toLocaleString(i18n.language),
    },
    {
      key: 'expires',
      header: t('holds.expiresAt'),
      render: (r: Hold) => <HoldExpiresCell hold={r} emphasizePickup />,
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: actionCell,
    },
  ];

  const listData = activeHoldsQuery.data;
  const preparedHolds = useMemo(
    () => applyStaffHoldList(listData?.items ?? [], statusFilter),
    [listData?.items, statusFilter],
  );
  const totalPages = Math.max(1, Math.ceil(preparedHolds.length / listPerPage));
  const safePage = Math.min(listPage, totalPages);
  const pageHolds = preparedHolds.slice((safePage - 1) * listPerPage, safePage * listPerPage);
  const emptyMessage = statusFilter === 'all' ? t('holds.noActiveHolds') : t('holds.noMatchingHolds');

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
            <Bookmark className="h-7 w-7 text-amber-600" />
            {t('holds.pageTitle')}
          </h1>
          <p className="text-gray-500 dark:text-gray-400 mt-1">{t('holds.subtitle')}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Link to="/transits">
            <Button variant="secondary" leftIcon={<Truck className="h-4 w-4" />}>
              {t('transits.openQueue')}
            </Button>
          </Link>
          <Button
            variant="primary"
            leftIcon={<Plus className="h-4 w-4" />}
            onClick={() => setShowCreateModal(true)}
          >
            {t('holds.newHoldButton')}
          </Button>
        </div>
      </div>

      <Card padding="none" className="flex flex-col min-h-0">
        {activeHoldsQuery.isError && (
          <div className="mx-4 mt-4 rounded-lg border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3">
            <div className="flex items-start gap-2 flex-1 min-w-0 text-sm text-red-800 dark:text-red-200">
              <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" aria-hidden />
              <span>{getApiErrorMessage(activeHoldsQuery.error, t)}</span>
            </div>
            <Button type="button" size="sm" variant="secondary" onClick={() => void activeHoldsQuery.refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
          <CardHeader
            title={t('holds.activeHoldsTitle')}
            subtitle={
              listData != null ? t('holds.activeHoldsCount', { total: preparedHolds.length }) : undefined
            }
          />
        </div>
        <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-800 flex flex-wrap items-end gap-3 flex-shrink-0">
          <fieldset className="flex flex-wrap gap-4">
            <legend className="sr-only">{t('holds.staffFilterLegend')}</legend>
            {(
              [
                ['all', 'holds.filterAllActive'],
                ['ready', 'holds.filterReady'],
                ['pending', 'holds.filterPending'],
                ['expiringSoon', 'holds.filterExpiringSoon'],
              ] as const
            ).map(([value, key]) => (
              <label key={value} className={formChoiceLabelClass()}>
                <input
                  type="radio"
                  name="staff-holds-filter"
                  className="text-indigo-600"
                  checked={statusFilter === value}
                  onChange={() => setStatusFilter(value)}
                />
                {t(key)}
              </label>
            ))}
          </fieldset>
          <div className="flex flex-col gap-1">
            <label className={formLabelClass({ marginBottom: false })} htmlFor="staff-holds-per-page">
              {t('common.perPage')}
            </label>
            <select
              id="staff-holds-per-page"
              value={listPerPage}
              onChange={(e) => {
                setListPerPage(Number(e.target.value));
                setListPage(1);
              }}
              className={formControlClass({ className: 'min-w-[5rem]' })}
            >
              {[25, 50, 100, 200].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </div>
        </div>
        <ScrollableListRegion aria-label={t('holds.activeHoldsTitle')}>
          {activeHoldsQuery.isLoading && !(listData?.items?.length) ? (
            <ListSkeleton rows={8} />
          ) : (
            <ResponsiveRecordList
              desktop={
                <Table
                  columns={columns}
                  data={pageHolds}
                  emptyMessage={emptyMessage}
                  keyExtractor={(r) => r.id}
                  isLoading={false}
                />
              }
              mobile={
                pageHolds.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                    {emptyMessage}
                  </div>
                ) : (
                  <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                    {pageHolds.map((r) => (
                      <HoldMobileCard
                        key={r.id}
                        hold={r}
                        emphasizePickup
                        statusBadge={(s) => statusBadge(t, s)}
                        pickupSources={sources}
                        transit={transitByHoldId.get(r.id)}
                        extraActions={
                          <HoldTransitActions
                            hold={r}
                            transit={transitByHoldId.get(r.id)}
                            actions={transitActions}
                          />
                        }
                        onCancel={() => setCancelHoldId(r.id)}
                        cancelPending={cancelMutation.isPending && cancelMutation.variables === r.id}
                      />
                    ))}
                  </div>
                )
              }
            />
          )}
        </ScrollableListRegion>
        {preparedHolds.length > 0 && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-800 flex-shrink-0">
            <Pagination currentPage={safePage} totalPages={totalPages} onPageChange={setListPage} />
          </div>
        )}
      </Card>

      <Modal
        isOpen={showCreateModal}
        onClose={() => {
          setShowCreateModal(false);
          resetCreateForm();
        }}
        title={t('holds.createSection')}
        size="lg"
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="ghost"
              onClick={() => {
                setShowCreateModal(false);
                resetCreateForm();
              }}
            >
              {t('common.cancel')}
            </Button>
            <Button
              variant="primary"
              isLoading={createMutation.isPending}
              disabled={
                !canSubmitCreate ||
                createMutation.isPending ||
                pickupMissing ||
                (createQuotaQuery.data != null && createQuotaQuery.data.remaining <= 0)
              }
              onClick={() => void createMutation.mutateAsync()}
            >
              {t('holds.confirmReserve')}
            </Button>
          </div>
        }
      >
        <div className="space-y-4 text-sm">
          <div>
            <BarcodeScanField
              label={t('holds.scanCopyBarcode')}
              value={copyBarcode}
              onChange={(e) => {
                setCopyBarcode(e.target.value);
                if (barcodeLookupError) setBarcodeLookupError(null);
              }}
              placeholder={t('holds.copyBarcodePlaceholder')}
              onCameraScan={(barcode) => {
                setCopyBarcode(barcode);
                void resolveCopyBarcode(barcode);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void resolveCopyBarcode(copyBarcode);
                }
              }}
              scannerTitle={t('holds.scanCopyBarcode')}
            />
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{t('holds.scanCopyBarcodeHintTitle')}</p>
            {barcodeLookupError && (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400 mt-2">
                {barcodeLookupError}
              </p>
            )}
          </div>

          <div>
            <Typeahead
              label={t('holds.searchBiblio')}
              value={biblioDraft}
              onChange={setBiblioDraft}
              items={visibleBiblioResults}
              getItemId={(b) => b.id}
              onSelect={(b) => void loadBiblio(b)}
              leftIcon={<Search className="h-4 w-4" />}
              loading={visibleBiblioSearching}
              renderItem={(b) => (
                <>
                  <span className="font-medium text-gray-900 dark:text-white">{b.title}</span>
                  {b.isbn && (
                    <span className="text-gray-500 ml-2 font-mono text-xs">{formatIsbnDisplay(b.isbn)}</span>
                  )}
                </>
              )}
            />
            {visibleBiblioSearching && <p className="text-xs text-gray-500 mt-1">{t('common.loading')}</p>}
            {biblioPickError && (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400 mt-2">{biblioPickError}</p>
            )}
          </div>

          {selectedBiblio && (
            <div className="space-y-3 rounded-lg border border-gray-200 dark:border-gray-700 p-3">
              <p className="font-medium text-gray-900 dark:text-white">{selectedBiblio.title}</p>
              <fieldset className="space-y-2">
                <legend className={formLabelClass({ marginBottom: false })}>
                  {t('holds.pickScope')}
                </legend>
                <label className={formChoiceLabelClass()}>
                  <input
                    type="radio"
                    name="desk-hold-scope"
                    className="text-indigo-600"
                    checked={holdScope === 'title'}
                    onChange={() => setHoldScope('title')}
                  />
                  <span>
                    {t('holds.scopeTitle')}
                    <span className="block text-xs font-normal text-gray-500 dark:text-gray-400">
                      {t('holds.scopeTitleHint')}
                    </span>
                  </span>
                </label>
                <label className={formChoiceLabelClass()}>
                  <input
                    type="radio"
                    name="desk-hold-scope"
                    className="text-indigo-600"
                    checked={holdScope === 'copy'}
                    disabled={(selectedBiblio.items ?? []).length === 0}
                    onChange={() => {
                      setHoldScope('copy');
                      if (!selectedItemId) {
                        setSelectedItemId(selectedBiblio.items?.[0]?.id ?? null);
                      }
                    }}
                  />
                  <span>
                    {t('holds.scopePinned')}
                    <span className="block text-xs font-normal text-gray-500 dark:text-gray-400">
                      {t('holds.scopePinnedHint')}
                    </span>
                  </span>
                </label>
              </fieldset>
              {holdScope === 'copy' && (
                <div className="flex flex-col gap-1">
                  <label className={formLabelClass({ marginBottom: false })}>
                    {t('holds.pickSpecimen')}
                  </label>
                  <select
                    value={selectedItemId ?? ''}
                    onChange={(e) => setSelectedItemId(e.target.value || null)}
                    className={formControlClass()}
                  >
                    <option value="">{t('holds.selectCopy')}</option>
                    {(selectedBiblio.items ?? []).map((it) => (
                      <option key={it.id} value={it.id}>
                        {it.barcode || it.callNumber || it.id}
                      </option>
                    ))}
                  </select>
                </div>
              )}
              {createQuotaQuery.data && createQuotaQuery.data.remaining > 0 && (
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  {t('holds.quotaRemaining', {
                    remaining: createQuotaQuery.data.remaining,
                    max: createQuotaQuery.data.maxActiveHolds,
                  })}
                </p>
              )}
              {createQuotaQuery.data && createQuotaQuery.data.remaining <= 0 && (
                <p className="text-sm text-amber-700 dark:text-amber-300">
                  {t('holds.quotaFull', {
                    active: createQuotaQuery.data.activeHolds,
                    max: createQuotaQuery.data.maxActiveHolds,
                  })}
                </p>
              )}
              <div>
                <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-1">
                  {t('holds.biblioQueue')}
                </p>
                {biblioQueueQuery.isLoading && (
                  <p className="text-xs text-gray-500">{t('common.loading')}</p>
                )}
                {!biblioQueueQuery.isLoading && (biblioQueueQuery.data?.length ?? 0) === 0 && (
                  <p className="text-xs text-gray-500 dark:text-gray-400">{t('holds.biblioQueueEmpty')}</p>
                )}
                {(biblioQueueQuery.data?.length ?? 0) > 0 && (
                  <ol className="list-decimal list-inside space-y-1 text-sm border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
                    {biblioQueueQuery.data!.map((h) => (
                      <li key={h.id} className="flex flex-wrap items-center gap-2">
                        <span className="font-medium">{formatUserShortName(h.user) || h.userId}</span>
                        <HoldScopeBadge hold={h} />
                      </li>
                    ))}
                  </ol>
                )}
              </div>
            </div>
          )}

          <div>
            <Typeahead
              label={t('holds.pickUser')}
              value={createUserDraft}
              onChange={setCreateUserDraft}
              items={visibleCreateUserResults}
              getItemId={(u) => u.id}
              selectedId={selectedUserForCreate?.id}
              onSelect={(u) => {
                setSelectedUserForCreate(u);
                setCreateUserDraft('');
                setCreateUserResults([]);
              }}
              placeholder={t('users.searchPlaceholder')}
              loading={visibleCreateUserSearching}
              renderItem={(u) => (
                <>
                  {u.firstname} {u.lastname}{' '}
                  <span className="text-gray-500 font-mono text-xs">{u.id}</span>
                </>
              )}
            />
            {visibleCreateUserSearching && <p className="text-xs text-gray-500 mt-1">{t('common.loading')}</p>}
            {selectedUserForCreate && (
              <p className="text-sm text-gray-600 dark:text-gray-400 mt-2">
                {selectedUserForCreate.firstname} {selectedUserForCreate.lastname}
              </p>
            )}
          </div>

          <PickupSiteSelect
            id="desk-pickup-site"
            sources={sources}
            value={pickupSiteId}
            onChange={setPickupSiteId}
            disabled={createMutation.isPending}
          />

          <Input
            label={t('holds.notesOptional')}
            value={createNotes}
            onChange={(e) => setCreateNotes(e.target.value)}
          />

          {createMutation.isError && (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {getApiErrorMessage(createMutation.error, t) || t('holds.createError')}
            </p>
          )}
        </div>
      </Modal>

      <ConfirmDialog
        isOpen={cancelHoldId !== null}
        onClose={() => setCancelHoldId(null)}
        onConfirm={() => {
          const id = cancelHoldId;
          setCancelHoldId(null);
          if (id) void cancelMutation.mutateAsync(id);
        }}
        message={t('holds.cancelConfirm')}
        confirmVariant="danger"
      />
      <TransitActionModals actions={transitActions} />
    </div>
  );
}
