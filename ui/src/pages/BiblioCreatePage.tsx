import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useNavigate, Navigate } from 'react-router-dom';
import { ArrowLeft, AlertCircle } from 'lucide-react';
import { Button, Modal } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { canManageItems } from '@/types';
import api from '@/services/api';
import type { DuplicateConfirmationRequired } from '@/types';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import type { AxiosError } from 'axios';
import BiblioEditorForm, {
  type BiblioCatalogDraft,
  type CreateBiblioPayload,
} from '@/components/items/BiblioEditorForm';
import { ItemQuickCreateDraftPanel } from '@/components/items/ItemQuickCreateForm';
import {
  emptyItemQuickCreateValues,
  type ItemQuickCreateValues,
} from '@/components/items/itemQuickCreateValues';
import { getApiErrorMessage } from '@/utils/apiError';
import { navigateBack } from '@/utils/navigateBack';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import { buildCallNumberOptions, validateCallNumber } from '@/utils/callNumber';
import { useGenreShelfCodes } from '@/hooks/useGenresQuery';

function getDuplicateConfirmationRequired(error: unknown): DuplicateConfirmationRequired | null {
  const ax = error as AxiosError<Record<string, unknown>>;
  if (ax?.response?.status !== 409) return null;
  const data = ax.response?.data as Partial<DuplicateConfirmationRequired> | undefined;
  if (!data) return null;
  if (data.code !== 'duplicate_isbn_needs_confirmation') return null;
  if (typeof data.existingId !== 'string') return null;
  if (typeof data.message !== 'string') return null;
  return data as DuplicateConfirmationRequired;
}

