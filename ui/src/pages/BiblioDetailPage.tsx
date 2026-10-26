import { useState, useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams, useNavigate, useLocation, useSearchParams } from 'react-router-dom';
import {
  ArrowLeft,
  Edit,
  Trash2,
  BookOpen,
  Calendar,
  User,
  Hash,
  Building,
  MapPin,
  FileText,
  Tag,
  Plus,
  ScanLine,
  AlertCircle,
  ExternalLink,
  Bookmark,
  RefreshCw,
} from 'lucide-react';
import { Card, CardHeader, Button, Badge, Modal, Input, BarcodeScanField } from '@/components/common';
import CallNumberField from '@/components/specimen/CallNumberField';
import {
  buildCallNumberOptions,
  type CallNumberOption,
  validateCallNumber,
} from '@/utils/callNumber';
import { formChoiceControlClass, formControlClass, formLabelClass } from '@/utils/formControl';
import { useAuth } from '@/contexts/AuthContext';
import { canArchiveItems, canCreateHold, canDeleteBiblios, canManageItems, canManageWeeding, canReadUsers, seesInternalCatalog, type MediaType } from '@/types';
import CopyLoanStatus from '@/components/items/CopyLoanStatus';
import NoticeHoldsWaiting from '@/components/holds/NoticeHoldsWaiting';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';
import PlaceHoldDialog from '@/components/holds/PlaceHoldDialog';
import BatchDeleteSpecimensDialog from '@/components/specimen/BatchDeleteSpecimensDialog';
import WeedingStatusControl from '@/components/items/WeedingStatusControl';
import WithdrawalDialog from '@/components/items/WithdrawalDialog';
import api from '@/services/api';
import { CopyNoteFields, CopyNotes } from '@/components/items/CopyNotes';
import DepositBadge, { DepositLossNotifiedLine } from '@/components/items/DepositBadge';
import ItemAvailabilityBadge, { ItemHoldPatronLine } from '@/components/items/ItemAvailabilityBadge';
import type { Author, AuthorPublic, BiblioStaffView, ItemPublicView, ItemStaffView, Site } from '@/types';
import { isBiblioStaffView, isItemStaffView } from '@/utils/catalogView';
import { isItemWithdrawn, isTitleWithdrawn } from '@/utils/weeding';
import { isDepositCopy } from '@/utils/deposits';
import { isCatalogDeletionRefusal } from '@/utils/withdrawals';
import { staffUnavailableDetailKey, type AvailabilityAudience } from '@/utils/itemAvailability';
import { useTranslation } from 'react-i18next';
import { FUNCTION_OPTIONS, PUBLIC_TYPE_OPTIONS, getCodeLabel } from '@/utils/codeLabels';
import { useGenreLabel, useGenreShelfCodes } from '@/hooks/useGenresQuery';
import { formatCatalogLanguage } from '@/utils/catalogLanguage';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import {
  isCirculationReadinessError,
  isReadyToCirculate,
  missingCirculationFields,
  recommendedCirculationFields,
  type CirculationField,
} from '@/utils/circulationReadiness';
import { COPY_NOTE_MAX_CHARS, copyNoteUpdate, copyNoteWithinLimit } from '@/utils/itemNotes';
import { catalogListPath, type CatalogSavedSearch } from '@/utils/catalogSearchState';
import { navigateBack } from '@/utils/navigateBack';
import ItemQuickCreateForm from '@/components/items/ItemQuickCreateForm';

interface BibliosListCache {
  pages?: Array<{
    items?: Array<{ id?: string | null }>;
    total?: number;
  }>;
}

// Helper function to get translation key for media type
function getMediaTypeTranslationKey(mediaType: MediaType | string | null | undefined): string {
  if (!mediaType) return 'unknown';
  const legacyMap: Record<string, string> = {
    u: 'unknown',
    b: 'printedText',
    bc: 'comics',
    p: 'periodic',
    v: 'video',
    vt: 'videoTape',
    vd: 'videoDvd',
    a: 'audio',
    am: 'audioMusic',
    amt: 'audioMusicTape',
    amc: 'audioMusicCd',
    an: 'audioNonMusic',
    ant: 'audioNonMusicTape',
    anc: 'audioNonMusicCd',
    c: 'cdRom',
    i: 'images',
    m: 'multimedia',
  };
  return legacyMap[String(mediaType)] ?? String(mediaType);
}

/** Call-number format options: Dewey / genre / legacy when constructible. */
function getCallNumberOptionsFromItem(
  item: {
    dewey?: string | null;
    genre?: string | null;
    mediaType?: string | null;
    publicationDate?: string | null;
    edition?: { date?: string | null } | null;
    authors?: Array<{ lastname?: string | null; firstname?: string | null; function?: string | null }> | null;
  },
  genreShelfCodes?: Record<string, string> | null,
): CallNumberOption[] {
  return buildCallNumberOptions({
    dewey: item.dewey,
    genre: item.genre,
    authors: item.authors,
    mediaType: item.mediaType,
    publicationDate:
      item.publicationDate?.trim().slice(0, 4) ||
      item.edition?.date?.trim().slice(0, 4) ||
      undefined,
    genreShelfCodes,
  });
}

function volumeBadgeVisible(vol: number | null | undefined): vol is number {
  return vol != null && vol !== 0;
}

function hasNonEmptyText(value: string | null | undefined): boolean {
  return typeof value === 'string' && value.trim().length > 0;
}

function defaultSiteIdFromList(sites: Site[]): string {
  return sites.find((s) => s.default)?.id ?? sites[0]?.id ?? '';
}

