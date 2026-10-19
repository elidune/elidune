import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Library, Plus } from 'lucide-react';
import { Badge, Button, Card, ConfirmDialog, Input, Modal, QueryErrorBanner, Table } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import { canManageItems, type Lender } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import {
  formatReplacementRateCents,
  lenderNameConflictMessage,
  parseReplacementRateEuros,
  replacementRateInputValue,
} from '@/utils/deposits';
import { formChoiceControlClass } from '@/utils/formControl';

const LENDERS_KEY = ['lenders'] as const;

export default function LendersPage() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageItems(user, api.getToken());
  const [includeArchived, setIncludeArchived] = useState(false);
  const [editing, setEditing] = useState<Lender | null | undefined>(undefined);
  const [archiveTarget, setArchiveTarget] = useState<Lender | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [contact, setContact] = useState('');
  const [notes, setNotes] = useState('');
  const [replacementRate, setReplacementRate] = useState('');

  const query = useQuery({
    queryKey: [...LENDERS_KEY, includeArchived],
    queryFn: () => api.listLenders(includeArchived),
  });

  const openForm = (lender: Lender | null) => {
    setFormError(null);
    setEditing(lender);
    setName(lender?.name ?? '');
    setContact(lender?.contact ?? '');
    setNotes(lender?.notes ?? '');
    setReplacementRate(replacementRateInputValue(lender?.replacementRateCents));
  };

  const saveMutation = useMutation({
    mutationFn: async () => {
      const rate = parseReplacementRateEuros(replacementRate);
      if (!rate.ok) {
        return Promise.reject(rate.reason);
      }
      const body = {
        name: name.trim(),
        contact: contact.trim() || null,
        notes: notes.trim() || null,
        ...(editing && rate.cents == null ? {} : { replacementRateCents: rate.cents }),
      };
      if (editing) return api.updateLender(editing.id, body);
      return api.createLender(body);
    },
    onSuccess: () => {
      setEditing(undefined);
      void queryClient.invalidateQueries({ queryKey: LENDERS_KEY });
      showToast({ variant: 'success', message: t('deposits.lenders.saved') });
    },
    onError: (error: unknown) => {
      if (error === 'invalid') {
        setFormError(t('deposits.lenders.replacementRateInvalid'));
        return;
      }
      if (error === 'notPositive') {
        setFormError(t('deposits.lenders.replacementRateNotPositive'));
        return;
      }
      setFormError(lenderNameConflictMessage(error, t) ?? getApiErrorMessage(error, t));
    },
  });

  const archiveMutation = useMutation({
    mutationFn: (id: string) => api.archiveLender(id),
    onSuccess: () => {
      setArchiveTarget(null);
      void queryClient.invalidateQueries({ queryKey: LENDERS_KEY });
      showToast({ variant: 'success', message: t('deposits.lenders.archived') });
    },
    onError: (error: unknown) => {
      showToast({ variant: 'error', message: getApiErrorMessage(error, t) });
    },
  });

  return (
    <div className={APP_PAGE_CLASS}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex items-start gap-3">
          <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-gray-100 dark:bg-gray-800">
            <Library className="h-6 w-6 text-gray-700 dark:text-gray-200" aria-hidden />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('deposits.lenders.title')}</h1>
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">{t('deposits.lenders.subtitle')}</p>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Link to="/deposits" className="text-sm text-indigo-700 hover:underline dark:text-indigo-300">
            {t('deposits.batches.title')}
          </Link>
          {canWrite ? (
            <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => openForm(null)}>
              {t('deposits.lenders.create')}
            </Button>
          ) : null}
        </div>
      </div>

      <Card className="overflow-hidden">
        <label className="flex items-center gap-2 px-4 py-3 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            className={formChoiceControlClass()}
            checked={includeArchived}
            onChange={(event) => setIncludeArchived(event.target.checked)}
          />
          {t('deposits.lenders.includeArchived')}
        </label>
        {query.isError ? (
          <QueryErrorBanner message={getApiErrorMessage(query.error, t)} onRetry={() => void query.refetch()} />
        ) : null}
        <Table
          columns={[
            {
              key: 'name',
              header: t('common.name'),
              render: (lender: Lender) => (
                <span className="font-medium text-gray-900 dark:text-white">{lender.name}</span>
              ),
            },
            {
              key: 'contact',
              header: t('deposits.lenders.contact'),
              render: (lender: Lender) => lender.contact || '—',
            },
            {
              key: 'replacementRate',
              header: t('deposits.lenders.replacementRate'),
              render: (lender: Lender) =>
                formatReplacementRateCents(lender.replacementRateCents, i18n.language)
                ?? t('deposits.lenders.replacementRateNone'),
            },
            {
              key: 'status',
              header: t('common.status'),
              render: (lender: Lender) =>
                lender.archivedAt ? (
                  <Badge variant="default" size="sm">{t('deposits.lenders.archivedBadge')}</Badge>
                ) : (
                  <Badge variant="success" size="sm">{t('common.active')}</Badge>
                ),
            },
            ...(canWrite
              ? [{
                  key: 'actions',
                  header: '',
                  align: 'right' as const,
                  render: (lender: Lender) => (
                    <div className="flex justify-end gap-2" data-row-action>
                      <Button type="button" size="sm" variant="secondary" onClick={() => openForm(lender)}>
                        {t('common.edit')}
                      </Button>
                      {lender.archivedAt ? null : (
                        <Button type="button" size="sm" variant="danger" onClick={() => setArchiveTarget(lender)}>
                          {t('deposits.lenders.archive')}
                        </Button>
                      )}
                    </div>
                  ),
                }]
              : []),
          ]}
          data={query.data ?? []}
          keyExtractor={(lender) => lender.id}
          isLoading={query.isLoading}
          emptyMessage={t('deposits.lenders.empty')}
        />
      </Card>

      <Modal
        isOpen={editing !== undefined}
        onClose={() => setEditing(undefined)}
        title={editing ? t('deposits.lenders.edit') : t('deposits.lenders.create')}
        footer={
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => setEditing(undefined)}>
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              isLoading={saveMutation.isPending}
              disabled={!name.trim()}
              onClick={() => saveMutation.mutate()}
            >
              {t('common.save')}
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <Input label={t('common.name')} value={name} onChange={(event) => setName(event.target.value)} required autoFocus />
          <Input label={t('deposits.lenders.contact')} value={contact} onChange={(event) => setContact(event.target.value)} />
          <Input
            label={t('deposits.lenders.replacementRate')}
            hint={t('deposits.lenders.replacementRateHint')}
            inputMode="decimal"
            value={replacementRate}
            onChange={(event) => setReplacementRate(event.target.value)}
          />
          <Input label={t('deposits.notes')} value={notes} onChange={(event) => setNotes(event.target.value)} />
          {formError ? <p className="text-sm text-red-600 dark:text-red-400">{formError}</p> : null}
        </div>
      </Modal>

      <ConfirmDialog
        isOpen={archiveTarget !== null}
        onClose={() => setArchiveTarget(null)}
        onConfirm={() => {
          if (archiveTarget) archiveMutation.mutate(archiveTarget.id);
        }}
        title={t('deposits.lenders.archive')}
        message={t('deposits.lenders.archiveConfirm', { name: archiveTarget?.name ?? '' })}
        confirmLabel={t('deposits.lenders.archive')}
        confirmVariant="danger"
        isLoading={archiveMutation.isPending}
      />
    </div>
  );
}