export default function BiblioCreatePage() {
  const { user } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const genreShelfCodes = useGenreShelfCodes();

  const [isLoading, setIsLoading] = useState(false);
  const [includeItem, setIncludeItem] = useState(true);
  const [itemDraft, setItemDraft] = useState<ItemQuickCreateValues>(emptyItemQuickCreateValues);
  const [itemError, setItemError] = useState<string | null>(null);
  const [catalogDraft, setCatalogDraft] = useState<BiblioCatalogDraft | null>(null);
  const [confirmReplaceModal, setConfirmReplaceModal] = useState<{
    existingId: string;
    isbn?: string;
    existingTitle?: string | null;
  } | null>(null);
  const [confirmReplaceLoading, setConfirmReplaceLoading] = useState(false);
  const [confirmReplaceError, setConfirmReplaceError] = useState<string | null>(null);
  const lastCreatePayloadRef = useRef<CreateBiblioPayload | null>(null);

  const handleCatalogDraftChange = useCallback((draft: BiblioCatalogDraft) => {
    setCatalogDraft(draft);
  }, []);

  const handleItemDraftChange = useCallback((next: ItemQuickCreateValues) => {
    setItemError(null);
    setItemDraft(next);
  }, []);

  const callNumberSuggestions = useMemo(
    () =>
      buildCallNumberOptions({
        dewey: catalogDraft?.dewey,
        genre: catalogDraft?.genre || null,
        authors: catalogDraft?.authors,
        mediaType: catalogDraft?.mediaType,
        publicationDate: catalogDraft?.publicationDate,
        genreShelfCodes,
      }),
    [catalogDraft, genreShelfCodes],
  );

  const createWithItem = includeItem && itemDraft.barcode.trim() !== '';

  useEffect(() => {
    if (!confirmReplaceModal) return;
    if (confirmReplaceModal.existingTitle !== undefined) return;
    const existingId = confirmReplaceModal.existingId;
    let cancelled = false;

    (async () => {
      try {
        const existing = await api.getBiblio(existingId);
        if (cancelled) return;
        setConfirmReplaceModal((prev) => {
          if (!prev || prev.existingId !== existingId) return prev;
          return { ...prev, existingTitle: existing.title ?? null };
        });
      } catch {
        if (cancelled) return;
        setConfirmReplaceModal((prev) => {
          if (!prev || prev.existingId !== existingId) return prev;
          return { ...prev, existingTitle: null };
        });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [confirmReplaceModal]);

  const goToDetail = (biblioId: string | null | undefined) => {
    queryClient.invalidateQueries({ queryKey: ['biblios'] });
    if (biblioId) {
      navigate(`/biblios/${biblioId}`);
    } else {
      navigate('/biblios');
    }
  };

  const mergeItemIntoPayload = (payload: CreateBiblioPayload): CreateBiblioPayload | null => {
    if (!includeItem) return payload;
    const barcode = itemDraft.barcode.trim();
    if (!barcode) {
      if (itemDraft.callNumber.trim() !== '') {
        setItemError(t('items.specimenBarcodeRequired'));
        return null;
      }
      return payload;
    }
    if (!itemDraft.siteId.trim()) {
      setItemError(t('items.selectSite'));
      return null;
    }
    if (itemDraft.callNumber.trim() !== '' && !validateCallNumber(itemDraft.callNumber)) {
      return null;
    }
    return {
      ...payload,
      items: [
        {
          barcode,
          callNumber: itemDraft.callNumber.trim() || undefined,
          siteId: itemDraft.siteId,
        },
      ],
    };
  };

  const handleSubmitCreate = async (payload: CreateBiblioPayload) => {
    const merged = mergeItemIntoPayload(payload);
    if (!merged) return;
    lastCreatePayloadRef.current = merged;
    setIsLoading(true);
    try {
      const created = await api.createBiblio(merged);
      goToDetail(created.biblio.id ?? null);
    } catch (error) {
      const confirm = getDuplicateConfirmationRequired(error);
      if (confirm) {
        setConfirmReplaceError(null);
        setConfirmReplaceModal({
          existingId: confirm.existingId,
          isbn: merged.isbn || undefined,
          existingTitle: undefined,
        });
      } else {
        console.error('Error creating item:', error);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleConfirmReplaceExisting = async () => {
    if (!confirmReplaceModal || !lastCreatePayloadRef.current) return;
    setConfirmReplaceLoading(true);
    setConfirmReplaceError(null);
    try {
      const created = await api.createBiblio(lastCreatePayloadRef.current, {
        confirmReplaceExistingId: confirmReplaceModal.existingId,
      });
      setConfirmReplaceModal(null);
      goToDetail(created.biblio.id ?? null);
    } catch (err) {
      console.error('Error confirming replace existing item:', err);
      setConfirmReplaceError(getApiErrorMessage(err, t));
    } finally {
      setConfirmReplaceLoading(false);
    }
  };

  const handleCreateNewDuplicateIsbn = async () => {
    if (!confirmReplaceModal || !lastCreatePayloadRef.current) return;
    setConfirmReplaceLoading(true);
    setConfirmReplaceError(null);
    try {
      const created = await api.createBiblio(lastCreatePayloadRef.current, {
        allowDuplicateIsbn: true,
      });
      setConfirmReplaceModal(null);
      goToDetail(created.biblio.id ?? null);
    } catch (err) {
      console.error('Error creating item with duplicate ISBN:', err);
      setConfirmReplaceError(getApiErrorMessage(err, t));
    } finally {
      setConfirmReplaceLoading(false);
    }
  };

  if (!canManageItems(user, api.getToken())) {
    return <Navigate to="/biblios" replace />;
  }

  return (
    <div className="-mx-4 -mt-4 lg:-mx-6 lg:-mt-6 pb-12">
      <div className="sticky top-0 z-20 px-4 py-4 lg:px-6 mb-6 bg-gray-50 dark:bg-gray-950 border-b border-gray-200 dark:border-gray-800">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start sm:items-center gap-3 min-w-0">
            <button
              type="button"
              onClick={() => navigateBack(navigate, '/biblios')}
              className="p-2 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-800 shrink-0"
              aria-label={t('common.back')}
            >
              <ArrowLeft className="h-5 w-5" />
            </button>
            <div className="min-w-0">
              <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white truncate">
                {t('items.add')}
              </h1>
            </div>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2 shrink-0">
            <Button variant="secondary" type="button" onClick={() => navigateBack(navigate, '/biblios')}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" form="biblio-create-form" isLoading={isLoading}>
              {createWithItem ? t('items.createNoticeAndCopy') : t('items.createNotice')}
            </Button>
          </div>
        </div>
      </div>

      <div className="space-y-6 px-4 lg:px-6">
        <div className="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 p-5 sm:p-6 lg:p-8 shadow-sm">
          <BiblioEditorForm
            mode="create"
            formId="biblio-create-form"
            onSubmitCreate={handleSubmitCreate}
            onCatalogDraftChange={handleCatalogDraftChange}
          />
        </div>

        <div className="rounded-xl border border-amber-300/80 dark:border-amber-700/50 bg-amber-50/80 dark:bg-amber-950/25 p-4 sm:p-5 shadow-sm space-y-3">
          <label className="flex items-start gap-2.5 cursor-pointer">
            <input
              type="checkbox"
              className="mt-0.5 h-4 w-4 rounded border-gray-300 text-amber-600 focus:ring-amber-500"
              checked={includeItem}
              onChange={(e) => {
                setIncludeItem(e.target.checked);
                setItemError(null);
              }}
            />
            <span className="min-w-0">
              <span className="block text-sm font-semibold text-amber-950 dark:text-amber-100">
                {t('items.addCopyNow')}
              </span>
              <span className="block text-xs text-amber-900/80 dark:text-amber-200/80">
                {t('items.addCopyNowHint')}
              </span>
            </span>
          </label>
          {includeItem ? (
            <ItemQuickCreateDraftPanel
              values={itemDraft}
              onChange={handleItemDraftChange}
              callNumberSuggestions={callNumberSuggestions}
              error={itemError}
              onClearError={() => setItemError(null)}
            />
          ) : null}
        </div>
      </div>

      <Modal
        isOpen={!!confirmReplaceModal}
        onClose={() => {
          if (confirmReplaceLoading) return;
          setConfirmReplaceModal(null);
          setConfirmReplaceError(null);
        }}
        title={t('importMarc.duplicateIsbnTitle')}
        size="lg"
        footer={
          confirmReplaceModal ? (
            <div className="flex justify-end gap-2">
              <Button
                variant="secondary"
                onClick={() => {
                  if (confirmReplaceLoading) return;
                  setConfirmReplaceModal(null);
                  setConfirmReplaceError(null);
                }}
              >
                {t('common.cancel')}
              </Button>
              <Button
                variant="secondary"
                onClick={handleCreateNewDuplicateIsbn}
                isLoading={confirmReplaceLoading}
              >
                {t('importMarc.duplicateIsbnCreateNew')}
              </Button>
              <Button onClick={handleConfirmReplaceExisting} isLoading={confirmReplaceLoading}>
                {t('importMarc.duplicateIsbnReplace')}
              </Button>
            </div>
          ) : undefined
        }
      >
        {confirmReplaceModal && (
          <div className="space-y-4">
            <p className="text-sm text-gray-700 dark:text-gray-300">
              {t('importMarc.duplicateIsbnPrompt', {
                isbn: confirmReplaceModal.isbn ? formatIsbnDisplay(confirmReplaceModal.isbn) : '-',
                title: confirmReplaceModal.existingTitle || t('items.notSpecified'),
              })}
            </p>
            {confirmReplaceError && (
              <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
                <AlertCircle className="h-4 w-4 flex-shrink-0" />
                {confirmReplaceError}
              </div>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
