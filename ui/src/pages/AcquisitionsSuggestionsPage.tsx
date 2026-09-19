import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Lightbulb } from 'lucide-react';
import {
  Button,
  Card,
  ListSkeleton,
  Modal,
  Pagination,
  QueryErrorBanner,
  ResponsiveRecordList,
  Table,
} from '@/components/common';
import AcquisitionsSubnav from '@/components/acquisitions/AcquisitionsSubnav';
import SuggestionStatusBadge from '@/components/acquisitions/SuggestionStatusBadge';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { ACQUISITIONS_ORDERS_KEY, PURCHASE_SUGGESTIONS_KEY } from '@/hooks/acquisitions/queryKeys';
import { useSuggestionsQuery } from '@/hooks/acquisitions/useSuggestionsQuery';
import api from '@/services/api';
import { canManageAcquisitions, type PurchaseSuggestion, type PurchaseSuggestionStatus } from '@/types';
import { SUGGESTION_STATUSES } from '@/utils/acquisitionDisplay';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';

const PER_PAGE = 20;

type ReviewAction = 'accept' | 'refuse';

export default function AcquisitionsSuggestionsPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageAcquisitions(user, api.getToken());

  const [status, setStatus] = useState<PurchaseSuggestionStatus | ''>('proposed');
  const [page, setPage] = useState(1);
  const [review, setReview] = useState<{ action: ReviewAction; row: PurchaseSuggestion } | null>(null);
  const [staffNote, setStaffNote] = useState('');
  const [reviewError, setReviewError] = useState<string | null>(null);

  const query = useSuggestionsQuery({
    status: status || undefined,
    page,
    perPage: PER_PAGE,
  });
  const suggestions = query.data?.suggestions ?? [];
  const total = query.data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PER_PAGE));

  const reviewMutation = useMutation({
    mutationFn: async () => {
      if (!review) throw new Error('No suggestion selected');
      const payload = { staffNote: staffNote.trim() || undefined };
      return review.action === 'accept'
        ? api.acceptPurchaseSuggestion(review.row.id, payload)
        : api.refusePurchaseSuggestion(review.row.id, payload);
    },
    onSuccess: (updated) => {
      const action = review?.action;
      setReview(null);
      setStaffNote('');
      setReviewError(null);
      void queryClient.invalidateQueries({ queryKey: PURCHASE_SUGGESTIONS_KEY });
      void queryClient.invalidateQueries({ queryKey: ACQUISITIONS_ORDERS_KEY });
      if (action === 'accept') {
        showToast({ message: t('acquisitions.suggestions.accepted'), variant: 'success' });
        if (updated.purchaseOrderId) {
          navigate(`/acquisitions/orders/${updated.purchaseOrderId}`);
        }
        return;
      }
      showToast({ message: t('acquisitions.suggestions.refused'), variant: 'success' });
    },
    onError: (err) => setReviewError(getApiErrorMessage(err, t)),
  });

  const openReview = (action: ReviewAction, row: PurchaseSuggestion) => {
    setReview({ action, row });
    setStaffNote('');
    setReviewError(null);
  };

  const openOrder = (row: PurchaseSuggestion) => {
    if (row.purchaseOrderId) {
      navigate(`/acquisitions/orders/${row.purchaseOrderId}`);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
          {t('acquisitions.suggestions.title')}
        </h1>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {t('acquisitions.suggestions.subtitle')}
        </p>
      </div>
      <AcquisitionsSubnav />

      <p className="text-sm text-gray-600 dark:text-gray-400">{t('acquisitions.suggestions.staffHelp')}</p>

      <Card className="overflow-hidden">
        <div className="grid gap-3 p-4 md:grid-cols-4">
          <select
            className={formControlClass()}
            value={status}
            onChange={(e) => {
              setStatus(e.target.value as PurchaseSuggestionStatus | '');
              setPage(1);
            }}
            aria-label={t('common.status')}
          >
            <option value="">{t('acquisitions.suggestions.allStatuses')}</option>
            {SUGGESTION_STATUSES.map((s) => (
              <option key={s} value={s}>
                {t(`acquisitions.suggestionStatuses.${s}`)}
              </option>
            ))}
          </select>
        </div>

        {query.isLoading && query.data === undefined ? (
          <ListSkeleton />
        ) : query.isError ? (
          <div className="p-4">
            <QueryErrorBanner
              message={getApiErrorMessage(query.error, t) || t('acquisitions.suggestions.errorLoad')}
              onRetry={() => void query.refetch()}
            />
          </div>
        ) : (
          <>
            <ResponsiveRecordList
              desktop={
                <Table
                  data={suggestions}
                  keyExtractor={(row) => row.id}
                  emptyMessage={t('acquisitions.suggestions.empty')}
                  columns={[
                    { key: 'title', header: t('acquisitions.suggestions.formTitle'), render: (row) => row.title },
                    { key: 'author', header: t('acquisitions.suggestions.formAuthor'), render: (row) => row.author },
                    {
                      key: 'proposedBy',
                      header: t('acquisitions.suggestions.proposedBy'),
                      render: (row) => row.proposedByName || row.proposedBy,
                    },
                    {
                      key: 'status',
                      header: t('common.status'),
                      render: (row) => <SuggestionStatusBadge status={row.status} />,
                    },
                    {
                      key: 'created',
                      header: t('acquisitions.updatedAt'),
                      render: (row) => new Date(row.createdAt).toLocaleDateString(i18n.language),
                    },
                    {
                      key: 'actions',
                      header: t('common.actions'),
                      render: (row) => (
                        <SuggestionActions
                          row={row}
                          canWrite={canWrite}
                          onAccept={() => openReview('accept', row)}
                          onRefuse={() => openReview('refuse', row)}
                          onOpenOrder={() => openOrder(row)}
                        />
                      ),
                    },
                  ]}
                />
              }
              mobile={
                suggestions.length === 0 ? (
                  <p className="px-4 py-8 text-center text-sm text-gray-500">
                    {t('acquisitions.suggestions.empty')}
                  </p>
                ) : (
                  <ul className="divide-y divide-gray-100 dark:divide-gray-800">
                    {suggestions.map((row) => (
                      <li key={row.id} className="flex items-start gap-3 px-4 py-3">
                        <Lightbulb className="mt-0.5 h-5 w-5 shrink-0 text-amber-600" />
                        <div className="min-w-0 flex-1">
                          <p className="font-medium text-gray-900 dark:text-white">{row.title}</p>
                          <p className="text-xs text-gray-500">
                            {row.author}
                            {row.proposedByName ? ` · ${row.proposedByName}` : ''}
                          </p>
                          {row.comment ? (
                            <p className="mt-1 text-xs text-gray-500">{row.comment}</p>
                          ) : null}
                          <div className="mt-2">
                            <SuggestionActions
                              row={row}
                              canWrite={canWrite}
                              onAccept={() => openReview('accept', row)}
                              onRefuse={() => openReview('refuse', row)}
                              onOpenOrder={() => openOrder(row)}
                            />
                          </div>
                        </div>
                        <SuggestionStatusBadge status={row.status} />
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
        isOpen={review !== null}
        onClose={() => {
          if (!reviewMutation.isPending) setReview(null);
        }}
        title={
          review?.action === 'accept'
            ? t('acquisitions.suggestions.acceptTitle')
            : t('acquisitions.suggestions.refuseTitle')
        }
        footer={
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="secondary"
              onClick={() => setReview(null)}
              disabled={reviewMutation.isPending}
            >
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              variant={review?.action === 'refuse' ? 'danger' : 'primary'}
              isLoading={reviewMutation.isPending}
              onClick={() => reviewMutation.mutate()}
            >
              {review?.action === 'accept'
                ? t('acquisitions.suggestions.accept')
                : t('acquisitions.suggestions.refuse')}
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <p className="text-sm text-gray-600 dark:text-gray-400">
            {review?.action === 'accept'
              ? t('acquisitions.suggestions.acceptConfirm')
              : t('acquisitions.suggestions.refuseConfirm')}
          </p>
          {review ? (
            <p className="text-sm font-medium text-gray-900 dark:text-white">
              {review.row.title} — {review.row.author}
            </p>
          ) : null}
          <div>
            <label className={formLabelClass()} htmlFor="suggestion-staff-note">
              {t('acquisitions.suggestions.staffNote')}
            </label>
            <textarea
              id="suggestion-staff-note"
              className={formTextareaClass()}
              rows={3}
              value={staffNote}
              onChange={(e) => setStaffNote(e.target.value)}
            />
          </div>
          {reviewError ? (
            <p className="text-sm text-red-600 dark:text-red-400" role="alert">
              {reviewError}
            </p>
          ) : null}
        </div>
      </Modal>
    </div>
  );
}

function SuggestionActions({
  row,
  canWrite,
  onAccept,
  onRefuse,
  onOpenOrder,
}: {
  row: PurchaseSuggestion;
  canWrite: boolean;
  onAccept: () => void;
  onRefuse: () => void;
  onOpenOrder: () => void;
}) {
  const { t } = useTranslation();

  if (row.status === 'accepted' && row.purchaseOrderId) {
    return (
      <Button type="button" variant="secondary" size="sm" onClick={onOpenOrder}>
        {t('acquisitions.suggestions.openOrder')}
      </Button>
    );
  }

  if (row.status !== 'proposed' || !canWrite) {
    return null;
  }

  return (
    <div className="flex flex-wrap gap-2">
      <Button type="button" size="sm" onClick={onAccept}>
        {t('acquisitions.suggestions.accept')}
      </Button>
      <Button type="button" variant="danger" size="sm" onClick={onRefuse}>
        {t('acquisitions.suggestions.refuse')}
      </Button>
    </div>
  );
}
