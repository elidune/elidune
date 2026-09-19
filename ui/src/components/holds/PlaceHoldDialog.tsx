import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Modal, Button, Input, Typeahead } from '@/components/common';
import HoldScopeBadge from '@/components/holds/HoldScopeBadge';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { canManageStaffHolds, canViewStaffHolds } from '@/types';
import type { Item, UserShort } from '@/types';
import { formatUserShortName } from '@/utils/userDisplay';
import { formChoiceLabelClass } from '@/utils/formControl';
import { buildCreateHold, type HoldPlacementScope } from '@/utils/holdDisplay';
import { useResetWhenInactive } from '@/hooks/common/useResetWhenInactive';
import { usePickupSiteDraft } from '@/hooks/holds/usePickupSiteDraft';
import PickupSiteSelect from '@/components/holds/PickupSiteSelect';

export interface PlaceHoldDialogProps {
  open: boolean;
  onClose: () => void;
  biblioId: string;
  biblioTitle: string;
  /** When set, staff/patrons may pin this copy instead of any copy. */
  specimen?: Item | null;
  /** Pre-select “this copy” when opening from a specimen row. */
  pinCopyDefault?: boolean;
  /** @deprecated Prefer rights via JWT; kept for call-site compatibility. */
  accountType?: string;
  currentUserId: string;
  onSuccess?: () => void;
}

type TargetMode = 'self' | 'other';

