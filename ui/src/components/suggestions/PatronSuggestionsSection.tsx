import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Lightbulb } from 'lucide-react';
import { Button, Card, CardHeader, Input, ListSkeleton, QueryErrorBanner } from '@/components/common';
import SuggestionStatusBadge from '@/components/acquisitions/SuggestionStatusBadge';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { PURCHASE_SUGGESTIONS_KEY } from '@/hooks/acquisitions/queryKeys';
import { useSuggestionsQuery } from '@/hooks/acquisitions/useSuggestionsQuery';
import api from '@/services/api';
import type { PurchaseSuggestion } from '@/types';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';

const PER_PAGE = 20;

export default function PatronSuggestionsSection() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();

  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [comment, setComment] = useState('');
  const [formError, setFormError] = useState<string | null>(null);

  const query = useSuggestionsQuery({ page: 1, perPage: PER_PAGE });
  const mine = (query.data?.suggestions ?? []).filter((row) => row.proposedBy === user?.id);

  useEffect(() => {
    if (window.location.hash === '#suggestions') {
      document.getElementById('suggestions')?.scrollIntoView({ behavior: 'smooth' });
    }
  }, []);

  const createMutation = useMutation({
    mutationFn: () =>
      api.createPurchaseSuggestion({
        title: title.trim(),
        author: author.trim(),
        comment: comment.trim() || undefined,
      }),
    onSuccess: () => {
      setTitle('');
      setAuthor('');
      setComment('');
      setFormError(null);
      void queryClient.invalidateQueries({ queryKey: PURCHASE_SUGGESTIONS_KEY });
      showToast({ message: t('acquisitions.suggestions.submitted'), variant: 'success' });
    },
    onError: (err) => setFormError(getApiErrorMessage(err, t)),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) {
      setFormError(t('acquisitions.suggestions.titleRequired'));
      return;
    }
    if (!author.trim()) {
      setFormError(t('acquisitions.suggestions.authorRequired'));
      return;
    }
    setFormError(null);
    createMutation.mutate();
  };

  return (
    <div id="suggestions">
    <Card>
      <CardHeader
        title={t('acquisitions.suggestions.patronTitle')}
        subtitle={t('acquisitions.suggestions.patronSubtitle')}
      />

      <form onSubmit={handleSubmit} className="mt-4 space-y-3">
        <Input
          label={t('acquisitions.suggestions.formTitle')}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          maxLength={500}
        />
        <Input
          label={t('acquisitions.suggestions.formAuthor')}
          value={author}
          onChange={(e) => setAuthor(e.target.value)}
          required
          maxLength={300}
        />
        <div>
          <label className={formLabelClass()} htmlFor="suggestion-comment">
            {t('acquisitions.suggestions.formComment')}
          </label>
          <textarea
            id="suggestion-comment"
            className={formTextareaClass()}
            rows={3}
            maxLength={2000}
            value={comment}
            onChange={(e) => setComment(e.target.value)}
          />
        </div>
        {formError ? (
          <p className="text-sm text-red-600 dark:text-red-400" role="alert">
            {formError}
          </p>
        ) : null}
        <Button type="submit" isLoading={createMutation.isPending} leftIcon={<Lightbulb className="h-4 w-4" />}>
          {t('acquisitions.suggestions.submit')}
        </Button>
      </form>

      <div className="mt-6 border-t border-gray-100 pt-4 dark:border-gray-800">
        <h3 className="mb-3 text-sm font-semibold text-gray-900 dark:text-white">
          {t('acquisitions.suggestions.myList')}
        </h3>
        {query.isLoading && query.data === undefined ? (
          <ListSkeleton rows={3} />
        ) : query.isError ? (
          <QueryErrorBanner
            message={getApiErrorMessage(query.error, t) || t('acquisitions.suggestions.errorLoad')}
            onRetry={() => void query.refetch()}
          />
        ) : mine.length === 0 ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('acquisitions.suggestions.emptyMine')}</p>
        ) : (
          <ul className="divide-y divide-gray-100 dark:divide-gray-800">
            {mine.map((row: PurchaseSuggestion) => (
              <li key={row.id} className="flex items-start justify-between gap-3 py-3">
                <div className="min-w-0">
                  <p className="font-medium text-gray-900 dark:text-white">{row.title}</p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">{row.author}</p>
                  {row.comment ? (
                    <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{row.comment}</p>
                  ) : null}
                  {row.staffNote ? (
                    <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                      {t('acquisitions.suggestions.staffNoteLabel')}: {row.staffNote}
                    </p>
                  ) : null}
                  <p className="mt-1 text-xs text-gray-400">
                    {new Date(row.createdAt).toLocaleDateString(i18n.language)}
                  </p>
                </div>
                <SuggestionStatusBadge status={row.status} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
    </div>
  );
}
