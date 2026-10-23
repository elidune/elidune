import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus } from 'lucide-react';
import { Badge, Button, Card, ConfirmDialog, Input, Modal, QueryErrorBanner, ResponsiveRecordList, Table, ListResultCountBar } from '@/components/common';
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

export function LendersPanel() {
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

  const lenders = query.data ?? [];
  const lenderColumns = [
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
  ];

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-gray-600 dark:text-gray-300">{t('deposits.lenders.subtitle')}</p>
        {canWrite ? (
          <Button type="button" leftIcon={<Plus className="h-4 w-4" />} onClick={() => openForm(null)}>
            {t('deposits.lenders.create')}
          </Button>
        ) : null}
      </div>

      <Card padding="none" className="overflow-hidden">
        <label className="flex items-center gap-2 px-4 py-3 text-sm text-gray-700 dark:text-gray-300 border-b border-gray-200 dark:border-gray-800">
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
        {!query.isLoading || lenders.length > 0 ? (
          <ListResultCountBar>{t('common.listCount', { count: lenders.length })}</ListResultCountBar>
        ) : null}
        <ResponsiveRecordList
          desktop={
            <Table
              columns={lenderColumns}
              data={lenders}
              keyExtractor={(lender) => lender.id}
              isLoading={query.isLoading}
              emptyMessage={t('deposits.lenders.empty')}
            />
          }
          mobile={
            query.isLoading && lenders.length === 0 ? (
              <Table
                columns={lenderColumns}
                data={[]}
                keyExtractor={(lender) => lender.id}
                isLoading
                emptyMessage={t('deposits.lenders.empty')}
              />
            ) : lenders.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                {t('deposits.lenders.empty')}
              </div>
            ) : (
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                {lenders.map((lender) => (
                  <div
                    key={lender.id}
                    className="border-b border-gray-100 dark:border-gray-800 last:border-b-0 p-4 space-y-2"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <p className="font-medium text-gray-900 dark:text-white">{lender.name}</p>
                      {lender.archivedAt ? (
                        <Badge variant="default" size="sm">{t('deposits.lenders.archivedBadge')}</Badge>
                      ) : (
                        <Badge variant="success" size="sm">{t('common.active')}</Badge>
                      )}
                    </div>
                    <p className="text-sm text-gray-600 dark:text-gray-300">
                      {t('deposits.lenders.contact')}: {lender.contact || '—'}
                    </p>
                    <p className="text-sm text-gray-600 dark:text-gray-300">
                      {t('deposits.lenders.replacementRate')}:{' '}
                      {formatReplacementRateCents(lender.replacementRateCents, i18n.language)
                        ?? t('deposits.lenders.replacementRateNone')}
                    </p>
                    {canWrite ? (
                      <div className="flex flex-wrap gap-2 pt-1">
                        <Button type="button" size="sm" variant="secondary" className="min-h-[2.75rem]" onClick={() => openForm(lender)}>
                          {t('common.edit')}
                        </Button>
                        {!lender.archivedAt ? (
                          <Button type="button" size="sm" variant="danger" className="min-h-[2.75rem]" onClick={() => setArchiveTarget(lender)}>
                            {t('deposits.lenders.archive')}
                          </Button>
                        ) : null}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            )
          }
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

export default function LendersPage() {
  return (
    <div className={APP_PAGE_CLASS}>
      <LendersPanel />
    </div>
  );
}