export default function PlaceHoldDialog({
  open,
  onClose,
  biblioId,
  biblioTitle,
  specimen,
  pinCopyDefault = false,
  currentUserId,
  onSuccess,
}: PlaceHoldDialogProps) {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const token = api.getToken();
  const canStaffWrite = canManageStaffHolds(undefined, token);
  const canStaffRead = canViewStaffHolds(undefined, token);
  const canPinCopy = !!specimen?.id;
  const defaultScope: HoldPlacementScope = canPinCopy && pinCopyDefault ? 'copy' : 'title';
  const [scope, setScope] = useState<HoldPlacementScope>(defaultScope);
  const [scopeForOpen, setScopeForOpen] = useState(open);
  if (open !== scopeForOpen) {
    setScopeForOpen(open);
    if (open) setScope(defaultScope);
  }
  const [targetMode, setTargetMode] = useState<TargetMode>('self');
  const [notes, setNotes] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { sources, pickupSiteId, setPickupSiteId, missing: pickupMissing } = usePickupSiteDraft(open);
  const [userSearchDraft, setUserSearchDraft] = useState('');
  const [userSearchResults, setUserSearchResults] = useState<UserShort[]>([]);
  const [isSearchingUsers, setIsSearchingUsers] = useState(false);
  const [selectedUser, setSelectedUser] = useState<UserShort | null>(null);
  const userSearchSeqRef = useRef(0);

  const { data: queue = [], isLoading: queueLoading } = useQuery({
    queryKey: ['biblioHolds', biblioId],
    queryFn: () => api.getBiblioHolds(biblioId),
    enabled: open && canStaffRead && !!biblioId,
    staleTime: 30 * 1000,
  });

  const quotaUserId = canStaffWrite && targetMode === 'other' ? selectedUser?.id : currentUserId;
  const { data: quota } = useQuery({
    queryKey: ['holdsQuota', quotaUserId],
    queryFn: () => api.getHoldQuota(quotaUserId),
    enabled: open && !!quotaUserId,
    staleTime: 15 * 1000,
  });

  useResetWhenInactive(open, () => {
    setNotes('');
    setError(null);
    setTargetMode('self');
    setSelectedUser(null);
    setUserSearchDraft('');
    setUserSearchResults([]);
    setScope(canPinCopy && pinCopyDefault ? 'copy' : 'title');
    setSubmitting(false);
    setPickupSiteId('');
  });

  const userQuery = userSearchDraft.trim();
  const visibleUserResults = userQuery ? userSearchResults : [];
  const visibleUserSearching = userQuery ? isSearchingUsers : false;

  useEffect(() => {
    const query = userSearchDraft.trim();
    if (!query) {
      userSearchSeqRef.current += 1;
      return;
    }

    const timer = window.setTimeout(() => {
      const seq = ++userSearchSeqRef.current;
      setIsSearchingUsers(true);
      void (async () => {
        try {
          const response = await api.getUsers({ name: query, perPage: 10 });
          if (seq !== userSearchSeqRef.current) return;
          setUserSearchResults(response.items);
        } catch {
          if (seq !== userSearchSeqRef.current) return;
          setUserSearchResults([]);
        } finally {
          if (seq === userSearchSeqRef.current) setIsSearchingUsers(false);
        }
      })();
    }, 300);

    return () => window.clearTimeout(timer);
  }, [userSearchDraft]);

  const pinning = scope === 'copy' && canPinCopy;
  const borrowed = specimen?.borrowed === true;
  const queueAhead = queue.length;
  const nextPosition = queueAhead + 1;
  const quotaBlocked = quota != null && quota.remaining <= 0;

  const resolveTargetUserId = (): string | null => {
    if (!canStaffWrite) return currentUserId;
    if (targetMode === 'self') return currentUserId;
    return selectedUser?.id ?? null;
  };

  const handleConfirm = async () => {
    if (submitting) return;
    const uid = resolveTargetUserId();
    if (!uid) {
      setError(t('holds.selectUser'));
      return;
    }
    if (pinning && !specimen?.id) {
      setError(t('holds.selectCopy'));
      return;
    }
    if (pickupMissing) {
      setError(t('holds.pickupSiteRequired'));
      return;
    }
    setError(null);
    setSubmitting(true);
    try {
      await api.createHold(
        buildCreateHold({
          userId: uid,
          biblioId,
          scope: pinning ? 'copy' : 'title',
          itemId: pinning ? specimen?.id : undefined,
          notes,
          pickupSiteId,
        }),
      );
      void queryClient.invalidateQueries({ queryKey: ['biblioHolds', biblioId] });
      void queryClient.invalidateQueries({ queryKey: ['holdsQuota'] });
      showToast({
        variant: 'success',
        message: pinning ? t('holds.createSuccessPinned') : t('holds.createSuccessTitle'),
      });
      onSuccess?.();
      onClose();
    } catch (e: unknown) {
      setError(getApiErrorMessage(e, t) || t('holds.createError'));
    } finally {
      setSubmitting(false);
    }
  };

  const specimenLabel = specimen
    ? specimen.barcode || specimen.callNumber || specimen.id
    : null;

  return (
    <Modal
      isOpen={open}
      onClose={onClose}
      title={t('holds.dialogTitle')}
      size="md"
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose} disabled={submitting}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={() => void handleConfirm()}
            isLoading={submitting}
            disabled={submitting || quotaBlocked || pickupMissing}
          >
            {t('holds.confirmReserve')}
          </Button>
        </div>
      }
    >
      <div className="space-y-4 text-sm text-gray-700 dark:text-gray-300">
        <div>
          <p className="font-medium text-gray-900 dark:text-white">{biblioTitle}</p>
          {canPinCopy && specimenLabel && (
            <p className="text-gray-500 dark:text-gray-400 mt-1">
              {t('items.specimens')}: <span className="font-mono">{specimenLabel}</span>
            </p>
          )}
        </div>

        {canPinCopy && (
          <fieldset className="space-y-2">
            <legend className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
              {t('holds.pickScope')}
            </legend>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name="hold-scope"
                checked={scope === 'title'}
                onChange={() => setScope('title')}
                className="text-indigo-600"
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
                name="hold-scope"
                checked={scope === 'copy'}
                onChange={() => setScope('copy')}
                className="text-indigo-600"
              />
              <span>
                {t('holds.scopePinned')}
                <span className="block text-xs font-normal text-gray-500 dark:text-gray-400">
                  {t('holds.scopePinnedHint')}
                </span>
              </span>
            </label>
          </fieldset>
        )}

        {canStaffRead && queueLoading && (
          <p className="text-gray-500">{t('common.loading')}</p>
        )}

        <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 px-3 py-2 space-y-1">
          {pinning && borrowed && <p>{t('holds.hintBorrowed')}</p>}
          {!pinning && canStaffRead && queueAhead > 0 && (
            <p>{t('holds.hintTitleQueue', { count: queueAhead, position: nextPosition })}</p>
          )}
          {pinning && canStaffRead && queueAhead > 0 && (
            <p>{t('holds.hintQueue', { count: queueAhead, position: nextPosition })}</p>
          )}
          {!pinning && !(canStaffRead && queueAhead > 0) && (
            <p>{t('holds.hintNotifyWhenReadyTitle')}</p>
          )}
          {pinning && !(canStaffRead && queueAhead > 0) && !borrowed && (
            <p>{t('holds.hintNotifyWhenReady')}</p>
          )}
          {quota && quota.remaining > 0 && (
            <p>{t('holds.quotaRemaining', { remaining: quota.remaining, max: quota.maxActiveHolds })}</p>
          )}
          {quota && quota.remaining <= 0 && (
            <p>{t('holds.quotaFull', { active: quota.activeHolds, max: quota.maxActiveHolds })}</p>
          )}
        </div>

        {canStaffRead && queueAhead > 0 && !queueLoading && (
          <div className="space-y-1">
            <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
              {t('holds.biblioQueue')}
            </p>
            <ol className="list-decimal list-inside space-y-1 text-sm text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
              {queue.map((h) => (
                <li key={h.id} className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{formatUserShortName(h.user) || h.userId}</span>
                  <HoldScopeBadge hold={h} />
                </li>
              ))}
            </ol>
          </div>
        )}

        {canStaffWrite && (
          <div className="space-y-2">
            <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
              {t('holds.reserveFor')}
            </p>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name="reserve-target"
                checked={targetMode === 'self'}
                onChange={() => {
                  setTargetMode('self');
                  setSelectedUser(null);
                }}
                className="text-indigo-600"
              />
              {t('holds.forMe')}
            </label>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name="reserve-target"
                checked={targetMode === 'other'}
                onChange={() => setTargetMode('other')}
                className="text-indigo-600"
              />
              {t('holds.forUser')}
            </label>
            {targetMode === 'other' && (
              <div className="pl-6 space-y-2">
                <Typeahead
                  label={t('users.searchPlaceholder')}
                  value={userSearchDraft}
                  onChange={setUserSearchDraft}
                  items={visibleUserResults}
                  getItemId={(u) => u.id}
                  selectedId={selectedUser?.id}
                  onSelect={setSelectedUser}
                  placeholder={t('common.search')}
                  loading={visibleUserSearching}
                  renderItem={(u) => (
                    <>
                      {u.firstname} {u.lastname}
                      <span className="text-gray-500 ml-2 font-mono text-xs">{u.id}</span>
                    </>
                  )}
                />
                {visibleUserSearching && (
                  <p className="text-xs text-gray-500">{t('common.loading')}</p>
                )}
                {selectedUser && (
                  <p className="text-xs text-gray-600 dark:text-gray-400">
                    {t('holds.selectedUser')}: {selectedUser.firstname} {selectedUser.lastname}
                  </p>
                )}
              </div>
            )}
          </div>
        )}

        <PickupSiteSelect
          sources={sources}
          value={pickupSiteId}
          onChange={setPickupSiteId}
          disabled={submitting}
        />

        <Input
          label={t('holds.notesOptional')}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />

        {error && (
          <div className="rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 px-3 py-2 text-red-700 dark:text-red-400">
            {error}
          </div>
        )}
      </div>
    </Modal>
  );
}