export default function BiblioDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { user } = useAuth();
  const { t, i18n } = useTranslation();
  const genreLabel = useGenreLabel();
  const genreShelfCodes = useGenreShelfCodes();
  const navigate = useNavigate();
  const location = useLocation();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();

  const {
    data: item,
    isPending: isLoading,
    isError: isBiblioQueryError,
    error: biblioQueryError,
    refetch: refetchBiblio,
  } = useQuery({
    queryKey: ['biblio', id],
    queryFn: () => api.getBiblio(id!),
    enabled: Boolean(id),
  });

  // Search state saved by BibliosPage when navigating here; used to restore on back
  const savedSearch = (location.state as { savedSearch?: CatalogSavedSearch } | null)?.savedSearch;
  const [showAddSpecimenModal, setShowAddSpecimenModal] = useState(false);
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [showEditSpecimenModal, setShowEditSpecimenModal] = useState(false);
  const [showDeleteSpecimenModal, setShowDeleteSpecimenModal] = useState(false);
  const [isAddSpecimenLoading, setIsAddSpecimenLoading] = useState(false);
  const [isEditSpecimenLoading, setIsEditSpecimenLoading] = useState(false);
  const [selectedSpecimen, setSelectedSpecimen] = useState<ItemStaffView | null>(null);
  const [deleteSpecimenBorrowedError, setDeleteSpecimenBorrowedError] = useState(false);
  const [deleteSpecimenLoading, setDeleteSpecimenLoading] = useState(false);
  const [deleteItemBorrowedError, setDeleteItemBorrowedError] = useState(false);
  const [deleteItemLoading, setDeleteItemLoading] = useState(false);
  const [deleteBiblioApiError, setDeleteBiblioApiError] = useState<string | null>(null);
  const [deleteSpecimenApiError, setDeleteSpecimenApiError] = useState<string | null>(null);
  const [reserveSpecimen, setReserveSpecimen] = useState<ItemStaffView | ItemPublicView | null>(null);
  const [holdDialogOpen, setHoldDialogOpen] = useState(false);
  const [showBatchDeleteSpecimens, setShowBatchDeleteSpecimens] = useState(false);
  const [selectedCopyIds, setSelectedCopyIds] = useState<string[]>([]);
  const [batchWithdrawOpen, setBatchWithdrawOpen] = useState(false);
  const token = api.getToken();
  const canWeed = canManageWeeding(user, token);
  const canArchive = canArchiveItems(user, token);
  const canDeleteBiblio = canDeleteBiblios(user, token);

  const invalidateBiblio = () => {
    if (id) void queryClient.invalidateQueries({ queryKey: ['biblio', id] });
  };

  const completeItemId = searchParams.get('completeItem');
  const staffNotice = item && isBiblioStaffView(item) ? item : null;
  const urlCompleteSpecimen =
    completeItemId && staffNotice
      ? staffNotice.items.find((copy) => copy.id === completeItemId) ?? null
      : null;
  const editingSpecimen = selectedSpecimen ?? urlCompleteSpecimen;
  const editSpecimenOpen = showEditSpecimenModal || urlCompleteSpecimen != null;

  const closeEditSpecimen = () => {
    setShowEditSpecimenModal(false);
    setSelectedSpecimen(null);
    if (searchParams.has('completeItem')) {
      const next = new URLSearchParams(searchParams);
      next.delete('completeItem');
      setSearchParams(next, { replace: true });
    }
  };

  const handleDelete = async (force = false) => {
    if (!item || item.id == null) return;
    if (deleteItemLoading) return;
    setDeleteItemLoading(true);
    setDeleteBiblioApiError(null);
    try {
      await api.deleteBiblio(item.id, force);
      // Ensure deleted biblio disappears from cached catalog searches immediately.
      queryClient.setQueriesData({ queryKey: ['biblios'] }, (old: BibliosListCache | undefined) => {
        if (!old?.pages?.length) return old;
        return {
          ...old,
          pages: old.pages.map((p) => {
            if (!p?.items?.length) return p;
            const nextItems = p.items.filter((it) => it?.id !== item.id);
            const nextTotal = typeof p.total === 'number' ? Math.max(0, p.total - (p.items.length - nextItems.length)) : p.total;
            return { ...p, items: nextItems, total: nextTotal };
          }),
        };
      });
      queryClient.invalidateQueries({ queryKey: ['biblios'] });
      navigate('/biblios');
    } catch (error: unknown) {
      const code = getApiErrorCode(error);
      const msg = (error as { response?: { data?: { message?: string } } })?.response?.data?.message ?? '';
      if (isCatalogDeletionRefusal(code)) {
        setDeleteItemBorrowedError(false);
        setDeleteBiblioApiError(getApiErrorMessage(error, t));
      } else if (
        !force &&
        (
          code === 'business_rule_violation' ||
          code === 'conflict' ||
          (
            typeof msg === 'string' &&
            (msg.includes('borrowed') || msg.includes('force=true'))
          )
        )
      ) {
        setDeleteItemBorrowedError(true);
      } else {
        setDeleteBiblioApiError(getApiErrorMessage(error, t));
      }
    } finally {
      setDeleteItemLoading(false);
    }
  };

  const formatAuthors = (authors?: Array<Author | AuthorPublic>) => {
    if (!authors || authors.length === 0) return t('items.notSpecified');
    return authors
      .map((a) => {
        const name = `${a.firstname || ''} ${a.lastname || ''}`.trim();
        const func = a.function ? getCodeLabel(t, FUNCTION_OPTIONS, a.function) : '';
        return func ? `${name} (${func})` : name;
      })
      .join(', ');
  };

  const handleDeleteSpecimen = async (force: boolean) => {
    if (!selectedSpecimen?.id || !item || item.id == null) return;
    setDeleteSpecimenLoading(true);
    setDeleteSpecimenApiError(null);
    try {
      await api.deleteItem(selectedSpecimen.id, force);
      setShowDeleteSpecimenModal(false);
      setSelectedSpecimen(null);
      setDeleteSpecimenBorrowedError(false);
      invalidateBiblio();
    } catch (error: unknown) {
      const code = getApiErrorCode(error);
      if (isCatalogDeletionRefusal(code)) {
        setDeleteSpecimenBorrowedError(false);
        setDeleteSpecimenApiError(getApiErrorMessage(error, t));
      } else if (!force) {
        const msg = (error as { response?: { data?: { message?: string } } })?.response?.data?.message ?? '';
        if (
          code === 'business_rule_violation' ||
          code === 'conflict' ||
          (typeof msg === 'string' && (msg.includes('borrowed') || msg.includes('force=true')))
        ) {
          setDeleteSpecimenBorrowedError(true);
        } else {
          setDeleteSpecimenApiError(getApiErrorMessage(error, t));
        }
      } else {
        setDeleteSpecimenApiError(getApiErrorMessage(error, t));
      }
    } finally {
      setDeleteSpecimenLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="h-8 w-8 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const biblioBackFallback = seesInternalCatalog(user, api.getToken())
    ? savedSearch !== undefined
      ? catalogListPath(savedSearch)
      : '/biblios'
    : '/catalog';

  if (isBiblioQueryError) {
    return (
      <div className="max-w-xl mx-auto px-4 py-12">
        <Card>
          <div className="flex items-start gap-3 p-4">
            <AlertCircle className="h-5 w-5 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
            <div className="flex-1 space-y-3">
              <p className="text-gray-900 dark:text-gray-100 text-sm">{getApiErrorMessage(biblioQueryError, t)}</p>
              <div className="flex flex-wrap gap-2">
                <Button
                  type="button"
                  leftIcon={<RefreshCw className="h-4 w-4" />}
                  onClick={() => void refetchBiblio()}
                >
                  {t('common.retry')}
                </Button>
                <Button type="button" variant="secondary" onClick={() => navigateBack(navigate, biblioBackFallback)}>
                  {t('common.back')}
                </Button>
              </div>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  if (!item) {
    return (
      <div className="text-center py-12">
        <p className="text-gray-500 dark:text-gray-400">{t('items.biblioNotFound')}</p>
      </div>
    );
  }

  const hasGeneralInfoFields =
    hasNonEmptyText(item.isbn) ||
    (item.authors?.length ?? 0) > 0 ||
    hasNonEmptyText(item.publicationDate) ||
    hasNonEmptyText(item.edition?.publisherName) ||
    hasNonEmptyText(item.edition?.placeOfPublication) ||
    (item.lang != null && item.lang !== '') ||
    item.audienceType != null;
  const hasGeneralInfoMeta = Boolean(item.createdAt) || Boolean(item.updatedAt);
  const showGeneralInfoCard = hasGeneralInfoFields || hasGeneralInfoMeta;
  const createdAt = item.createdAt;
  const updatedAt = item.updatedAt;
  const metaDatesAreSameInstant =
    createdAt != null &&
    updatedAt != null &&
    new Date(createdAt).getTime() === new Date(updatedAt).getTime();
  const formatBiblioMetaDate = (iso: string) =>
    new Date(iso).toLocaleString(i18n.language, {
      dateStyle: 'medium',
      timeStyle: 'short',
    });
  const titleWithdrawn = isBiblioStaffView(item) && isTitleWithdrawn(item);
  const canPlaceTitleHold =
    !!user?.id &&
    !!item.id &&
    !titleWithdrawn &&
    canCreateHold(user, api.getToken());

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
        <div className="flex items-start gap-4">
          <button
            type="button"
            onClick={() => navigateBack(navigate, biblioBackFallback)}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800"
            aria-label={t('common.back')}
          >
            <ArrowLeft className="h-5 w-5" />
          </button>
          <LoanMediaTypeBadge
            mediaType={item.mediaType}
            size="comfortable"
            depositCopies={item.items}
            className="flex-shrink-0"
          />
          <div>
            <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white">
              {item.title || t('items.noTitle')}
            </h1>
            <div className="flex items-center gap-2 mt-2">
              <Badge>
                {item.mediaType 
                  ? t(`items.mediaType.${getMediaTypeTranslationKey(item.mediaType)}`)
                  : t('items.document')
                }
              </Badge>
              {item.audienceType != null && (
                <Badge variant="default">
                  {getCodeLabel(t, PUBLIC_TYPE_OPTIONS, item.audienceType)}
                </Badge>
              )}
              {item.isValid === false && <Badge variant="warning">{t('items.notValidated')}</Badge>}
              {isBiblioStaffView(item) && isTitleWithdrawn(item) && (
                <Badge variant="danger">{t('weeding.status.withdrawn')}</Badge>
              )}
            </div>
            <NoticeHoldsWaiting holdsCount={item.holdsCount} />
          </div>
        </div>

        {(canPlaceTitleHold || canManageItems(user, api.getToken())) && (
          <div className="flex flex-wrap gap-2">
            {canPlaceTitleHold && (
              <Button
                variant="primary"
                leftIcon={<Bookmark className="h-4 w-4" />}
                onClick={() => {
                  setReserveSpecimen(null);
                  setHoldDialogOpen(true);
                }}
              >
                {t('holds.reserve')}
              </Button>
            )}
            {canManageItems(user, api.getToken()) && (
              <>
                <Button
                  variant="secondary"
                  onClick={() =>
                    navigate(`/biblios/${item.id}/edit`, {
                      state: savedSearch !== undefined ? { savedSearch } : undefined,
                    })
                  }
                  leftIcon={<Edit className="h-4 w-4" />}
                >
                  {t('common.edit')}
                </Button>
                {canDeleteBiblio ? (
                  <Button variant="danger" onClick={() => setShowDeleteModal(true)} leftIcon={<Trash2 className="h-4 w-4" />}>
                    {t('common.delete')}
                  </Button>
                ) : null}
              </>
            )}
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Main info */}
        <div className="lg:col-span-2 space-y-6">
          {showGeneralInfoCard && (
            <Card>
              <CardHeader title={t('items.generalInfo')} />
              {hasGeneralInfoFields && (
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <InfoRow icon={Hash} label={t('items.isbn')} value={formatIsbnDisplay(item.isbn)} />
                  <InfoRow icon={User} label={t('items.mainAuthor')} value={item.authors?.length ? formatAuthors([item.authors[0]]) : undefined} />
                  <InfoRow icon={User} label={t('items.secondaryAuthor')} value={item.authors && item.authors.length > 1 ? formatAuthors(item.authors.slice(1)) : undefined} />
                  <InfoRow icon={Calendar} label={t('items.publicationDate')} value={item.publicationDate} />
                  <InfoRow icon={Building} label={t('items.publisher')} value={item.edition?.publisherName} />
                  <InfoRow icon={MapPin} label={t('items.publicationPlace')} value={item.edition?.placeOfPublication} />
                  <InfoRow
                    icon={BookOpen}
                    label={t('items.language')}
                    value={
                      item.lang != null && item.lang !== ''
                        ? formatCatalogLanguage(item.lang, i18n.language, t('codes.lang.unknown'))
                        : undefined
                    }
                  />
                  <InfoRow
                    icon={Tag}
                    label={t('items.publicType')}
                    value={item.audienceType != null ? getCodeLabel(t, PUBLIC_TYPE_OPTIONS, item.audienceType) : undefined}
                  />
                </div>
              )}
              {hasGeneralInfoMeta && (
                <div
                  className={
                    hasGeneralInfoFields
                      ? 'mt-4 pt-3 border-t border-gray-200 dark:border-gray-700 flex flex-wrap items-baseline gap-x-5 gap-y-1 text-xs text-gray-500 dark:text-gray-400'
                      : 'flex flex-wrap items-baseline gap-x-5 gap-y-1 text-xs text-gray-500 dark:text-gray-400'
                  }
                >
                  {metaDatesAreSameInstant && createdAt ? (
                    <span>
                      {t('items.createdAt')}: {formatBiblioMetaDate(createdAt)}
                    </span>
                  ) : (
                    <>
                      {createdAt && (
                        <span>
                          {t('items.createdAt')}: {formatBiblioMetaDate(createdAt)}
                        </span>
                      )}
                      {updatedAt && (
                        <span>
                          {t('items.updatedAt')}: {formatBiblioMetaDate(updatedAt)}
                        </span>
                      )}
                    </>
                  )}
                </div>
              )}
            </Card>
          )}

          {item.abstract && (
            <Card>
              <CardHeader title={t('items.abstract')} />
              <p className="text-gray-600 dark:text-gray-300 whitespace-pre-wrap">
                {item.abstract}
              </p>
            </Card>
          )}

          {(item.keywords || item.subject || item.dewey || ('genre' in item && item.genre)) && (
            <Card>
              <CardHeader title={t('items.keywordsAndSubject')} />
              <div className="space-y-3">
                {item.keywords && (
                  <div className="flex items-start gap-2">
                    <Tag className="h-4 w-4 mt-1 text-gray-400" />
                    <p className="text-gray-600 dark:text-gray-300">{Array.isArray(item.keywords) ? item.keywords.join(', ') : item.keywords}</p>
                  </div>
                )}
                {item.subject && (
                  <div className="flex items-start gap-2">
                    <FileText className="h-4 w-4 mt-1 text-gray-400" />
                    <p className="text-gray-600 dark:text-gray-300">{item.subject}</p>
                  </div>
                )}
                <div className="flex items-start gap-2">
                  <Hash className="h-4 w-4 mt-1 text-gray-400" />
                  <div>
                    <p className="text-xs font-medium text-gray-500 dark:text-gray-400">{t('items.dewey')}</p>
                    <p className="text-gray-600 dark:text-gray-300 font-mono">
                      {hasNonEmptyText(item.dewey) ? item.dewey : t('items.notSpecified')}
                    </p>
                  </div>
                </div>
                {'genre' in item && item.genre ? (
                  <div className="flex items-start gap-2">
                    <Tag className="h-4 w-4 mt-1 text-gray-400" />
                    <div>
                      <p className="text-xs font-medium text-gray-500 dark:text-gray-400">{t('items.genre')}</p>
                      <p className="text-gray-600 dark:text-gray-300">
                        {genreLabel(item.genre)}
                      </p>
                    </div>
                  </div>
                ) : null}
              </div>
            </Card>
          )}
        </div>

        {/* Specimens */}
        <div className="space-y-6">
          <Card>
            <CardHeader
              title={t('items.specimens')}
              subtitle={t('items.specimenCount', { count: item.items?.length ?? 0 })}
              action={
                canManageItems(user, api.getToken()) && (
                  <div className="flex flex-wrap items-center gap-2">
                    {canWeed && selectedCopyIds.length > 0 && (
                      <Button size="sm" variant="danger" onClick={() => setBatchWithdrawOpen(true)}>
                        {t('weeding.withdrawSelected', { count: selectedCopyIds.length })}
                      </Button>
                    )}
                    {canArchive && (item.items?.length ?? 0) > 0 && (
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => setShowBatchDeleteSpecimens(true)}
                        leftIcon={<ScanLine className="h-4 w-4" />}
                      >
                        {t('items.batchDeleteScan.openButton')}
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => setShowAddSpecimenModal(true)}
                      leftIcon={<Plus className="h-4 w-4" />}
                    >
                      {t('items.addSpecimen')}
                    </Button>
                  </div>
                )
              }
            />
            {item.items && item.items.length > 0 ? (
              <div className="space-y-3">
                {item.items.map((specimen) => (
                  <SpecimenCard
                    key={specimen.id}
                    specimen={specimen}
                    canManage={canManageItems(user, api.getToken())}
                    canDelete={canArchive}
                    selectable={
                      canWeed &&
                      isItemStaffView(specimen) &&
                      specimen.id != null &&
                      !isItemWithdrawn(specimen) &&
                      !isDepositCopy(specimen.deposit)
                    }
                    selected={specimen.id != null && selectedCopyIds.includes(specimen.id)}
                    onToggleSelect={() => {
                      if (specimen.id == null) return;
                      const id = specimen.id;
                      setSelectedCopyIds((prev) =>
                        prev.includes(id) ? prev.filter((value) => value !== id) : [...prev, id],
                      );
                    }}
                    audience={seesInternalCatalog(user, api.getToken()) ? 'staff' : 'public'}
                    showReserveButton={canPlaceTitleHold && specimen.canHoldItem === true}
                    onReserve={() => {
                      setReserveSpecimen(specimen);
                      setHoldDialogOpen(true);
                    }}
                    onUpdated={invalidateBiblio}
                    onEdit={() => {
                      if (!isItemStaffView(specimen)) return;
                      setSelectedSpecimen(specimen);
                      setShowEditSpecimenModal(true);
                    }}
                    onDelete={() => {
                      if (!isItemStaffView(specimen) || specimen.id == null) return;
                      setSelectedSpecimen(specimen);
                      setShowDeleteSpecimenModal(true);
                    }}
                  />
                ))}
              </div>
            ) : (
              <p className="text-center py-4 text-gray-500 dark:text-gray-400">
                {t('items.noSpecimens')}
              </p>
            )}
          </Card>

          {/* Collections — prefer new array, fall back to legacy single */}
          {(() => {
            const cols = item.collections ?? [];
            if (cols.length === 0) return null;
            return (
              <Card>
                <CardHeader title={t('items.collection')} />
                <div className="space-y-3">
                  {cols.map((c, i) => {
                    const volNum =
                      c.volumeNumber ??
                      item.collectionVolumeNumbers?.[i] ??
                      null;
                    const title = c.name ?? c.secondaryTitle ?? '—';
                    const openCollectionCatalog = () => {
                      if (!c.id) return;
                      const params = new URLSearchParams();
                      params.set('collection_id', c.id);
                      params.set('collection_name', title);
                      navigate(`/biblios?${params.toString()}`);
                    };
                    return (
                      <div key={c.id ?? i} className="flex items-start justify-between gap-4">
                        <div className="min-w-0 flex-1">
                          {c.id ? (
                            <button
                              type="button"
                              onClick={openCollectionCatalog}
                              title={t('catalog.openInCatalog')}
                              className="group w-full text-left rounded-lg -m-1 p-1.5 border border-transparent hover:border-amber-200/90 dark:hover:border-amber-800/60 hover:bg-amber-50/90 dark:hover:bg-amber-950/35 transition-colors"
                            >
                              <div className="flex items-start gap-2">
                                <div className="min-w-0 flex-1">
                                  <p className="font-medium text-amber-800 dark:text-amber-200 group-hover:underline underline-offset-2 decoration-amber-600/50">
                                    {title}
                                  </p>
                                  {c.secondaryTitle && (
                                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                      {c.secondaryTitle}
                                    </p>
                                  )}
                                  {c.issn && (
                                    <p className="text-xs text-gray-500 dark:text-gray-400">ISSN: {c.issn}</p>
                                  )}
                                </div>
                                <ExternalLink
                                  className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400 mt-0.5 opacity-80 group-hover:opacity-100"
                                  aria-hidden
                                />
                              </div>
                            </button>
                          ) : (
                            <>
                              <p className="font-medium text-gray-900 dark:text-white">{title}</p>
                              {c.secondaryTitle && (
                                <p className="text-xs text-gray-500 dark:text-gray-400">
                                  {c.secondaryTitle}
                                </p>
                              )}
                              {c.issn && (
                                <p className="text-xs text-gray-500 dark:text-gray-400">ISSN: {c.issn}</p>
                              )}
                            </>
                          )}
                        </div>
                        {volumeBadgeVisible(volNum) && (
                          <span className="shrink-0 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300">
                            {t('catalog.volumeNumber')} {volNum}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </Card>
            );
          })()}

          {item.series && item.series.length > 0 && (
            <Card>
              <CardHeader title={t('items.series')} />
              <div className="space-y-3">
                {item.series.map((s, i) => {
                  const volNum =
                    s.volumeNumber ??
                    s.volumeNumber ??
                    item.seriesVolumeNumbers?.[i] ??
                    item.seriesVolumeNumbers?.[i] ??
                    null;
                  const serieTitle = s.name ?? '—';
                  const openSerieCatalog = () => {
                    if (!s.id) return;
                    const params = new URLSearchParams();
                    params.set('serie_id', s.id);
                    params.set('serie_name', serieTitle);
                    navigate(`/biblios?${params.toString()}`);
                  };
                  return (
                    <div key={s.id ?? i} className="flex items-start justify-between gap-4">
                      <div className="min-w-0 flex-1">
                        {s.id ? (
                          <button
                            type="button"
                            onClick={openSerieCatalog}
                            title={t('catalog.openInCatalog')}
                            className="group w-full text-left rounded-lg -m-1 p-1.5 border border-transparent hover:border-amber-200/90 dark:hover:border-amber-800/60 hover:bg-amber-50/90 dark:hover:bg-amber-950/35 transition-colors"
                          >
                            <div className="flex items-start gap-2">
                              <div className="min-w-0 flex-1">
                                <p className="font-medium text-amber-800 dark:text-amber-200 group-hover:underline underline-offset-2 decoration-amber-600/50">
                                  {serieTitle}
                                </p>
                                {s.issn && (
                                  <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">ISSN: {s.issn}</p>
                                )}
                              </div>
                              <ExternalLink
                                className="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400 mt-0.5 opacity-80 group-hover:opacity-100"
                                aria-hidden
                              />
                            </div>
                          </button>
                        ) : (
                          <>
                            <p className="font-medium text-gray-900 dark:text-white">{serieTitle}</p>
                            {s.issn && (
                              <p className="text-xs text-gray-500 dark:text-gray-400">ISSN: {s.issn}</p>
                            )}
                          </>
                        )}
                      </div>
                      {volumeBadgeVisible(volNum) && (
                        <span className="shrink-0 inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300">
                          {t('catalog.volumeNumber')} {volNum}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </Card>
          )}
        </div>
      </div>

      {/* Delete confirmation modal */}
      <Modal
        isOpen={showDeleteModal}
        onClose={() => {
          if (deleteItemLoading) return;
          setShowDeleteModal(false);
          setDeleteItemBorrowedError(false);
          setDeleteBiblioApiError(null);
        }}
        title={t('common.confirmDeletion')}
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                if (deleteItemLoading) return;
                setShowDeleteModal(false);
                setDeleteItemBorrowedError(false);
                setDeleteBiblioApiError(null);
              }}
            >
              {t('common.cancel')}
            </Button>
            {deleteItemBorrowedError ? (
              <Button variant="danger" disabled={deleteItemLoading} onClick={() => handleDelete(true)}>
                {t('items.forceDeleteItem')}
              </Button>
            ) : (
              <Button variant="danger" disabled={deleteItemLoading} onClick={() => handleDelete(false)}>
                {t('common.delete')}
              </Button>
            )}
          </div>
        }
      >
        <p className="text-gray-600 dark:text-gray-300">
          {deleteItemBorrowedError
            ? t('items.itemBorrowedForceDelete')
            : t('items.deleteConfirm', { title: item.title || t('items.noTitle') })}
        </p>
        {deleteBiblioApiError && (
          <p className="mt-3 text-sm text-red-600 dark:text-red-400">{deleteBiblioApiError}</p>
        )}
      </Modal>

      {/* Add specimen modal */}
      <Modal
        isOpen={showAddSpecimenModal}
        onClose={() => setShowAddSpecimenModal(false)}
        title={t('items.addSpecimen')}
        footer={
          <div className="flex justify-end gap-2">
            <Button type="submit" form="add-specimen-form" isLoading={isAddSpecimenLoading}>
              {t('common.add')}
            </Button>
          </div>
        }
      >
        {staffNotice?.id ? (
          <ItemQuickCreateForm
            formId="add-specimen-form"
            biblioId={String(staffNotice.id)}
            callNumberSuggestions={getCallNumberOptionsFromItem(staffNotice, genreShelfCodes)}
            onLoadingChange={setIsAddSpecimenLoading}
            onSuccess={() => {
              setShowAddSpecimenModal(false);
              invalidateBiblio();
            }}
          />
        ) : null}
      </Modal>

      {/* Edit specimen modal */}
      <Modal
        isOpen={editSpecimenOpen}
        onClose={closeEditSpecimen}
        title={
          editingSpecimen && editingSpecimen.borrowable === false && !isReadyToCirculate(editingSpecimen)
            ? t('items.completeForCirculation')
            : t('items.editSpecimen')
        }
        footer={
          <div className="flex justify-end gap-2">
            <Button type="submit" form="edit-specimen-form" isLoading={isEditSpecimenLoading}>
              {t('common.save')}
            </Button>
          </div>
        }
      >
        {editingSpecimen && staffNotice && isItemStaffView(editingSpecimen) && (
          <EditSpecimenForm
            formId="edit-specimen-form"
            item={staffNotice}
            specimen={editingSpecimen}
            onLoadingChange={setIsEditSpecimenLoading}
            onSuccess={() => {
              closeEditSpecimen();
              invalidateBiblio();
            }}
          />
        )}
      </Modal>

      {/* Delete specimen confirmation modal */}
      <Modal
        isOpen={showDeleteSpecimenModal}
        onClose={() => {
          setShowDeleteSpecimenModal(false);
          setSelectedSpecimen(null);
          setDeleteSpecimenBorrowedError(false);
          setDeleteSpecimenApiError(null);
        }}
        title={t('common.confirm')}
        size="sm"
        footer={
          <div className="flex justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                setShowDeleteSpecimenModal(false);
                setSelectedSpecimen(null);
                setDeleteSpecimenBorrowedError(false);
                setDeleteSpecimenApiError(null);
              }}
            >
              {t('common.cancel')}
            </Button>
            <Button
              variant="danger"
              disabled={deleteSpecimenLoading}
              onClick={() => handleDeleteSpecimen(deleteSpecimenBorrowedError)}
            >
              {deleteSpecimenBorrowedError ? t('items.forceDeleteSpecimen') : t('common.delete')}
            </Button>
          </div>
        }
      >
        {selectedSpecimen && (
          <>
            <p className="text-gray-600 dark:text-gray-300">
              {deleteSpecimenBorrowedError
                ? t('items.specimenBorrowedForceDelete')
                : t('items.confirmDeleteSpecimen', { identification: selectedSpecimen.barcode || 'Sans code' })}
            </p>
            {deleteSpecimenApiError && (
              <p className="mt-3 text-sm text-red-600 dark:text-red-400">{deleteSpecimenApiError}</p>
            )}
          </>
        )}
      </Modal>

      {item && item.id && user?.id && (
        <PlaceHoldDialog
          open={holdDialogOpen}
          onClose={() => {
            setHoldDialogOpen(false);
            setReserveSpecimen(null);
          }}
          biblioId={item.id}
          biblioTitle={item.title || '—'}
          specimen={reserveSpecimen}
          pinCopyDefault={!!reserveSpecimen}
          accountType={user.accountType}
          currentUserId={user.id}
          onSuccess={() => {
            invalidateBiblio();
            void queryClient.invalidateQueries({ queryKey: ['my-holds'] });
            void queryClient.invalidateQueries({ queryKey: ['activeHolds'] });
          }}
        />
      )}

      <WithdrawalDialog
        open={batchWithdrawOpen}
        mode="create"
        itemIds={selectedCopyIds}
        onClose={() => setBatchWithdrawOpen(false)}
        onSuccess={() => {
          setSelectedCopyIds([]);
          invalidateBiblio();
        }}
      />

      {item && item.id != null && canArchive && (
        <BatchDeleteSpecimensDialog
          isOpen={showBatchDeleteSpecimens}
          onClose={() => setShowBatchDeleteSpecimens(false)}
          restrictToBiblio={{
            id: item.id,
            title: item.title,
            authorLabel: formatAuthors(item.authors),
            items: item.items,
          }}
          onSpecimenDeleted={() => invalidateBiblio()}
        />
      )}
    </div>
  );
}

interface InfoRowProps {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value?: string | null;
}

function InfoRow({ icon: Icon, label, value }: InfoRowProps) {
  const text = value === undefined || value === null ? '' : String(value).trim();
  if (!text) return null;
  return (
    <div className="flex items-start gap-3">
      <Icon className="h-5 w-5 text-gray-400 mt-0.5" />
      <div>
        <p className="text-xs text-gray-500 dark:text-gray-400">{label}</p>
        <p className="text-gray-900 dark:text-white">{text}</p>
      </div>
    </div>
  );
}

interface SpecimenCardProps {
  specimen: ItemStaffView | ItemPublicView;
  canManage: boolean;
  canDelete: boolean;
  selectable?: boolean;
  selected?: boolean;
  onToggleSelect?: () => void;
  audience: AvailabilityAudience;
  showReserveButton?: boolean;
  onReserve?: () => void;
  onUpdated?: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function missingFieldLabel(t: (key: string) => string, field: CirculationField): string {
  if (field === 'barcode') return t('items.missingBarcode');
  return t('items.missingSite');
}

function SpecimenCard({
  specimen,
  canManage,
  canDelete,
  selectable = false,
  selected = false,
  onToggleSelect,
  audience,
  showReserveButton,
  onReserve,
  onUpdated,
  onEdit,
  onDelete,
}: SpecimenCardProps) {
  const { t } = useTranslation();
  const { user: authUser } = useAuth();
  const canReadBorrowers = canReadUsers(authUser, api.getToken());
  const staffCopy = isItemStaffView(specimen) ? specimen : null;
  const ready = isReadyToCirculate(specimen);
  const incomplete = specimen.borrowable === false && !ready;
  const missing = audience === 'staff' && incomplete ? missingCirculationFields(specimen) : [];
  const recommended =
    audience === 'staff' && (specimen.borrowable !== false || ready)
      ? recommendedCirculationFields(specimen)
      : [];
  const staffDetailKey = staffCopy ? staffUnavailableDetailKey(staffCopy) : null;

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2 min-w-0">
          {selectable ? (
            <input
              type="checkbox"
              className={formChoiceControlClass()}
              checked={selected}
              onChange={() => onToggleSelect?.()}
              aria-label={t('weeding.selectCopy', { barcode: specimen.barcode || specimen.id || '' })}
            />
          ) : null}
          <div className="min-w-0">
            <p className="font-medium text-gray-900 dark:text-white truncate">
              {specimen.barcode || t('items.noBarcode')}
            </p>
            {staffCopy?.lenderBarcode &&
            staffCopy.lenderBarcode.trim() !== '' &&
            staffCopy.lenderBarcode !== (specimen.barcode ?? '') ? (
              <p className="text-xs text-gray-500 dark:text-gray-400 font-mono truncate">
                {t('deposits.lenderBarcode')}: {staffCopy.lenderBarcode}
              </p>
            ) : null}
            <DepositBadge deposit={specimen.deposit} />
            {audience === 'staff' ? <DepositLossNotifiedLine deposit={specimen.deposit} /> : null}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <ItemAvailabilityBadge availability={specimen.availability} audience={audience} />
          {canManage && (
            <div className="flex gap-1">
              <button
                type="button"
                onClick={onEdit}
                className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-400"
                title={t('common.edit')}
                aria-label={t('common.edit')}
              >
                <Edit className="h-4 w-4" />
              </button>
              {canDelete ? (
                <button
                  type="button"
                  onClick={onDelete}
                  className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-red-600 dark:text-red-400"
                  title={t('common.delete')}
                  aria-label={t('common.delete')}
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              ) : null}
            </div>
          )}
        </div>
      </div>
      {specimen.callNumber && (
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('items.callNumber')}: {specimen.callNumber}</p>
      )}
      {specimen.volumeDesignation && (
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('items.volumeDesignation')}: {specimen.volumeDesignation}</p>
      )}
      <CopyNotes
        audience={audience}
        publicNote={specimen.publicNote}
        internalNote={audience === 'staff' ? staffCopy?.internalNote : undefined}
      />

      <p className="text-sm text-gray-500 dark:text-gray-400">
        {t('items.site')}: {specimen.siteName ?? '—'}
      </p>

      {staffCopy ? (
        <ItemHoldPatronLine
          audience={audience}
          availability={staffCopy.availability}
          holdFor={staffCopy.holdFor}
          linked={canReadBorrowers}
        />
      ) : null}

      {staffDetailKey && (staffDetailKey !== 'items.incompleteNotCirculable' || missing.length === 0) && (
        <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{t(staffDetailKey)}</p>
      )}

      {audience === 'staff' && staffCopy ? (
        <CopyLoanStatus
          audience="staff"
          linked={canReadBorrowers}
          copy={{
            borrowed: staffCopy.borrowed,
            availability: staffCopy.availability,
            dueDate: staffCopy.dueDate,
            onHold: staffCopy.onHold,
            borrower: staffCopy.borrower,
          }}
        />
      ) : (
        <CopyLoanStatus
          audience="public"
          copy={{
            borrowed: specimen.borrowed,
            availability: specimen.availability,
            onHold: specimen.onHold,
            expectedReturnDate:
              'expectedReturnDate' in specimen ? specimen.expectedReturnDate : undefined,
          }}
        />
      )}

      {audience === 'staff' && incomplete && missing.length > 0 ? (
        <p className="mt-2 text-xs text-amber-700 dark:text-amber-400">
          {t('items.missingCirculationFields', {
            fields: missing.map((field) => missingFieldLabel(t, field)).join(', '),
          })}
        </p>
      ) : null}
      {recommended.length > 0 ? (
        <p className="mt-2 text-xs text-gray-600 dark:text-gray-400">{t('items.recommendedPrice')}</p>
      ) : null}

      {canManage && incomplete ? (
        <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-600">
          <Button size="sm" onClick={onEdit}>
            {t('items.completeForCirculation')}
          </Button>
        </div>
      ) : null}

      {showReserveButton && onReserve && (
        <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-600">
          <Button size="sm" variant="ghost" leftIcon={<Bookmark className="h-4 w-4" />} onClick={onReserve}>
            {t('holds.reserveThisCopy')}
          </Button>
        </div>
      )}

      {audience === 'staff' && staffCopy && !isDepositCopy(staffCopy.deposit) ? (
        <WeedingStatusControl item={staffCopy} onUpdated={onUpdated} />
      ) : null}
    </div>
  );
}

interface EditSpecimenFormProps {
  formId: string;
  item: BiblioStaffView;
  specimen: ItemStaffView;
  onLoadingChange: (loading: boolean) => void;
  onSuccess: () => void;
}

function EditSpecimenForm({ formId, item, specimen, onLoadingChange, onSuccess }: EditSpecimenFormProps) {
  const { t } = useTranslation();
  const genreShelfCodes = useGenreShelfCodes();
  const [error, setError] = useState<string | null>(null);
  const [sites, setSites] = useState<Site[]>([]);
  const callNumberSuggestions = getCallNumberOptionsFromItem(item, genreShelfCodes);
  const [formData, setFormData] = useState({
    barcode: specimen.barcode || '',
    callNumber: specimen.callNumber || '',
    volumeDesignation: specimen.volumeDesignation || '',
    place: specimen.place != null ? String(specimen.place) : '',
    publicNote: specimen.publicNote ?? '',
    internalNote: specimen.internalNote ?? '',
    price: specimen.price || '',
    priceDeferred: specimen.priceDeferred === true,
    siteId: specimen.siteId || '',
  });
  const draftReady = isReadyToCirculate({
    barcode: formData.barcode,
    siteId: formData.siteId,
    price: formData.price,
    priceDeferred: formData.priceDeferred,
  });
  const draftMissing = missingCirculationFields({
    barcode: formData.barcode,
    siteId: formData.siteId,
    price: formData.price,
    priceDeferred: formData.priceDeferred,
  });
  const draftRecommended = recommendedCirculationFields({
    barcode: formData.barcode,
    siteId: formData.siteId,
    price: formData.price,
    priceDeferred: formData.priceDeferred,
  });

  useEffect(() => {
    api
      .getSites(false)
      .then((list) => {
        setSites(list);
        setFormData((prev) => {
          if (prev.siteId.trim() !== '') return prev;
          const id = defaultSiteIdFromList(list);
          return id ? { ...prev, siteId: id } : prev;
        });
      })
      .catch(() => {});
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (item.id == null || specimen.id == null) return;
    if (formData.barcode.trim() === '') {
      setError(t('items.specimenBarcodeRequired'));
      return;
    }
    if (!validateCallNumber(formData.callNumber)) return;
    if (!copyNoteWithinLimit(formData.publicNote) || !copyNoteWithinLimit(formData.internalNote)) {
      setError(t('items.noteTooLong', { max: COPY_NOTE_MAX_CHARS }));
      return;
    }
    const specimenId = specimen.id;
    onLoadingChange(true);
    setError(null);
    try {
      const ready = isReadyToCirculate({
        barcode: formData.barcode,
        siteId: formData.siteId,
        price: formData.price,
        priceDeferred: formData.priceDeferred,
      });
      await api.updateItem(specimenId, {
        barcode: formData.barcode.trim(),
        callNumber: formData.callNumber || undefined,
        volumeDesignation: formData.volumeDesignation || undefined,
        borrowable: ready,
        place: formData.place ? parseInt(formData.place, 10) : undefined,
        ...copyNoteUpdate(specimen, {
          publicNote: formData.publicNote,
          internalNote: formData.internalNote,
        }),
        price: formData.priceDeferred ? null : formData.price || undefined,
        priceDeferred: formData.priceDeferred || undefined,
        siteId: formData.siteId || undefined,
      });
      onSuccess();
    } catch (error) {
      const message = getApiErrorMessage(error, t);
      setError(isCirculationReadinessError(message) ? t('items.cannotMakeBorrowable') : message);
    } finally {
      onLoadingChange(false);
    }
  };

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4">
      <BarcodeScanField
        label={t('items.specimenBarcode')}
        value={formData.barcode}
        onChange={(e) => {
          setError(null);
          setFormData({ ...formData, barcode: e.target.value });
        }}
        scannerTitle={t('items.specimenBarcode')}
        onCameraScan={(code) => {
          setError(null);
          setFormData({ ...formData, barcode: code });
        }}
      />
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
          <AlertCircle className="h-4 w-4 flex-shrink-0" />
          {error}
        </div>
      )}
      <CallNumberField
        label={t('items.callNumber')}
        value={formData.callNumber}
        onChange={(v) => setFormData({ ...formData, callNumber: v })}
        suggestions={callNumberSuggestions}
        excludeSpecimenId={specimen.id ?? undefined}
        placeholder={t('items.callNumber')}
      />
      <Input
        label={t('items.volumeDesignation')}
        value={formData.volumeDesignation}
        onChange={(e) => setFormData({ ...formData, volumeDesignation: e.target.value })}
        placeholder={t('items.volumeDesignationPlaceholder')}
      />
      <div>
        <label className={formLabelClass()}>
          {t('items.site')}
        </label>
        <select
          value={formData.siteId}
          onChange={(e) => setFormData({ ...formData, siteId: e.target.value })}
          className={formControlClass({ className: 'w-full' })}
        >
          <option value="">{t('items.selectSite')}</option>
          {sites.map((src) => (
            <option key={src.id} value={src.id}>
              {src.name || src.id}
              {src.default ? ` (${t('importMarc.default')})` : ''}
            </option>
          ))}
        </select>
      </div>
      <Input
        label={t('items.specimenPrice')}
        value={formData.price}
        onChange={(e) => setFormData({ ...formData, price: e.target.value })}
        disabled={formData.priceDeferred}
      />
      <label className="flex items-start gap-2 text-sm text-gray-700 dark:text-gray-300">
        <input
          type="checkbox"
          className="mt-1"
          checked={formData.priceDeferred}
          onChange={(e) => setFormData({ ...formData, priceDeferred: e.target.checked })}
        />
        <span>
          {t('items.priceDeferred')}
          <span className="mt-0.5 block text-xs text-gray-500">{t('items.priceDeferredHelp')}</span>
        </span>
      </label>
      {!draftReady ? (
        <div className="space-y-1">
          <p className="text-sm text-amber-800 dark:text-amber-300">{t('items.circulationRequirements')}</p>
          <p className="text-xs text-amber-700 dark:text-amber-400">
            {t('items.missingCirculationFields', {
              fields: draftMissing.map((field) => missingFieldLabel(t, field)).join(', '),
            })}
          </p>
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('items.stillIncomplete')}</p>
        </div>
      ) : null}
      {draftRecommended.length > 0 ? (
        <p className="text-sm text-red-700 dark:text-red-400">{t('items.recommendedPrice')}</p>
      ) : null}
      <CopyNoteFields
        publicNote={formData.publicNote}
        internalNote={formData.internalNote}
        onPublicNoteChange={(publicNote) => setFormData({ ...formData, publicNote })}
        onInternalNoteChange={(internalNote) => setFormData({ ...formData, internalNote })}
      />
    </form>
  );
}

