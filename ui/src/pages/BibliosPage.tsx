import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useNavigate, useSearchParams, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Plus, BookOpen, LibraryBig, SlidersHorizontal, Loader2, AlertCircle, Trash2, Layers, BookMarked, Edit, ChevronLeft, ChevronRight, ScanLine, Upload } from 'lucide-react';
import { MarcImportWorkflow } from '@/pages/ImportIsoPage';
import { useLibrary } from '@/contexts/LibraryContext';
import { isFeatureEnabled } from '@/utils/featureAccess';
import { Card, Button, Table, SearchInput, Modal, Input, ScrollableListRegion, ResponsiveRecordList, ListSkeleton, SectionTabBar, PageHeading, PageTabLeadGroup, ListPageLayout, ListResultCountBar } from '@/components/common';
import BiblioCatalogItemCard from '@/components/items/BiblioCatalogItemCard';
import BatchDeleteSpecimensDialog from '@/components/specimen/BatchDeleteSpecimensDialog';
import { useAuth } from '@/contexts/AuthContext';
import ItemAvailabilityBadge from '@/components/items/ItemAvailabilityBadge';
import { canManageItems, type MediaType, type MediaTypeOption, type Serie, type Collection } from '@/types';
import { summaryAvailability } from '@/utils/itemAvailability';
import api from '@/services/api';
import type { BiblioShort, Author, PaginatedResponse } from '@/types';
import { PUBLIC_TYPE_OPTIONS } from '@/utils/codeLabels';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { isCatalogDeletionRefusal } from '@/utils/withdrawals';
import { getListScrollRoot } from '@/utils/listScrollRoot';
import { LIST_ROW_ICON_BTN, LIST_ROW_ICON_BTN_DANGER } from '@/utils/listRowActionIconClass';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { useInfiniteQuery, useQuery, useQueryClient } from '@tanstack/react-query';
import { filterLoadedBiblios, type DepositStockFilter } from '@/utils/deposits';
type CatalogTab = 'catalog' | 'collections' | 'series' | 'import';

function resolveCatalogTab(tab: string | null, importIsoEnabled: boolean): CatalogTab {
  if (tab === 'collections') return 'collections';
  if (tab === 'series') return 'series';
  if (tab === 'import' && importIsoEnabled) return 'import';
  return 'catalog';
}

import {
  buildCatalogSearchParams,
  catalogListPath,
  clearCatalogRestoreSession,
  readCatalogRestoreSession,
  readCatalogScrollTop,
  restoreCatalogScrollTop,
  writeCatalogRestoreSession,
  type CatalogSavedSearch,
} from '@/utils/catalogSearchState';

const PAGE_SIZE = 20;

