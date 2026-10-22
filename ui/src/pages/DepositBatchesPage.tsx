import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus } from 'lucide-react';
import { Badge, Button, Card, Input, Modal, QueryErrorBanner, Table } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import {
  canManageItems,
  type DepositBatch,
  type DepositBatchStatus,
  type Lender,
  type Site,
} from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { formatCatalogDate } from '@/utils/deposits';
import DepositGlobalReturnScan from '@/components/deposits/DepositGlobalReturnScan';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const BATCHES_KEY = ['deposit-batches'] as const;

const STATUSES: Array<DepositBatchStatus | ''> = ['', 'active', 'returning', 'closed'];

function statusVariant(status: DepositBatchStatus): 'success' | 'warning' | 'default' {
  if (status === 'active') return 'success';
  if (status === 'returning') return 'warning';
  return 'default';
}

export function DepositBatchesPanel() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageItems(user, api.getToken());
  const [status, setStatus] = useState<DepositBatchStatus | ''>('');
  const [creating, setCreating] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [lenderId, setLenderId] = useState('');
  const [reference, setReference] = useState('');
  const [siteId, setSiteId] = useState('');
  const [arrivedOn, setArrivedOn] = useState('');
  const [returnDueOn, setReturnDueOn] = useState('');
  const [notes, setNotes] = useState('');

  const batchesQuery = useQuery({
    queryKey: [...BATCHES_KEY, status],
    queryFn: () => api.listDepositBatches(status ? { status } : undefined),
  });
  const lendersQuery = useQuery({
    queryKey: ['lenders', 'names'],
    queryFn: () => api.listLenders(true),
  });
  const sitesQuery = useQuery({
    queryKey: ['sites', 'active'],
    queryFn: () => api.getSites(false),
  });

  const lenderName = (id: string) =>
    lendersQuery.data?.find((lender) => lender.id === id)?.name ?? id;
  const siteName = (id: string) =>
    sitesQuery.data?.find((source) => String(source.id) === id)?.name ?? id;

  const resetForm = () => {
    setLenderId('');
    setReference('');
    setSiteId('');
    setArrivedOn('');
    setReturnDueOn('');
    setNotes('');
    setFormError(null);
  };

  const createMutation = useMutation({
    mutationFn: () =>
      api.createDepositBatch({
        lenderId,
        reference: reference.trim(),
        siteId,
        arrivedOn,
        returnDueOn: returnDueOn.trim() || null,
        notes: notes.trim() || null,
      }),
    onSuccess: (batch) => {
      setCreating(false);
      resetForm();
      void queryClient.invalidateQueries({ queryKey: BATCHES_KEY });
      showToast({ variant: 'success', message: t('deposits.batches.created') });
      navigate(`/deposits/${batch.id}`);
    },
    onError: (error: unknown) => setFormError(getApiErrorMessage(error, t)),
  });

  const activeLenders = (lendersQuery.data ?? []).filter((lender) => !lender.archivedAt);

  return (
    <div className="space-y-4">
      <DepositGlobalReturnScan canWrite={canWrite} />
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-gray-600 dark:text-gray-300">{t('deposits.batches.subtitle')}</p>
        {canWrite ? (
          <Button
            type="button"
            leftIcon={<Plus className="h-4 w-4" />}
            onClick={() => {
              resetForm();
              setCreating(true);
            }}
          >
            {t('deposits.batches.create')}
          </Button>
        ) : null}
      </div>

      <Card className="overflow-hidden">
        <div className="flex flex-col gap-1.5 px-4 py-3 sm:max-w-xs">
          <label htmlFor="deposit-status" className={formLabelClass({ marginBottom: false })}>
            {t('common.status')}
          </label>
          <select
            id="deposit-status"
            className={formControlClass()}
            value={status}
            onChange={(event) => setStatus(event.target.value as DepositBatchStatus | '')}
          >
            {STATUSES.map((value) => (
              <option key={value || 'all'} value={value}>
                {value ? t(`deposits.status.${value}`) : t('deposits.batches.allStatuses')}
              </option>
            ))}
          </select>
        </div>
        {batchesQuery.isError ? (
          <QueryErrorBanner
            message={getApiErrorMessage(batchesQuery.error, t)}
            onRetry={() => void batchesQuery.refetch()}
          />
        ) : null}
        <Table
          columns={[
            {
              key: 'reference',
              header: t('deposits.reference'),
              render: (batch: DepositBatch) => (
                <span className="font-medium text-gray-900 dark:text-white">{batch.reference}</span>
              ),
            },
            {
              key: 'lender',
              header: t('deposits.lender'),
              render: (batch: DepositBatch) => lenderName(batch.lenderId),
            },
            {
              key: 'site',
              header: t('deposits.site'),
              render: (batch: DepositBatch) => siteName(batch.siteId),
            },
            {
              key: 'return',
              header: t('deposits.returnDueOn'),
              render: (batch: DepositBatch) =>
                batch.returnDueOn
                  ? formatCatalogDate(batch.returnDueOn, i18n.language)
                  : t('deposits.returnDueOpen'),
            },
            {
              key: 'status',
              header: t('common.status'),
              render: (batch: DepositBatch) => (
                <Badge variant={statusVariant(batch.status)} size="sm">
                  {t(`deposits.status.${batch.status}`)}
                </Badge>
              ),
            },
          ]}
          data={batchesQuery.data ?? []}
          keyExtractor={(batch) => batch.id}
          onRowClick={(batch) => navigate(`/deposits/${batch.id}`)}
          isLoading={batchesQuery.isLoading}
          emptyMessage={t('deposits.batches.empty')}
        />
      </Card>

      <Modal
        isOpen={creating}
        onClose={() => setCreating(false)}
        title={t('deposits.batches.create')}
        footer={
          <div className="flex justify-end gap-2">
            <Button type="button" variant="ghost" onClick={() => setCreating(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              type="button"
              isLoading={createMutation.isPending}
              disabled={!lenderId || !reference.trim() || !siteId || !arrivedOn}
              onClick={() => createMutation.mutate()}
            >
              {t('common.create')}
            </Button>
          </div>
        }
      >
        <div className="space-y-3">
          <LenderSiteFields
            lenders={activeLenders}
            sites={sitesQuery.data ?? []}
            lenderId={lenderId}
            siteId={siteId}
            onLender={setLenderId}
            onSite={setSiteId}
          />
          <Input label={t('deposits.reference')} value={reference} onChange={(event) => setReference(event.target.value)} required />
          <Input label={t('deposits.arrivedOn')} type="date" value={arrivedOn} onChange={(event) => setArrivedOn(event.target.value)} required />
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
    </div>
  );
}

export default function DepositBatchesPage() {
  return (
    <div className={APP_PAGE_CLASS}>
      <DepositBatchesPanel />
    </div>
  );
}

function LenderSiteFields({
  lenders,
  sites,
  lenderId,
  siteId,
  onLender,
  onSite,
}: {
  lenders: Lender[];
  sites: Site[];
  lenderId: string;
  siteId: string;
  onLender: (id: string) => void;
  onSite: (id: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <div>
        <label htmlFor="deposit-lender" className={formLabelClass()}>{t('deposits.lender')}</label>
        <select id="deposit-lender" className={formControlClass()} value={lenderId} onChange={(event) => onLender(event.target.value)}>
          <option value="">{t('common.select')}</option>
          {lenders.map((lender) => (
            <option key={lender.id} value={lender.id}>{lender.name}</option>
          ))}
        </select>
      </div>
      <div>
        <label htmlFor="deposit-site" className={formLabelClass()}>{t('deposits.site')}</label>
        <select id="deposit-site" className={formControlClass()} value={siteId} onChange={(event) => onSite(event.target.value)}>
          <option value="">{t('common.select')}</option>
          {sites.map((source) => (
            <option key={source.id} value={String(source.id)}>{source.name || source.key || source.id}</option>
          ))}
        </select>
      </div>
    </>
  );
}
