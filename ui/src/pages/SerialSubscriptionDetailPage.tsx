import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Newspaper, Plus } from 'lucide-react';
import {
  Button,
  Card,
  ConfirmDialog,
  Input,
  Modal,
  PageHeading,
  QueryErrorBanner,
  ResponsiveRecordList,
  Table,
} from '@/components/common';
import { IssueStatusBadge } from '@/components/serials/IssueStatusBadge';
import ReceiveIssueDialog, { ReceivedSerialCopy } from '@/components/serials/ReceiveIssueDialog';
import SubscriptionFormModal from '@/components/serials/SubscriptionFormModal';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import api from '@/services/api';
import { canManageSerials, type SerialIssue, type SerialIssueStaffStatus } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import {
  barcodeFromNotice,
  canReceiveIssue,
  focusReceiveButton,
  formatIsoDate,
  formatIsoDateTime,
  formatSerialCents,
  latestIssueId,
  nextReceivableIssueId,
} from '@/utils/serialSubscriptionDisplay';

export default function SerialSubscriptionDetailPage() {
  const { id = '' } = useParams();
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { user } = useAuth();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const canWrite = canManageSerials(user, api.getToken());
  const [editing, setEditing] = useState(false);
  const [formError, setFormError] = useState<unknown>(null);
  const [receiving, setReceiving] = useState<SerialIssue | null>(null);
  const [receiveError, setReceiveError] = useState<string | null>(null);
  const [receivedCopy, setReceivedCopy] = useState<{ label: string; barcode: string | null } | null>(null);
  const [focusIssueId, setFocusIssueId] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<{ issue: SerialIssue; status: 'missing' | 'deleted' } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [supplementOpen, setSupplementOpen] = useState(false);
  const [supplementLabel, setSupplementLabel] = useState('');
  const [supplementDate, setSupplementDate] = useState('');
  const [supplementNotes, setSupplementNotes] = useState('');
  const [supplementError, setSupplementError] = useState<string | null>(null);

  const query = useQuery({
    queryKey: ['serial-subscription', id],
    queryFn: () => api.getSerialSubscription(id),
    enabled: Boolean(id),
  });
  const subscription = query.data;
  const issues = useMemo(() => subscription?.issues ?? [], [subscription?.issues]);
  const latestId = subscription?.latestIssueLibraryUseOnly ? latestIssueId(issues) : null;

  useEffect(() => {
    if (!focusIssueId) return;
    const timer = window.setTimeout(() => focusReceiveButton(focusIssueId), 0);
    return () => window.clearTimeout(timer);
  }, [focusIssueId, issues]);

  const refresh = async () => {
    await queryClient.invalidateQueries({ queryKey: ['serial-subscription', id] });
    await queryClient.invalidateQueries({ queryKey: ['serial-subscriptions'] });
  };

  const saveMutation = useMutation({
    mutationFn: (body: import('@/types').SerialSubscriptionWrite) => api.updateSerialSubscription(id, body),
    onSuccess: async () => {
      setEditing(false);
      setFormError(null);
      await refresh();
      showToast({ variant: 'success', message: t('serials.saved') });
    },
    onError: (error: unknown) => setFormError(error),
  });

  const receiveMutation = useMutation({
    mutationFn: async ({ issue, barcode }: { issue: SerialIssue; barcode: string }) => {
      const received = await api.receiveSerialIssue(id, issue.id, barcode ? { barcode } : {});
      let shown = barcode || null;
      if (received.itemId && !shown) {
        try {
          const notice = await api.getItem(received.itemId);
          shown = barcodeFromNotice(notice, received.itemId);
        } catch {
          shown = null;
        }
      }
      return { received, shown };
    },
    onSuccess: async ({ received, shown }, { issue }) => {
      const nextId = nextReceivableIssueId(issues, issue.id);
      setReceiving(null);
      setReceiveError(null);
      if (received.itemId) setReceivedCopy({ label: received.displayLabel, barcode: shown });
      setFocusIssueId(nextId);
      await refresh();
      showToast({ variant: 'success', message: t('serials.received') });
    },
    onError: (error: unknown) => setReceiveError(getApiErrorMessage(error, t)),
  });

  const statusMutation = useMutation({
    mutationFn: ({ issueId, status }: { issueId: string; status: SerialIssueStaffStatus }) =>
      api.setSerialIssueStatus(id, issueId, status),
    onSuccess: async () => {
      setConfirm(null);
      setActionError(null);
      await refresh();
    },
    onError: (error: unknown) => setActionError(getApiErrorMessage(error, t)),
  });

  const lateMutation = useMutation({
    mutationFn: () => api.markSerialIssuesLate(id),
    onSuccess: async (result) => {
      setActionError(null);
      await refresh();
      showToast({ variant: 'success', message: t('serials.markedLate', { count: result.issues.length }) });
    },
    onError: (error: unknown) => setActionError(getApiErrorMessage(error, t)),
  });

  const generateMutation = useMutation({
    mutationFn: () => api.generateSerialIssues(id),
    onSuccess: async (result) => {
      setActionError(null);
      await refresh();
      showToast({ variant: 'success', message: t('serials.generated', { count: result.issues.length }) });
    },
    onError: (error: unknown) => setActionError(getApiErrorMessage(error, t)),
  });

  const supplementMutation = useMutation({
    mutationFn: () =>
      api.createSerialSupplement(id, {
        plannedDate: supplementDate,
        displayLabel: supplementLabel.trim(),
        notes: supplementNotes.trim() || null,
      }),
    onSuccess: async () => {
      setSupplementOpen(false);
      setSupplementLabel('');
      setSupplementDate('');
      setSupplementNotes('');
      setSupplementError(null);
      await refresh();
    },
    onError: (error: unknown) => setSupplementError(getApiErrorMessage(error, t)),
  });

  const askStatus = (issue: SerialIssue, status: SerialIssueStaffStatus) => {
    setActionError(null);
    if (status === 'missing' || status === 'deleted') {
      setConfirm({ issue, status });
      return;
    }
    statusMutation.mutate({ issueId: issue.id, status });
  };

  if (query.isLoading) {
    return <p className="p-4 text-sm text-gray-600 dark:text-gray-300">{t('common.loading')}</p>;
  }
  if (query.isError || !subscription) {
    return (
      <div className={APP_PAGE_CLASS}>
        <QueryErrorBanner message={getApiErrorMessage(query.error, t)} onRetry={() => void query.refetch()} />
      </div>
    );
  }

  const columns = [
    {
      key: 'label',
      header: t('serials.issues.label'),
      render: (issue: SerialIssue) => (
        <IssueLabel issue={issue} latest={issue.id === latestId} />
      ),
    },
    {
      key: 'planned',
      header: t('serials.issues.plannedDate'),
      render: (issue: SerialIssue) => formatIsoDate(issue.plannedDate, i18n.language),
    },
    {
      key: 'status',
      header: t('common.status'),
      render: (issue: SerialIssue) => <IssueStatusBadge status={issue.status} />,
    },
    {
      key: 'received',
      header: t('serials.issues.received'),
      render: (issue: SerialIssue) => formatIsoDateTime(issue.receivedAt, i18n.language),
    },
    {
      key: 'actions',
      header: t('serials.issues.actions'),
      render: (issue: SerialIssue) =>
        canWrite ? (
          <IssueActions
            issue={issue}
            onReceive={() => {
              setReceiveError(null);
              setReceiving(issue);
            }}
            onStatus={(status) => askStatus(issue, status)}
          />
        ) : null,
    },
  ];

  return (
    <div className={`${APP_PAGE_CLASS} gap-4`}>
      <PageHeading
        icon={Newspaper}
        title={subscription.biblioTitle?.trim() || t('serials.detail.untitled')}
        subtitle={t('serials.detail.subtitle')}
        actions={
          <div className="flex flex-wrap gap-2">
            <Button type="button" variant="secondary" leftIcon={<ArrowLeft className="h-4 w-4" />} onClick={() => navigate('/serials')}>
              {t('serials.detail.back')}
            </Button>
            {canWrite ? (
              <Button type="button" onClick={() => { setFormError(null); setEditing(true); }}>
                {t('serials.detail.edit')}
              </Button>
            ) : null}
          </div>
        }
      />

      <Card className="grid gap-2 p-4 text-sm text-gray-800 dark:text-gray-100 sm:grid-cols-2">
        <p>{t('serials.column.issn')}: {subscription.issns.length ? subscription.issns.join(', ') : '—'}</p>
        <p>{t('serials.form.cost')}: {formatSerialCents(subscription.costCents, i18n.language)}</p>
        <p>{t('serials.form.issuePrice')}: {formatSerialCents(subscription.issuePriceCents, i18n.language)}</p>
        <p>
          {t('serials.form.graceDays')}:{' '}
          {subscription.graceDays == null
            ? t('serials.detail.graceDefault', { days: subscription.effectiveGraceDays ?? '—' })
            : subscription.graceDays}
        </p>
      </Card>

      {receivedCopy ? (
        <ReceivedSerialCopy
          label={receivedCopy.label}
          barcode={receivedCopy.barcode}
          onDismiss={() => setReceivedCopy(null)}
        />
      ) : null}
      {actionError ? <p role="alert" className="text-sm text-red-600 dark:text-red-400">{actionError}</p> : null}

      <div className="flex flex-wrap gap-2">
        {canWrite ? (
          <>
            <Button type="button" variant="secondary" onClick={() => lateMutation.mutate()} isLoading={lateMutation.isPending}>
              {t('serials.issues.markLate')}
            </Button>
            <Button type="button" variant="secondary" onClick={() => generateMutation.mutate()} isLoading={generateMutation.isPending}>
              {t('serials.issues.generate')}
            </Button>
            <Button
              type="button"
              variant="secondary"
              leftIcon={<Plus className="h-4 w-4" />}
              onClick={() => {
                setSupplementError(null);
                setSupplementOpen(true);
              }}
            >
              {t('serials.issues.supplement')}
            </Button>
          </>
        ) : null}
      </div>

      <Card padding="none" className="overflow-hidden">
        <ResponsiveRecordList
          desktop={
            <Table
              columns={columns}
              data={issues}
              keyExtractor={(issue) => issue.id}
              emptyMessage={t('serials.issues.empty')}
            />
          }
          mobile={
            <ul className="divide-y divide-gray-200 dark:divide-gray-800">
              {issues.length === 0 ? (
                <li className="px-4 py-6 text-sm text-gray-500 dark:text-gray-400">{t('serials.issues.empty')}</li>
              ) : (
                issues.map((issue) => (
                  <li key={issue.id} className="space-y-2 px-4 py-3">
                    <IssueLabel issue={issue} latest={issue.id === latestId} />
                    <p className="text-sm text-gray-700 dark:text-gray-300">
                      {formatIsoDate(issue.plannedDate, i18n.language)}
                    </p>
                    <IssueStatusBadge status={issue.status} />
                    {canWrite ? (
                      <IssueActions
                        issue={issue}
                        onReceive={() => {
                          setReceiveError(null);
                          setReceiving(issue);
                        }}
                        onStatus={(status) => askStatus(issue, status)}
                      />
                    ) : null}
                  </li>
                ))
              )}
            </ul>
          }
        />
      </Card>

      <p className="text-sm">
        <Link to="/serials" className="text-amber-800 underline-offset-2 hover:underline dark:text-amber-200">
          {t('serials.detail.back')}
        </Link>
      </p>

      {editing ? (
        <SubscriptionFormModal
          key={subscription.updatedAt}
          subscription={subscription}
          isOpen
          isSaving={saveMutation.isPending}
          error={formError}
          onClose={() => setEditing(false)}
          onSubmit={(body) => saveMutation.mutate(body)}
        />
      ) : null}

      {receiving ? (
        <ReceiveIssueDialog
          key={receiving.id}
          isOpen
          label={receiving.displayLabel}
          createItems={subscription.createItems}
          isSaving={receiveMutation.isPending}
          error={receiveError}
          onClose={() => {
            const issueId = receiving.id;
            setReceiving(null);
            window.setTimeout(() => focusReceiveButton(issueId), 50);
          }}
          onSubmit={(barcode) => receiveMutation.mutate({ issue: receiving, barcode })}
        />
      ) : null}

      <ConfirmDialog
        isOpen={confirm != null}
        onClose={() => setConfirm(null)}
        onConfirm={() => {
          if (!confirm) return;
          statusMutation.mutate({ issueId: confirm.issue.id, status: confirm.status });
        }}
        title={confirm?.status === 'deleted' ? t('serials.confirm.deleteTitle') : t('serials.confirm.missingTitle')}
        message={
          confirm?.status === 'deleted'
            ? t('serials.confirm.delete', { label: confirm.issue.displayLabel })
            : t('serials.confirm.missing', { label: confirm?.issue.displayLabel ?? '' })
        }
        confirmVariant="danger"
        isLoading={statusMutation.isPending}
        error={confirm ? actionError : null}
      />

      <Modal
        isOpen={supplementOpen}
        onClose={() => setSupplementOpen(false)}
        title={t('serials.supplement.title')}
        footer={
          <div className="flex flex-wrap justify-end gap-2">
            <Button type="button" variant="secondary" onClick={() => setSupplementOpen(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" form="serial-supplement-form" isLoading={supplementMutation.isPending} disabled={!supplementLabel.trim() || !supplementDate}>
              {t('common.save')}
            </Button>
          </div>
        }
      >
        <form
          id="serial-supplement-form"
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            supplementMutation.mutate();
          }}
        >
          <Input
            id="serial-supplement-label"
            label={t('serials.supplement.label')}
            value={supplementLabel}
            onChange={(event) => setSupplementLabel(event.target.value)}
            required
            autoFocus
          />
          <Input
            id="serial-supplement-date"
            label={t('serials.supplement.date')}
            type="date"
            value={supplementDate}
            onChange={(event) => setSupplementDate(event.target.value)}
            required
          />
          <Input
            id="serial-supplement-notes"
            label={t('serials.supplement.notes')}
            value={supplementNotes}
            onChange={(event) => setSupplementNotes(event.target.value)}
          />
          {supplementError ? <p role="alert" className="text-sm text-red-600 dark:text-red-400">{supplementError}</p> : null}
        </form>
      </Modal>
    </div>
  );
}