export default function BibliosPage() {
  const { t } = useTranslation();
  const { user } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const { libraryInfo } = useLibrary();
  const importIsoEnabled = isFeatureEnabled(libraryInfo?.enabledFeatures, 'importIso');

  const restoredFromState = (location.state as { restoredSearch?: CatalogSavedSearch } | null)
    ?.restoredSearch;
  /** Filter snapshot when returning via the in-app back button (browser back uses URL params). */
  const restoredSearch = restoredFromState;

  const activeTab = resolveCatalogTab(searchParams.get('tab'), importIsoEnabled);
  const setActiveTab = useCallback(
    (tab: CatalogTab) => {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (tab === 'catalog') {
            next.delete('tab');
          } else {
            next.set('tab', tab);
            next.delete('freesearch');
          }
          return next;
        },
        { replace: true }
      );
    },
    [setSearchParams]
  );

  // Active catalog filter from a collection or series click
  const [activeFilterSerieId, setActiveFilterSerieId] = useState(
    () => restoredSearch?.serieId ?? searchParams.get('serie_id') ?? ''
  );
  const [activeFilterSerieName, setActiveFilterSerieName] = useState(
    () => restoredSearch?.serieName ?? searchParams.get('serie_name') ?? ''
  );
  const [activeFilterCollectionId, setActiveFilterCollectionId] = useState(
    () => restoredSearch?.collectionId ?? searchParams.get('collection_id') ?? ''
  );
  const [activeFilterCollectionName, setActiveFilterCollectionName] = useState(
    () => restoredSearch?.collectionName ?? searchParams.get('collection_name') ?? ''
  );

  const goToCatalogWithFilter = useCallback(
    (kind: 'serie' | 'collection', id: string, name: string) => {
      setActiveFilterSerieId(kind === 'serie' ? id : '');
      setActiveFilterSerieName(kind === 'serie' ? name : '');
      setActiveFilterCollectionId(kind === 'collection' ? id : '');
      setActiveFilterCollectionName(kind === 'collection' ? name : '');
      // activeTab is driven by URL `tab=`; must clear it to show the catalog tab
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.delete('tab');
          next.delete('freesearch');
          next.delete('serie_id');
          next.delete('serie_name');
          next.delete('collection_id');
          next.delete('collection_name');
          if (kind === 'serie') {
            next.set('serie_id', id);
            next.set('serie_name', name);
          } else {
            next.set('collection_id', id);
            next.set('collection_name', name);
          }
          return next;
        },
        { replace: true }
      );
    },
    [setSearchParams]
  );

  const clearCatalogFilter = useCallback(() => {
    setActiveFilterSerieId('');
    setActiveFilterSerieName('');
    setActiveFilterCollectionId('');
    setActiveFilterCollectionName('');
  }, []);

  const MEDIA_TYPES: MediaTypeOption[] = [
    { value: '', label: t('items.allTypes') },
    { value: 'unknown', label: t('items.mediaType.unknown') },
    { value: 'printedText', label: t('items.mediaType.printedText') },
    { value: 'comics', label: t('items.mediaType.comics') },
    { value: 'periodic', label: t('items.mediaType.periodic') },
    { value: 'video', label: t('items.mediaType.video') },
    { value: 'videoTape', label: t('items.mediaType.videoTape') },
    { value: 'videoDvd', label: t('items.mediaType.videoDvd') },
    { value: 'audio', label: t('items.mediaType.audio') },
    { value: 'audioMusic', label: t('items.mediaType.audioMusic') },
    { value: 'audioMusicTape', label: t('items.mediaType.audioMusicTape') },
    { value: 'audioMusicCd', label: t('items.mediaType.audioMusicCd') },
    { value: 'audioNonMusic', label: t('items.mediaType.audioNonMusic') },
    { value: 'cdRom', label: t('items.mediaType.cdRom') },
    { value: 'images', label: t('items.mediaType.images') },
    { value: 'multimedia', label: t('items.mediaType.multimedia') },
  ];

  // Filters – priority: location.state (back from detail) > URL params > default
  const [searchQuery, setSearchQuery] = useState(
    () => restoredSearch?.searchQuery ?? searchParams.get('freesearch') ?? ''
  );
  const [searchQueryDraft, setSearchQueryDraft] = useState(
    () => restoredSearch?.searchQuery ?? searchParams.get('freesearch') ?? ''
  );
  const [mediaType, setMediaType] = useState<MediaType | ''>(
    () => (restoredSearch?.mediaType ?? searchParams.get('media_type') ?? '') as MediaType | ''
  );
  const [audienceType, setAudienceType] = useState<string>(
    () => restoredSearch?.audienceType ?? searchParams.get('audience_type') ?? ''
  );
  const [showFilters, setShowFilters] = useState(
    () => restoredSearch?.showFilters ?? searchParams.get('show_filters') === '1'
  );

  const [includeWithoutActiveItems, setIncludeWithoutActiveItems] = useState(
    () => restoredSearch?.includeWithoutActiveItems ?? searchParams.get('include_without_items') === '1'
  );

  const {
    data,
    isLoading: isItemsLoading,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
    isError: isCatalogQueryError,
    error: catalogQueryError,
    refetch: refetchCatalog,
  } = useInfiniteQuery({
    queryKey: [
      'biblios',
      {
        searchQuery,
        mediaType,
        audienceType,
        includeWithoutActiveItems,
        activeFilterSerieId,
        activeFilterCollectionId,
      },
    ],
    enabled: activeTab === 'catalog',
    queryFn: async ({ pageParam }) => {
      return api.getBiblios({
        freesearch: searchQuery || undefined,
        mediaType: mediaType || undefined,
        audienceType: audienceType || undefined,
        serieId: activeFilterSerieId || undefined,
        collectionId: activeFilterCollectionId || undefined,
        ...(includeWithoutActiveItems ? { includeWithoutActiveItems: true } : {}),
        page: pageParam,
        perPage: PAGE_SIZE,
      });
    },
    staleTime: 5 * 60 * 1000,
    /** Refetch when stale (e.g. after edit invalidates); skip when cache is still fresh (back from detail). */
    refetchOnMount: true,
    refetchOnWindowFocus: false,
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      if (!lastPage?.items?.length) return undefined;
      const loaded = lastPage.page * lastPage.perPage;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
  });

  const items = data?.pages.flatMap((p) => p.items) ?? [];
  const totalItems = data?.pages[0]?.total ?? 0;
  const [depositFilter, setDepositFilter] = useState<DepositStockFilter>('all');
  const [depositLenderId, setDepositLenderId] = useState('');
  const lendersQuery = useQuery({
    queryKey: ['lenders', 'catalog-filter'],
    queryFn: () => api.listLenders(false),
    enabled: depositFilter === 'deposits',
  });
  const depositFilterActive = depositFilter !== 'all' || depositLenderId !== '';
  const visibleItems = filterLoadedBiblios(items, depositFilter, depositLenderId);
  const listedCount = depositFilterActive ? visibleItems.length : totalItems;

  const canManage = canManageItems(user, api.getToken());

  useEffect(() => {
    if (!canManage && activeTab !== 'catalog') {
      setActiveTab('catalog');
    }
  }, [canManage, activeTab, setActiveTab]);

  const [catalogBiblioToDelete, setCatalogBiblioToDelete] = useState<BiblioShort | null>(null);
  const [catalogDeleteBorrowedError, setCatalogDeleteBorrowedError] = useState(false);
  const [catalogDeleteForceBlocked, setCatalogDeleteForceBlocked] = useState(false);
  const [catalogDeleteLoading, setCatalogDeleteLoading] = useState(false);
  const [catalogDeleteApiError, setCatalogDeleteApiError] = useState<string | null>(null);
  const [showBatchDeleteSpecimens, setShowBatchDeleteSpecimens] = useState(false);

  const loadMoreRef = useRef<HTMLDivElement>(null);
  const initialScrollTop = useMemo(() => {
    const fromState = restoredFromState?.scrollTop;
    if (typeof fromState === 'number' && fromState > 0) return fromState;
    const fromSession = readCatalogRestoreSession()?.scrollTop;
    if (typeof fromSession === 'number' && fromSession > 0) return fromSession;
    return null;
  }, []);
  const pendingScrollTop = useRef<number | null>(initialScrollTop);

  useEffect(() => {
    const el = loadMoreRef.current;
    const scrollRoot = getListScrollRoot(el);
    if (!el || !scrollRoot || !hasNextPage || isFetchingNextPage || !data?.pages?.length) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) fetchNextPage();
      },
      { root: scrollRoot, rootMargin: '200px', threshold: 0 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, data?.pages?.length]);

  useEffect(() => {
    if (pendingScrollTop.current == null || isItemsLoading || !data?.pages?.length) return;
    const top = pendingScrollTop.current;
    const scrollRoot = getListScrollRoot(loadMoreRef.current);
    if (scrollRoot instanceof HTMLElement && top + scrollRoot.clientHeight > scrollRoot.scrollHeight) {
      if (hasNextPage && !isFetchingNextPage) {
        void fetchNextPage();
      }
      if (hasNextPage) return;
    }
    pendingScrollTop.current = null;
    restoreCatalogScrollTop(loadMoreRef.current, top);
  }, [
    isItemsLoading,
    data?.pages?.length,
    visibleItems.length,
    hasNextPage,
    isFetchingNextPage,
    fetchNextPage,
  ]);

  // Clear one-shot restore hints so refresh does not re-apply scroll/state
  useEffect(() => {
    if (restoredFromState) {
      navigate('.', { replace: true, state: null });
    }
    clearCatalogRestoreSession();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Persist search state in URL so browser back restores filters without re-fetch churn
  useEffect(() => {
    if (activeTab !== 'catalog') return;
    const next = buildCatalogSearchParams({
      searchQuery,
      mediaType,
      audienceType,
      showFilters,
      includeWithoutActiveItems,
      serieId: activeFilterSerieId,
      serieName: activeFilterSerieName,
      collectionId: activeFilterCollectionId,
      collectionName: activeFilterCollectionName,
    });
    setSearchParams(next, { replace: true });
  }, [
    searchQuery,
    mediaType,
    audienceType,
    showFilters,
    includeWithoutActiveItems,
    activeFilterSerieId,
    activeFilterSerieName,
    activeFilterCollectionId,
    activeFilterCollectionName,
    setSearchParams,
    activeTab,
  ]);

  const handleApplySearch = () => {
    setSearchQuery(searchQueryDraft);
    queryClient.invalidateQueries({ queryKey: ['biblios'] });
  };

  const getSavedSearch = useCallback((): CatalogSavedSearch => ({
    searchQuery,
    mediaType,
    audienceType,
    showFilters,
    includeWithoutActiveItems,
    serieId: activeFilterSerieId,
    serieName: activeFilterSerieName,
    collectionId: activeFilterCollectionId,
    collectionName: activeFilterCollectionName,
    scrollTop: readCatalogScrollTop(loadMoreRef.current),
  }), [
    searchQuery,
    mediaType,
    audienceType,
    showFilters,
    includeWithoutActiveItems,
    activeFilterSerieId,
    activeFilterSerieName,
    activeFilterCollectionId,
    activeFilterCollectionName,
  ]);

  const handleRowClick = (item: BiblioShort) => {
    const savedSearch = getSavedSearch();
    writeCatalogRestoreSession(savedSearch);
    const listPath = catalogListPath(savedSearch);
    navigate(listPath, { replace: true });
    navigate(`/biblios/${item.id}`, { state: { savedSearch } });
  };

  const handleCatalogBiblioDelete = async (force: boolean) => {
    if (!catalogBiblioToDelete) return;
    setCatalogDeleteLoading(true);
    setCatalogDeleteApiError(null);
    setCatalogDeleteForceBlocked(false);
    try {
      await api.deleteBiblio(catalogBiblioToDelete.id, force);
      setCatalogBiblioToDelete(null);
      setCatalogDeleteBorrowedError(false);
      setCatalogDeleteForceBlocked(false);
      await queryClient.invalidateQueries({ queryKey: ['biblios'] });
    } catch (error: unknown) {
      const code = getApiErrorCode(error);
      const msg =
        (error as { response?: { data?: { message?: string } } })?.response?.data?.message ?? '';
      if (isCatalogDeletionRefusal(code)) {
        setCatalogDeleteBorrowedError(false);
        setCatalogDeleteForceBlocked(true);
        setCatalogDeleteApiError(getApiErrorMessage(error, t));
      } else if (
        !force &&
        (code === 'business_rule_violation' ||
          code === 'conflict' ||
          (typeof msg === 'string' && (msg.includes('borrowed') || msg.includes('force=true'))))
      ) {
        setCatalogDeleteBorrowedError(true);
      } else {
        setCatalogDeleteApiError(getApiErrorMessage(error, t));
      }
    } finally {
      setCatalogDeleteLoading(false);
    }
  };

  const formatAuthor = (author?: Author | null) => {
    if (!author) return '-';
    return `${author.firstname || ''} ${author.lastname || ''}`.trim() || '-';
  };

  const getCatalogRowStatusBadge = (row: BiblioShort) => {
    const list = row.items ?? [];
    if (list.length === 0) return null;
    return (
      <ItemAvailabilityBadge
        availability={summaryAvailability(list, 'staff')}
        audience="staff"
        size="sm"
      />
    );
  };

  const columns = [
    {
      key: 'title',
      header: t('items.titleField'),
      className: 'min-w-[12rem] max-w-[24rem]',
      render: (item: BiblioShort) => (
        <div className="flex items-center gap-3">
          <LoanMediaTypeBadge
            mediaType={item.mediaType}
            size="table"
            depositCopies={item.items}
          />
          <div className="min-w-0">
            <p className="font-medium text-gray-900 dark:text-white truncate">
              {item.title || t('items.notSpecified')}
            </p>
            <p className="text-sm text-gray-500 dark:text-gray-400 truncate font-mono">
              {formatIsbnDisplay(item.isbn)}
            </p>
          </div>
        </div>
      ),
    },
    {
      key: 'author',
      header: t('items.author'),
      className: 'min-w-[7rem] max-w-[14rem]',
      render: (item: BiblioShort) => (
        <span className="block truncate text-gray-600 dark:text-gray-300">
          {formatAuthor(item.author)}
        </span>
      ),
    },
    {
      key: 'date',
      header: t('common.date'),
      render: (item: BiblioShort) => item.date || '-',
      className: 'hidden md:table-cell',
    },
    {
      key: 'status',
      header: t('common.status'),
      render: (item: BiblioShort) => getCatalogRowStatusBadge(item),
    },
    ...(canManage
      ? [
          {
            key: 'actions',
            header: '',
            align: 'right' as const,
            className: 'w-[1%] whitespace-nowrap',
            render: (item: BiblioShort) => (
              <div
                className="flex justify-end gap-1.5"
                onClick={(e) => e.stopPropagation()}
                role="group"
                aria-label={t('common.actions')}
              >
                <button
                  type="button"
                  className={LIST_ROW_ICON_BTN}
                  title={t('common.edit')}
                  onClick={() =>
                    navigate(`/biblios/${item.id}/edit`, { state: { savedSearch: getSavedSearch() } })
                  }
                >
                  <Edit className="h-4 w-4" aria-hidden />
                </button>
                <button
                  type="button"
                  className={LIST_ROW_ICON_BTN_DANGER}
                  title={t('common.delete')}
                  onClick={() => {
                    setCatalogDeleteBorrowedError(false);
                    setCatalogBiblioToDelete(item);
                  }}
                >
                  <Trash2 className="h-4 w-4" aria-hidden />
                </button>
              </div>
            ),
          },
        ]
      : []),
  ];

  return (
    <ListPageLayout>
      <PageTabLeadGroup className="shrink-0">
        <PageHeading
          icon={LibraryBig}
          title={t('catalog.pageTitle')}
          subtitle={t('catalog.pageSubtitle')}
        />
        {canManage && activeTab !== 'import' ? (
          <SectionTabBar
            ariaLabel={t('catalog.pageTitle')}
            activeTab={activeTab}
            onSelectTab={setActiveTab}
            tabs={[
              { id: 'catalog', label: t('catalog.tabCatalog'), icon: BookOpen },
              { id: 'collections', label: t('catalog.tabCollections'), icon: Layers },
              { id: 'series', label: t('catalog.tabSeries'), icon: BookMarked },
            ]}
          />
        ) : null}
      </PageTabLeadGroup>

      {activeTab === 'import' && importIsoEnabled ? (
        <div className="shrink-0 space-y-4">
          <Button type="button" variant="secondary" size="sm" onClick={() => setActiveTab('catalog')}>
            {t('catalog.backToCatalog')}
          </Button>
          <MarcImportWorkflow layout="embedded" />
        </div>
      ) : null}

      {/* Collections tab */}
      {activeTab === 'collections' && (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <CollectionsTab
            canManage={canManage}
            onSelect={(id, name) => goToCatalogWithFilter('collection', id, name)}
          />
        </div>
      )}

      {/* Series tab */}
      {activeTab === 'series' && (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <SeriesTab
            canManage={canManage}
            onSelect={(id, name) => goToCatalogWithFilter('serie', id, name)}
          />
        </div>
      )}

      {/* Catalog tab content */}
      {activeTab === 'catalog' && (
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
      <>
      {isCatalogQueryError && (
        <div className="shrink-0 rounded-lg border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3">
          <div className="flex items-start gap-2 flex-1 min-w-0 text-sm text-red-800 dark:text-red-200">
            <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" aria-hidden />
            <span>{getApiErrorMessage(catalogQueryError, t)}</span>
          </div>
          <Button type="button" size="sm" variant="secondary" onClick={() => void refetchCatalog()}>
            {t('common.retry')}
          </Button>
        </div>
      )}

      {/* Active filter chip */}
      {(activeFilterSerieId || activeFilterCollectionId) && (
        <div className="flex shrink-0 items-center gap-2">
          <span className="text-sm text-gray-500 dark:text-gray-400">{t('catalog.filteredBy')}</span>
          <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-sm font-medium bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300">
            {activeFilterSerieId ? (
              <BookMarked className="h-3.5 w-3.5" />
            ) : (
              <Layers className="h-3.5 w-3.5" />
            )}
            {activeFilterSerieName || activeFilterCollectionName}
            <button
              onClick={clearCatalogFilter}
              className="ml-1 hover:text-amber-600 dark:hover:text-amber-200"
              title={t('common.clear')}
            >
              ×
            </button>
          </span>
        </div>
      )}

      {/* Header */}
      <div className="flex shrink-0 flex-col sm:flex-row sm:items-center sm:justify-end gap-4">
        {canManageItems(user, api.getToken()) && (
          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="secondary"
              onClick={() => setShowBatchDeleteSpecimens(true)}
              leftIcon={<ScanLine className="h-4 w-4" />}
            >
              {t('items.batchDeleteScan.openButton')}
            </Button>
            {importIsoEnabled ? (
              <Button
                variant="secondary"
                onClick={() => setActiveTab('import')}
                leftIcon={<Upload className="h-4 w-4" />}
              >
                {t('nav.importIso')}
              </Button>
            ) : null}
            <Button onClick={() => navigate('/biblios/new')} leftIcon={<Plus className="h-4 w-4" />}>
              {t('items.add')}
            </Button>
          </div>
        )}
      </div>

      {/* Search and filters — hidden while a serie/collection filter is active */}
      {!(activeFilterSerieId || activeFilterCollectionId) && (
      <Card className="shrink-0">
        <div className="flex flex-col gap-2 min-w-0 sm:flex-row sm:items-end">
          <div className="min-w-0 flex-1">
            <SearchInput
              value={searchQueryDraft}
              onChange={setSearchQueryDraft}
              placeholder={t('items.searchPlaceholder')}
              submitMode
              onSubmit={() => handleApplySearch()}
              submitLabel={t('common.search')}
            />
          </div>
          <button
            type="button"
            className={`shrink-0 self-stretch inline-flex items-center justify-center rounded-lg border px-3 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-900 ${
              showFilters
                ? 'border-indigo-500 bg-indigo-50 text-indigo-700 dark:border-indigo-400 dark:bg-indigo-950/50 dark:text-indigo-200'
                : 'border-gray-300 bg-white text-gray-700 hover:bg-gray-50 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200 dark:hover:bg-gray-800'
            }`}
            aria-label={t('items.advancedSearch')}
            aria-expanded={showFilters}
            title={t('items.advancedSearch')}
            onClick={() => setShowFilters(!showFilters)}
          >
            <SlidersHorizontal className="h-5 w-5" aria-hidden />
          </button>
        </div>

        {showFilters && (
          <div className="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700 overflow-x-auto">
            <div className="flex flex-nowrap items-end gap-4 min-w-min pb-0.5">
              <div className="flex flex-col gap-1.5 w-[11rem] shrink-0">
                <label
                  htmlFor="catalog-media-type"
                  className={formLabelClass({ marginBottom: false })}
                >
                  {t('items.mediaTypeLabel')}
                </label>
                <select
                  id="catalog-media-type"
                  value={mediaType}
                  onChange={(e) => setMediaType(e.target.value as MediaType | '')}
                  className={formControlClass({ className: 'w-full' })}
                >
                  {MEDIA_TYPES.map((type) => (
                    <option key={type.value} value={type.value}>
                      {type.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col gap-1.5 w-[11rem] shrink-0">
                <label
                  htmlFor="catalog-audience-type"
                  className={formLabelClass({ marginBottom: false })}
                >
                  {t('items.publicType')}
                </label>
                <select
                  id="catalog-audience-type"
                  value={audienceType}
                  onChange={(e) => setAudienceType(e.target.value)}
                  className={formControlClass({ className: 'w-full' })}
                >
                  <option value="">{t('items.allPublicTypes')}</option>
                  {PUBLIC_TYPE_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {t(opt.labelKey)}
                    </option>
                  ))}
                </select>
              </div>
              {canManage && (
              <div className="flex flex-col gap-1.5 w-[min(14rem,max-content)] shrink-0">
                <span
                  id="catalog-include-without-items-label"
                  className={formLabelClass({ marginBottom: false, className: 'leading-snug' })}
                >
                  {t('items.includeWithoutActiveItems')}
                </span>
                <div className="flex h-10 items-center">
                  <button
                    type="button"
                    role="switch"
                    aria-checked={includeWithoutActiveItems}
                    aria-labelledby="catalog-include-without-items-label"
                    onClick={() => setIncludeWithoutActiveItems((v) => !v)}
                    className={`relative h-7 w-12 shrink-0 rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-900 ${
                      includeWithoutActiveItems
                        ? 'bg-amber-500 dark:bg-amber-600'
                        : 'bg-gray-300 dark:bg-gray-600'
                    }`}
                  >
                    <span
                      className={`pointer-events-none absolute top-0.5 left-0.5 block h-6 w-6 rounded-full bg-white shadow transition-transform ${
                        includeWithoutActiveItems ? 'translate-x-5' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
              </div>
              )}
              <div className="flex flex-col gap-1.5 w-[11rem] shrink-0">
                <label htmlFor="catalog-deposits" className={formLabelClass({ marginBottom: false })}>
                  {t('deposits.filter.label')}
                </label>
                <select
                  id="catalog-deposits"
                  value={depositFilter}
                  onChange={(event) => {
                    const next = event.target.value as DepositStockFilter;
                    setDepositFilter(next);
                    if (next !== 'deposits') setDepositLenderId('');
                  }}
                  className={formControlClass({ className: 'w-full' })}
                >
                  <option value="all">{t('deposits.filter.all')}</option>
                  <option value="own">{t('deposits.filter.own')}</option>
                  <option value="deposits">{t('deposits.filter.deposits')}</option>
                </select>
              </div>
              {depositFilter === 'deposits' && (
                <div className="flex flex-col gap-1.5 w-[14rem] shrink-0">
                  <label htmlFor="catalog-deposit-lender" className={formLabelClass({ marginBottom: false })}>
                    {t('deposits.lender')}
                  </label>
                  <select
                    id="catalog-deposit-lender"
                    value={depositLenderId}
                    onChange={(event) => setDepositLenderId(event.target.value)}
                    className={formControlClass({ className: 'w-full' })}
                  >
                    <option value="">{t('deposits.filter.anyLender')}</option>
                    {(lendersQuery.data ?? []).map((lender) => (
                      <option key={lender.id} value={lender.id}>{lender.name}</option>
                    ))}
                  </select>
                </div>
              )}
            </div>
            {depositFilterActive && (
              <p className="mt-3 text-xs text-gray-500 dark:text-gray-400">{t('deposits.filter.loadedOnly')}</p>
            )}
          </div>
        )}
      </Card>
      )}

      {/* Items list: fixed-height scroll area so header/filters stay static */}
      <Card padding="none" className="flex min-h-0 flex-1 flex-col">
        <ListResultCountBar>{t('items.count', { count: listedCount })}</ListResultCountBar>
        <ScrollableListRegion fill aria-label={t('items.title')}>
          {isItemsLoading && !visibleItems.length ? (
            <ListSkeleton rows={10} />
          ) : (
            <>
              <ResponsiveRecordList
                desktop={
                  <Table
                    columns={columns}
                    data={visibleItems}
                    keyExtractor={(item) => item.id}
                    onRowClick={handleRowClick}
                    isLoading={false}
                    emptyMessage={t('items.noItems')}
                  />
                }
                mobile={
                  visibleItems.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                      {t('items.noItems')}
                    </div>
                  ) : (
                    <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                      {visibleItems.map((item) => (
                        <BiblioCatalogItemCard
                          key={item.id}
                          item={item}
                          formatAuthor={formatAuthor}
                          notSpecified={t('items.notSpecified')}
                          statusBadge={getCatalogRowStatusBadge(item)}
                          onOpen={() => handleRowClick(item)}
                          canManage={canManage}
                          editLabel={t('common.edit')}
                          deleteLabel={t('common.delete')}
                          actionsAriaLabel={t('common.actions')}
                          onEdit={
                            canManage
                              ? () =>
                                  navigate(`/biblios/${item.id}/edit`, {
                                    state: { savedSearch: getSavedSearch() },
                                  })
                              : undefined
                          }
                          onDelete={
                            canManage
                              ? () => {
                                  setCatalogDeleteBorrowedError(false);
                                  setCatalogBiblioToDelete(item);
                                }
                              : undefined
                          }
                        />
                      ))}
                    </div>
                  )
                }
              />
              <div ref={loadMoreRef} className="h-4 flex-shrink-0" aria-hidden />
              {isFetchingNextPage && (
                <div className="flex items-center justify-center gap-2 py-4 text-sm text-gray-500 dark:text-gray-400">
                  <Loader2 className="h-5 w-5 animate-spin" />
                  <span>{t('common.loading')}</span>
                </div>
              )}
            </>
          )}
        </ScrollableListRegion>
      </Card>

      <Modal
        isOpen={catalogBiblioToDelete !== null}
        onClose={() => {
          if (catalogDeleteLoading) return;
          setCatalogBiblioToDelete(null);
          setCatalogDeleteBorrowedError(false);
          setCatalogDeleteForceBlocked(false);
          setCatalogDeleteApiError(null);
        }}
        title={t('common.confirm')}
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                if (catalogDeleteLoading) return;
                setCatalogBiblioToDelete(null);
                setCatalogDeleteBorrowedError(false);
                setCatalogDeleteForceBlocked(false);
                setCatalogDeleteApiError(null);
              }}
            >
              {t('common.cancel')}
            </Button>
            {!catalogDeleteForceBlocked &&
              (catalogDeleteBorrowedError ? (
                <Button
                  variant="danger"
                  disabled={catalogDeleteLoading}
                  isLoading={catalogDeleteLoading}
                  onClick={() => void handleCatalogBiblioDelete(true)}
                >
                  {t('common.forceDelete')}
                </Button>
              ) : (
                <Button
                  variant="danger"
                  disabled={catalogDeleteLoading}
                  isLoading={catalogDeleteLoading}
                  onClick={() => void handleCatalogBiblioDelete(false)}
                >
                  {t('common.delete')}
                </Button>
              ))}
          </div>
        }
      >
        <p className="text-gray-600 dark:text-gray-300">
          {catalogDeleteBorrowedError
            ? t('items.itemBorrowedForceDelete')
            : t('items.deleteConfirm', {
                title: catalogBiblioToDelete?.title || t('items.notSpecified'),
              })}
        </p>
        {catalogDeleteApiError && (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400">{catalogDeleteApiError}</p>
        )}
      </Modal>

      <BatchDeleteSpecimensDialog
        isOpen={showBatchDeleteSpecimens}
        onClose={() => setShowBatchDeleteSpecimens(false)}
        onSpecimenDeleted={(_, biblioId) => {
          void queryClient.invalidateQueries({ queryKey: ['biblios'] });
          void queryClient.invalidateQueries({ queryKey: ['biblio', biblioId] });
        }}
      />

      </>
      </div>
      )}
    </ListPageLayout>
  );
}

// ─── Collections Tab ─────────────────────────────────────────────────────────

const ENTITY_UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function looksLikeEntityUuid(s: string): boolean {
  return ENTITY_UUID_RE.test(s.trim());
}

const COLL_PAGE_SIZE = 25;

interface CollectionsTabProps {
  canManage: boolean;
  onSelect?: (id: string, name: string) => void;
}

async function fetchCollectionsPage(q: string, p: number): Promise<PaginatedResponse<Collection>> {
  const trimmed = q.trim();
  if (looksLikeEntityUuid(trimmed)) {
    const one = await api.getCollection(trimmed).catch(() => null);
    if (one) {
      return {
        items: [one],
        total: 1,
        page: 1,
        perPage: COLL_PAGE_SIZE,
        pageCount: 1,
      };
    }
  }
  return api.getCollections({ name: q || undefined, page: p, perPage: COLL_PAGE_SIZE });
}

function CollectionsTab({ canManage, onSelect }: CollectionsTabProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [searchDraft, setSearchDraft] = useState('');
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const { data = null, isLoading, refetch } = useQuery({
    queryKey: ['collections', search, page],
    queryFn: () => fetchCollectionsPage(search, page),
  });
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editItem, setEditItem] = useState<Collection | null>(null);
  const [deleteItem, setDeleteItem] = useState<Collection | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleSearch = () => { setPage(1); setSearch(searchDraft); };

  const handleDelete = async () => {
    if (!deleteItem?.id) return;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await api.deleteCollection(deleteItem.id);
      setDeleteItem(null);
      void queryClient.invalidateQueries({ queryKey: ['collections'] });
      void refetch();
    } catch (err) {
      setDeleteError(getApiErrorMessage(err, t));
    } finally {
      setDeleteLoading(false);
    }
  };

  const collName = (c: Collection) =>
    c.name ?? c.secondaryTitle ?? '—';

  const columns = [
    {
      key: 'name',
      header: t('catalog.collectionName'),
      render: (c: Collection) => (
        <div>
          <p className="font-medium text-gray-900 dark:text-white">{collName(c)}</p>
          {c.secondaryTitle && (
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {c.secondaryTitle}
            </p>
          )}
        </div>
      ),
    },
    {
      key: 'issn',
      header: 'ISSN',
      render: (c: Collection) => <span className="text-gray-600 dark:text-gray-300">{c.issn ?? '—'}</span>,
      className: 'hidden md:table-cell',
    },
    {
      key: 'key',
      header: t('catalog.key'),
      render: (c: Collection) => <span className="text-xs text-gray-500 dark:text-gray-400 font-mono">{c.key ?? '—'}</span>,
      className: 'hidden lg:table-cell',
    },
    ...(canManage
      ? [{
          key: 'actions',
          header: '',
          align: 'right' as const,
          render: (c: Collection) => (
            <div className="flex justify-end gap-1">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setEditItem(c);
                }}
                className={LIST_ROW_ICON_BTN}
                title={t('common.edit')}
              >
                <Edit className="h-4 w-4" aria-hidden />
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setDeleteError(null);
                  setDeleteItem(c);
                }}
                className={LIST_ROW_ICON_BTN_DANGER}
                title={t('common.delete')}
              >
                <Trash2 className="h-4 w-4" aria-hidden />
              </button>
            </div>
          ),
        }]
      : []),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
      <div className="flex shrink-0 flex-col sm:flex-row sm:items-center sm:justify-end gap-4">
        {canManage && (
          <Button onClick={() => setShowCreateModal(true)} leftIcon={<Plus className="h-4 w-4" />}>
            {t('catalog.addCollection')}
          </Button>
        )}
      </div>

      <Card className="shrink-0">
        <div className="flex gap-2">
          <SearchInput
            value={searchDraft}
            onChange={setSearchDraft}
            placeholder={t('catalog.searchCollections')}
            submitMode
            onSubmit={handleSearch}
            submitLabel={t('common.search')}
          />
        </div>
      </Card>

      <Card padding="none" className="flex min-h-0 flex-1 flex-col">
        {data ? (
          <ListResultCountBar>{t('catalog.collectionCount', { count: data.total })}</ListResultCountBar>
        ) : null}
        <ScrollableListRegion fill aria-label={t('catalog.tabCollections')}>
          {isLoading && !(data?.items?.length) ? (
            <ListSkeleton rows={8} />
          ) : (
            <ResponsiveRecordList
              desktop={
                <Table
                  columns={columns}
                  data={data?.items ?? []}
                  keyExtractor={(c) => c.id ?? String(Math.random())}
                  isLoading={false}
                  emptyMessage={t('catalog.noCollections')}
                  onRowClick={onSelect ? (c) => c.id && onSelect(c.id, collName(c)) : undefined}
                />
              }
              mobile={
                (data?.items ?? []).length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                    {t('catalog.noCollections')}
                  </div>
                ) : (
                  <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2 divide-y divide-gray-100 dark:divide-gray-800">
                    {(data?.items ?? []).map((c, idx) => (
                      <div
                        key={c.id ?? `collection-${idx}`}
                        className="flex items-stretch gap-2 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/50"
                      >
                        <button
                          type="button"
                          className="flex-1 min-w-0 text-left flex items-center gap-2"
                          onClick={() => c.id && onSelect?.(c.id, collName(c))}
                        >
                          <Layers className="h-5 w-5 text-amber-600 shrink-0" />
                          <div className="min-w-0">
                            <p className="font-medium text-gray-900 dark:text-white truncate">{collName(c)}</p>
                            {c.secondaryTitle && (
                              <p className="text-xs text-gray-500 dark:text-gray-400 truncate">{c.secondaryTitle}</p>
                            )}
                            <p className="text-xs text-gray-500 font-mono mt-0.5">{c.issn ?? '—'}</p>
                          </div>
                        </button>
                        {canManage && (
                          <div className="flex items-center gap-1 shrink-0">
                            <button
                              type="button"
                              className={`${LIST_ROW_ICON_BTN} min-h-[2.75rem] min-w-[2.75rem]`}
                              title={t('common.edit')}
                              onClick={() => setEditItem(c)}
                            >
                              <Edit className="h-4 w-4" aria-hidden />
                            </button>
                            <button
                              type="button"
                              className={`${LIST_ROW_ICON_BTN_DANGER} min-h-[2.75rem] min-w-[2.75rem]`}
                              title={t('common.delete')}
                              onClick={() => {
                                setDeleteError(null);
                                setDeleteItem(c);
                              }}
                            >
                              <Trash2 className="h-4 w-4" aria-hidden />
                            </button>
                          </div>
                        )}
                        <button
                          type="button"
                          className="self-center p-2 text-gray-400"
                          onClick={() => c.id && onSelect?.(c.id, collName(c))}
                        >
                          <ChevronRight className="h-5 w-5" />
                        </button>
                      </div>
                    ))}
                  </div>
                )
              }
            />
          )}
        </ScrollableListRegion>
        {data && data.pageCount > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-gray-200 dark:border-gray-700 flex-shrink-0">
            <span className="text-sm text-gray-500 dark:text-gray-400">
              {t('common.page')} {page} / {data.pageCount}
            </span>
            <div className="flex gap-2">
              <Button variant="secondary" size="sm" onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page <= 1}>
                <ChevronLeft className="h-4 w-4" />
              </Button>
              <Button variant="secondary" size="sm" onClick={() => setPage((p) => Math.min(data.pageCount, p + 1))} disabled={page >= data.pageCount}>
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>
          </div>
        )}
      </Card>

      {/* Create modal */}
      <Modal isOpen={showCreateModal} onClose={() => setShowCreateModal(false)} title={t('catalog.addCollection')} size="md">
        <CollectionForm
          onSuccess={() => { setShowCreateModal(false); void queryClient.invalidateQueries({ queryKey: ['collections'] }); }}
          onCancel={() => setShowCreateModal(false)}
        />
      </Modal>

      {/* Edit modal */}
      <Modal isOpen={!!editItem} onClose={() => setEditItem(null)} title={t('catalog.editCollection')} size="md">
        {editItem && (
          <CollectionForm
            initial={editItem}
            onSuccess={() => { setEditItem(null); void queryClient.invalidateQueries({ queryKey: ['collections'] }); }}
            onCancel={() => setEditItem(null)}
          />
        )}
      </Modal>

      {/* Delete modal */}
      <Modal
        isOpen={!!deleteItem}
        onClose={() => { if (!deleteLoading) { setDeleteItem(null); setDeleteError(null); } }}
        title={t('common.confirm')}
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={() => { setDeleteItem(null); setDeleteError(null); }} disabled={deleteLoading}>
              {t('common.cancel')}
            </Button>
            <Button variant="danger" onClick={handleDelete} isLoading={deleteLoading}>
              {t('common.delete')}
            </Button>
          </div>
        }
      >
        <p className="text-gray-600 dark:text-gray-300 text-sm">
          {t('catalog.deleteCollectionConfirm', { name: deleteItem ? collName(deleteItem) : '' })}
        </p>
        {deleteError && (
          <div className="mt-3 flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
            <AlertCircle className="h-4 w-4 flex-shrink-0" />
            {deleteError}
          </div>
        )}
      </Modal>
    </div>
  );
}

interface CollectionFormProps {
  initial?: Collection;
  onSuccess: () => void;
  onCancel: () => void;
}

function CollectionForm({ initial, onSuccess, onCancel }: CollectionFormProps) {
  const { t } = useTranslation();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [formData, setFormData] = useState({
    name: initial?.name ?? '',
    secondaryTitle: initial?.secondaryTitle ?? '',
    tertiaryTitle: initial?.tertiaryTitle ?? '',
    issn: initial?.issn ?? '',
  });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.name.trim()) return;
    setIsLoading(true);
    setError(null);
    try {
      const payload = {
        name: formData.name.trim(),
        secondaryTitle: formData.secondaryTitle.trim() || undefined,
        tertiaryTitle: formData.tertiaryTitle.trim() || undefined,
        issn: formData.issn.trim() || undefined,
      };
      if (initial?.id) {
        await api.updateCollection(initial.id, payload);
      } else {
        await api.createCollection(payload);
      }
      onSuccess();
    } catch (err) {
      setError(getApiErrorMessage(err, t));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <Input
        label={t('catalog.collectionName')}
        value={formData.name}
        onChange={(e) => setFormData({ ...formData, name: e.target.value })}
        required
      />
      <Input
        label={t('catalog.collectionSecondaryTitle')}
        value={formData.secondaryTitle}
        onChange={(e) => setFormData({ ...formData, secondaryTitle: e.target.value })}
      />
      <Input
        label={t('catalog.collectionTertiaryTitle')}
        value={formData.tertiaryTitle}
        onChange={(e) => setFormData({ ...formData, tertiaryTitle: e.target.value })}
      />
      <Input
        label="ISSN"
        value={formData.issn}
        onChange={(e) => setFormData({ ...formData, issn: e.target.value })}
        placeholder="0000-0000"
      />
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
          <AlertCircle className="h-4 w-4 flex-shrink-0" />
          {error}
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="secondary" onClick={onCancel}>{t('common.cancel')}</Button>
        <Button type="submit" isLoading={isLoading}>{initial ? t('common.save') : t('common.create')}</Button>
      </div>
    </form>
  );
}

// ─── Series Tab ───────────────────────────────────────────────────────────────

const SERIES_PAGE_SIZE = 25;

interface SeriesTabProps {
  canManage: boolean;
  onSelect?: (id: string, name: string) => void;
}

async function fetchSeriesPage(q: string, p: number): Promise<PaginatedResponse<Serie>> {
  const trimmed = q.trim();
  if (looksLikeEntityUuid(trimmed)) {
    const one = await api.getSerie(trimmed).catch(() => null);
    if (one) {
      return {
        items: [one],
        total: 1,
        page: 1,
        perPage: SERIES_PAGE_SIZE,
        pageCount: 1,
      };
    }
  }
  return api.getSeries({ name: q || undefined, page: p, perPage: SERIES_PAGE_SIZE });
}

function SeriesTab({ canManage, onSelect }: SeriesTabProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [searchDraft, setSearchDraft] = useState('');
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const { data = null, isLoading, refetch } = useQuery({
    queryKey: ['series', search, page],
    queryFn: () => fetchSeriesPage(search, page),
  });
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editItem, setEditItem] = useState<Serie | null>(null);
  const [deleteItem, setDeleteItem] = useState<Serie | null>(null);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleSearch = () => { setPage(1); setSearch(searchDraft); };

  const handleDelete = async () => {
    if (!deleteItem?.id) return;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await api.deleteSerie(deleteItem.id);
      setDeleteItem(null);
      void queryClient.invalidateQueries({ queryKey: ['series'] });
      void refetch();
    } catch (err) {
      setDeleteError(getApiErrorMessage(err, t));
    } finally {
      setDeleteLoading(false);
    }
  };

  const columns = [
    {
      key: 'name',
      header: t('catalog.serieName'),
      render: (s: Serie) => (
        <p className="font-medium text-gray-900 dark:text-white">{s.name ?? '—'}</p>
      ),
    },
    {
      key: 'issn',
      header: 'ISSN',
      render: (s: Serie) => <span className="text-gray-600 dark:text-gray-300">{s.issn ?? '—'}</span>,
      className: 'hidden md:table-cell',
    },
    {
      key: 'key',
      header: t('catalog.key'),
      render: (s: Serie) => <span className="text-xs text-gray-500 dark:text-gray-400 font-mono">{s.key ?? '—'}</span>,
      className: 'hidden lg:table-cell',
    },
    ...(canManage
      ? [{
          key: 'actions',
          header: '',
          align: 'right' as const,
          render: (s: Serie) => (
            <div className="flex justify-end gap-1">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setEditItem(s);
                }}
                className={LIST_ROW_ICON_BTN}
                title={t('common.edit')}
              >
                <Edit className="h-4 w-4" aria-hidden />
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setDeleteError(null);
                  setDeleteItem(s);
                }}
                className={LIST_ROW_ICON_BTN_DANGER}
                title={t('common.delete')}
              >
                <Trash2 className="h-4 w-4" aria-hidden />
              </button>
            </div>
          ),
        }]
      : []),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
      <div className="flex shrink-0 flex-col sm:flex-row sm:items-center sm:justify-end gap-4">
        {canManage && (
          <Button onClick={() => setShowCreateModal(true)} leftIcon={<Plus className="h-4 w-4" />}>
            {t('catalog.addSerie')}
          </Button>
        )}
      </div>

      <Card className="shrink-0">
        <div className="flex gap-2">
          <SearchInput
            value={searchDraft}
            onChange={setSearchDraft}
            placeholder={t('catalog.searchSeries')}
            submitMode
            onSubmit={handleSearch}
            submitLabel={t('common.search')}
          />
        </div>
      </Card>

      <Card padding="none" className="flex min-h-0 flex-1 flex-col">
        {data ? (
          <ListResultCountBar>{t('catalog.serieCount', { count: data.total })}</ListResultCountBar>
        ) : null}
        <ScrollableListRegion fill aria-label={t('catalog.tabSeries')}>
          {isLoading && !(data?.items?.length) ? (
            <ListSkeleton rows={8} />
          ) : (
            <ResponsiveRecordList
              desktop={
                <Table
                  columns={columns}
                  data={data?.items ?? []}
                  keyExtractor={(s) => s.id ?? String(Math.random())}
                  isLoading={false}
                  emptyMessage={t('catalog.noSeries')}
                  onRowClick={onSelect ? (s) => s.id && onSelect(s.id, s.name ?? '') : undefined}
                />
              }
              mobile={
                (data?.items ?? []).length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                    {t('catalog.noSeries')}
                  </div>
                ) : (
                  <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2 divide-y divide-gray-100 dark:divide-gray-800">
                    {(data?.items ?? []).map((s, idx) => (
                      <div
                        key={s.id ?? `serie-${idx}`}
                        className="flex items-stretch gap-2 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/50"
                      >
                        <button
                          type="button"
                          className="flex-1 min-w-0 text-left flex items-center gap-2"
                          onClick={() => s.id && onSelect?.(s.id, s.name ?? '')}
                        >
                          <BookMarked className="h-5 w-5 text-amber-600 shrink-0" />
                          <div className="min-w-0">
                            <p className="font-medium text-gray-900 dark:text-white truncate">{s.name ?? '—'}</p>
                            <p className="text-xs text-gray-600 dark:text-gray-300">{s.issn ?? '—'}</p>
                            <p className="text-xs text-gray-500 font-mono mt-0.5">{s.key ?? '—'}</p>
                          </div>
                        </button>
                        {canManage && (
                          <div className="flex items-center gap-1 shrink-0">
                            <button
                              type="button"
                              className={`${LIST_ROW_ICON_BTN} min-h-[2.75rem] min-w-[2.75rem]`}
                              title={t('common.edit')}
                              onClick={() => setEditItem(s)}
                            >
                              <Edit className="h-4 w-4" aria-hidden />
                            </button>
                            <button
                              type="button"
                              className={`${LIST_ROW_ICON_BTN_DANGER} min-h-[2.75rem] min-w-[2.75rem]`}
                              title={t('common.delete')}
                              onClick={() => {
                                setDeleteError(null);
                                setDeleteItem(s);
                              }}
                            >
                              <Trash2 className="h-4 w-4" aria-hidden />
                            </button>
                          </div>
                        )}
                        <button
                          type="button"
                          className="self-center p-2 text-gray-400"
                          onClick={() => s.id && onSelect?.(s.id, s.name ?? '')}
                        >
                          <ChevronRight className="h-5 w-5" />
                        </button>
                      </div>
                    ))}
                  </div>
                )
              }
            />
          )}
        </ScrollableListRegion>
        {data && data.pageCount > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-gray-200 dark:border-gray-700 flex-shrink-0">
            <span className="text-sm text-gray-500 dark:text-gray-400">
              {t('common.page')} {page} / {data.pageCount}
            </span>
            <div className="flex gap-2">
              <Button variant="secondary" size="sm" onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page <= 1}>
                <ChevronLeft className="h-4 w-4" />
              </Button>
              <Button variant="secondary" size="sm" onClick={() => setPage((p) => Math.min(data.pageCount, p + 1))} disabled={page >= data.pageCount}>
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>
          </div>
        )}
      </Card>

      {/* Create modal */}
      <Modal isOpen={showCreateModal} onClose={() => setShowCreateModal(false)} title={t('catalog.addSerie')} size="md">
        <SerieForm
          onSuccess={() => { setShowCreateModal(false); void queryClient.invalidateQueries({ queryKey: ['series'] }); }}
          onCancel={() => setShowCreateModal(false)}
        />
      </Modal>

      {/* Edit modal */}
      <Modal isOpen={!!editItem} onClose={() => setEditItem(null)} title={t('catalog.editSerie')} size="md">
        {editItem && (
          <SerieForm
            initial={editItem}
            onSuccess={() => { setEditItem(null); void queryClient.invalidateQueries({ queryKey: ['series'] }); }}
            onCancel={() => setEditItem(null)}
          />
        )}
      </Modal>

      {/* Delete modal */}
      <Modal
        isOpen={!!deleteItem}
        onClose={() => { if (!deleteLoading) { setDeleteItem(null); setDeleteError(null); } }}
        title={t('common.confirm')}
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={() => { setDeleteItem(null); setDeleteError(null); }} disabled={deleteLoading}>
              {t('common.cancel')}
            </Button>
            <Button variant="danger" onClick={handleDelete} isLoading={deleteLoading}>
              {t('common.delete')}
            </Button>
          </div>
        }
      >
        <p className="text-gray-600 dark:text-gray-300 text-sm">
          {t('catalog.deleteSerieConfirm', { name: deleteItem?.name ?? '' })}
        </p>
        {deleteError && (
          <div className="mt-3 flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
            <AlertCircle className="h-4 w-4 flex-shrink-0" />
            {deleteError}
          </div>
        )}
      </Modal>
    </div>
  );
}

interface SerieFormProps {
  initial?: Serie;
  onSuccess: () => void;
  onCancel: () => void;
}

function SerieForm({ initial, onSuccess, onCancel }: SerieFormProps) {
  const { t } = useTranslation();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [formData, setFormData] = useState({
    name: initial?.name ?? '',
    issn: initial?.issn ?? '',
  });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!formData.name.trim()) return;
    setIsLoading(true);
    setError(null);
    try {
      const payload = {
        name: formData.name.trim(),
        issn: formData.issn.trim() || undefined,
      };
      if (initial?.id) {
        await api.updateSerie(initial.id, payload);
      } else {
        await api.createSerie(payload);
      }
      onSuccess();
    } catch (err) {
      setError(getApiErrorMessage(err, t));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <Input
        label={t('catalog.serieName')}
        value={formData.name}
        onChange={(e) => setFormData({ ...formData, name: e.target.value })}
        required
      />
      <Input
        label="ISSN"
        value={formData.issn}
        onChange={(e) => setFormData({ ...formData, issn: e.target.value })}
        placeholder="0000-0000"
      />
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
          <AlertCircle className="h-4 w-4 flex-shrink-0" />
          {error}
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="secondary" onClick={onCancel}>{t('common.cancel')}</Button>
        <Button type="submit" isLoading={isLoading}>{initial ? t('common.save') : t('common.create')}</Button>
      </div>
    </form>
  );
}
