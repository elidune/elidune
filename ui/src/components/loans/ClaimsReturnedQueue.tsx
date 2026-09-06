import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Card, CardHeader, Button, Table, Badge, Pagination, ScrollableListRegion, ResponsiveRecordList, ListSkeleton } from '@/components/common';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import type { CirculationExceptionKind } from '@/hooks/loans/useCirculationExceptionAction';
import type { ClaimsReturnedQueueItem } from '@/types';
import LoanExceptionActions from './LoanExceptionActions';

const PAGE_SIZE = 20;

interface ClaimsReturnedQueueProps {
  disabled?: boolean;
  busyLoanId?: string | null;
  busyKind?: CirculationExceptionKind | null;
  onOpen: (loanId: string, kind: CirculationExceptionKind, label?: string) => void;
}

function ClaimPatronLink({ userId }: { userId: string }) {
  const { data } = useQuery({
    queryKey: ['user', userId],
    queryFn: () => api.getUser(userId),
    staleTime: 5 * 60_000,
  });
  const name = data ? `${data.firstname} ${data.lastname}`.trim() : userId;
  return (
    <Link
      to={`/users/${userId}`}
      className="font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
    >
      {name || userId}
    </Link>
  );
}

export default function ClaimsReturnedQueue({
  disabled = false,
  busyLoanId,
  busyKind,
  onOpen,
}: ClaimsReturnedQueueProps) {
  const { t, i18n } = useTranslation();
  const [page, setPage] = useState(1);

  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ['loans-claims-returned', page],
    queryFn: () => api.getClaimsReturned({ page, perPage: PAGE_SIZE }),
  });

  const items = data?.items ?? [];
  const total = data?.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  const df = (value?: string | null) =>
    value
      ? new Date(value).toLocaleDateString(i18n.language, {
          day: '2-digit',
          month: 'short',
          year: 'numeric',
        })
      : '—';

  const columns = [
    {
      key: 'title',
      header: t('loans.document'),
      render: (row: ClaimsReturnedQueueItem) => (
        <div className="min-w-0">
          <p className="font-medium text-gray-900 dark:text-white">{row.title || t('loans.noTitle')}</p>
          <p className="text-xs font-mono text-gray-500 dark:text-gray-400">{row.barcode ?? '—'}</p>
        </div>
      ),
    },
    {
      key: 'patron',
      header: t('loans.exceptions.patron'),
      render: (row: ClaimsReturnedQueueItem) => <ClaimPatronLink userId={row.userId} />,
    },
    {
      key: 'dates',
      header: t('loans.dueDate'),
      render: (row: ClaimsReturnedQueueItem) => df(row.expiryAt),
    },
    {
      key: 'flagged',
      header: t('loans.exceptions.flaggedAt'),
      render: (row: ClaimsReturnedQueueItem) => df(row.itemUpdatedAt),
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: (row: ClaimsReturnedQueueItem) => (
        <LoanExceptionActions
          loanId={row.loanId}
          loanLabel={row.title || undefined}
          claimedReturned
          disabled={disabled}
          busyKind={busyLoanId === row.loanId ? busyKind : null}
          onOpen={(kind, label) => onOpen(row.loanId, kind, label)}
        />
      ),
    },
  ];

  return (
    <Card padding="none">
      <div className="p-4 sm:p-6">
        <div className="flex items-start justify-between gap-3">
          <CardHeader
            title={t('loans.exceptions.tabClaims')}
            subtitle={t('loans.exceptions.queueHint')}
          />
          {total > 0 ? <Badge variant="warning" size="sm">{total}</Badge> : null}
        </div>
        <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">{t('loans.exceptions.escalateLostHint')}</p>
      </div>
      {isError && (
        <div
          role="alert"
          className="mx-4 sm:mx-6 mb-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 flex flex-col sm:flex-row sm:items-center gap-2"
        >
          <p className="text-sm font-medium text-red-800 dark:text-red-200 flex-1">
            {getApiErrorMessage(error, t) || t('loans.exceptions.queueLoadError')}
          </p>
          <Button type="button" size="sm" variant="secondary" onClick={() => void refetch()}>
            {t('common.retry')}
          </Button>
        </div>
      )}
      <ScrollableListRegion aria-label={t('loans.exceptions.tabClaims')}>
        {isLoading && !items.length ? (
          <ListSkeleton rows={6} />
        ) : (
          <ResponsiveRecordList
            desktop={
              <Table
                columns={columns}
                data={items}
                keyExtractor={(row) => row.loanId}
                isLoading={false}
                emptyMessage={t('loans.exceptions.queueEmpty')}
              />
            }
            mobile={
              items.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                  {t('loans.exceptions.queueEmpty')}
                </div>
              ) : (
                <div className="space-y-3 px-3 sm:px-4 pb-3">
                  {items.map((row) => (
                    <div
                      key={row.loanId}
                      className="rounded-lg border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 p-4 space-y-3"
                    >
                      <div>
                        <p className="font-medium text-gray-900 dark:text-white">{row.title || t('loans.noTitle')}</p>
                        <p className="text-xs font-mono text-gray-500 dark:text-gray-400">{row.barcode ?? '—'}</p>
                      </div>
                      <p className="text-sm text-gray-600 dark:text-gray-300">
                        <span className="text-gray-500">{t('loans.exceptions.patron')}: </span>
                        <ClaimPatronLink userId={row.userId} />
                      </p>
                      <p className="text-xs text-gray-500 dark:text-gray-400">
                        {t('loans.dueDate')}: {df(row.expiryAt)} · {t('loans.exceptions.flaggedAt')}: {df(row.itemUpdatedAt)}
                      </p>
                      {row.notes ? (
                        <p className="text-xs text-gray-500 dark:text-gray-400 whitespace-pre-wrap">{row.notes}</p>
                      ) : null}
                      <LoanExceptionActions
                        loanId={row.loanId}
                        loanLabel={row.title || undefined}
                        claimedReturned
                        disabled={disabled}
                        busyKind={busyLoanId === row.loanId ? busyKind : null}
                        onOpen={(kind, label) => onOpen(row.loanId, kind, label)}
                      />
                    </div>
                  ))}
                </div>
              )
            }
          />
        )}
      </ScrollableListRegion>
      {total > 0 && (
        <div className="p-4 border-t border-gray-200 dark:border-gray-800">
          <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
        </div>
      )}
    </Card>
  );
}