function IssueLabel({ issue, latest }: { issue: SerialIssue; latest: boolean }) {
  const { t } = useTranslation();
  return (
    <div className="min-w-0">
      <p className="font-medium text-gray-900 dark:text-white">{issue.displayLabel}</p>
      {issue.supplement ? (
        <p className="text-xs text-gray-600 dark:text-gray-300">{t('serials.issues.supplementBadge')}</p>
      ) : null}
      {latest ? (
        <p className="text-xs font-medium text-amber-900 dark:text-amber-100">{t('serials.issues.latestInLibrary')}</p>
      ) : null}
    </div>
  );
}

function IssueActions({
  issue,
  onReceive,
  onStatus,
}: {
  issue: SerialIssue;
  onReceive: () => void;
  onStatus: (status: SerialIssueStaffStatus) => void;
}) {
  const { t } = useTranslation();
  const fromExpected = issue.status === 'expected' || issue.status === 'late';
  const canClaim = fromExpected || issue.status === 'missing';
  return (
    <div className="flex flex-wrap gap-1">
      {canReceiveIssue(issue.status) ? (
        <Button type="button" size="sm" data-serial-receive={issue.id} onClick={onReceive}>
          {t('serials.issues.receive')}
        </Button>
      ) : null}
      {issue.status === 'expected' ? (
        <Button type="button" size="sm" variant="secondary" onClick={() => onStatus('late')}>
          {t('serials.issueStatus.late')}
        </Button>
      ) : null}
      {fromExpected ? (
        <Button type="button" size="sm" variant="secondary" onClick={() => onStatus('missing')}>
          {t('serials.issues.missing')}
        </Button>
      ) : null}
      {canClaim ? (
        <Button type="button" size="sm" variant="secondary" onClick={() => onStatus('claimed')}>
          {t('serials.issues.claimed')}
        </Button>
      ) : null}
      {fromExpected ? (
        <Button type="button" size="sm" variant="secondary" onClick={() => onStatus('notAvailable')}>
          {t('serials.issues.notPublished')}
        </Button>
      ) : null}
      {fromExpected ? (
        <Button type="button" size="sm" variant="danger" onClick={() => onStatus('deleted')}>
          {t('serials.issues.deleted')}
        </Button>
      ) : null}
    </div>
  );
}
