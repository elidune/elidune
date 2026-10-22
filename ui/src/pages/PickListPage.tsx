import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Check, Printer, SearchX } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ConfirmDialog,
  QueryErrorBanner,
  Table,
} from '@/components/common';
import { useSitesQuery } from '@/hooks/holds/useSitesQuery';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import {
  PICK_LIST_ALL_SITES,
  pickListRequestSiteId,
  readPickListSiteChoice,
  resolvePickListSiteId,
  writePickListSiteChoice,
} from '@/utils/pickList';
import HoldPickCheckInPanel from '@/components/holds/HoldPickCheckInPanel';
import HoldShelfSlip, { HoldShelfPatron } from '@/components/holds/HoldShelfSlip';
import { deskHoldListQueryOptions } from '@/hooks/holds/deskHoldQueryOptions';
import { refreshHoldDeskQueries } from '@/hooks/holds/useRefreshHoldDeskQueries';
import type { HoldPickListRow } from '@/types';
import type { PickCheckInBind } from '@/utils/pickCheckInFlow';
import { useToast } from '@/contexts/ToastContext';

function browserStorage(): Pick<Storage, 'getItem' | 'setItem'> | null {
  try {
    if (typeof localStorage === 'undefined') return null;
    return localStorage;
  } catch {
    return null;
  }
}

export default function PickListPage() {
  const { t, i18n } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const sitesQuery = useSitesQuery();
  const sites = sitesQuery.data ?? [];
  const [siteChoice, setSiteChoice] = useState<string | null>(() =>
    readPickListSiteChoice(browserStorage()),
  );
  const [pending, setPending] = useState<HoldPickListRow | null>(null);
  const [checkInBind, setCheckInBind] = useState<PickCheckInBind | null>(null);

  const afterCheckIn = useCallback(async () => {
    await refreshHoldDeskQueries(queryClient);
  }, [queryClient]);
  const siteId =
    siteChoice == null || siteChoice.trim() === ''
      ? PICK_LIST_ALL_SITES
      : sitesQuery.isFetched
        ? resolvePickListSiteId(sites, siteChoice)
        : null;

  const pickQuery = useQuery({
    queryKey: ['pickList', siteId],
    queryFn: () => api.getHoldPickList(pickListRequestSiteId(siteId ?? '')),
    enabled: siteId !== null,
    ...deskHoldListQueryOptions,
  });

  const notFoundMutation = useMutation({
    mutationFn: (holdId: string) => api.markPickNotFound(holdId),
    onSuccess: async () => {
      setPending(null);
      showToast({ variant: 'success', message: t('pickList.notFoundSuccess') });
      await queryClient.invalidateQueries({ queryKey: ['pickList'] });
    },
    onError: (error: unknown) => {
      showToast({
        variant: 'error',
        message: getApiErrorMessage(error, t) || t('pickList.notFoundError'),
      });
    },
  });

  const rows = pickQuery.data ?? [];
  const formatDue = (iso: string | null) => {
    if (!iso) return t('pickList.noDue');
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return t('pickList.noDue');
    return date.toLocaleString(i18n.language);
  };

  const columns = [
    {
      key: 'barcode',
      header: t('pickList.columnBarcode'),
      render: (row: HoldPickListRow) => (
        <span className="font-mono text-sm">{row.barcode || '—'}</span>
      ),
    },
    {
      key: 'callNumber',
      header: t('pickList.columnCallNumber'),
      render: (row: HoldPickListRow) => (
        <span className="font-mono text-sm">{row.callNumber || '—'}</span>
      ),
    },
    {
      key: 'title',
      header: t('pickList.columnTitle'),
      render: (row: HoldPickListRow) => (
        <span className="font-medium text-gray-900 dark:text-white">
          {row.title?.trim() || t('pickList.noTitle')}
        </span>
      ),
    },
    {
      key: 'patron',
      header: t('pickList.columnPatron'),
      render: (row: HoldPickListRow) => <HoldShelfPatron holdFor={row.holdFor} />,
    },
    {
      key: 'pickup',
      header: t('pickList.columnPickup'),
      render: (row: HoldPickListRow) => row.pickupSiteName?.trim() || '—',
    },
    {
      key: 'pickupDeadline',
      header: t('pickList.columnPickupDeadline'),
      render: (row: HoldPickListRow) => (
        <span className="tabular-nums">{formatDue(row.pickupDeadline)}</span>
      ),
    },
    {
      key: 'due',
      header: t('pickList.columnDue'),
      render: (row: HoldPickListRow) => (
        <div className="flex items-center gap-2">
          <span className="tabular-nums">{formatDue(row.pickDueAt)}</span>
          {row.overdue ? <Badge variant="danger">{t('pickList.overdue')}</Badge> : null}
        </div>
      ),
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      className: 'print:hidden',
      render: (row: HoldPickListRow) => (
        <div className="flex flex-wrap justify-end gap-2 print:hidden">
          <Button
            size="sm"
            variant="primary"
            className="bg-emerald-600 hover:bg-emerald-700 dark:bg-emerald-500"
            leftIcon={<Check className="h-4 w-4" />}
            onClick={() => {
              setCheckInBind({
                holdId: row.holdId,
                itemId: row.itemId,
                barcode: row.barcode,
                title: row.title,
              });
              document.getElementById('pick-list-check-in')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }}
          >
            {t('holds.checkInAction')}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            leftIcon={<SearchX className="h-4 w-4" />}
            isLoading={notFoundMutation.isPending && notFoundMutation.variables === row.holdId}
            onClick={() => setPending(row)}
          >
            {t('pickList.notFound')}
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="space-y-6 print:space-y-3">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between print:hidden">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('pickList.pageTitle')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('pickList.subtitle')}</p>
        </div>
        <Button
          type="button"
          variant="secondary"
          leftIcon={<Printer className="h-4 w-4" />}
          onClick={() => window.print()}
        >
          {t('pickList.print')}
        </Button>
      </div>

      <h1 className="hidden print:block text-xl font-bold">{t('pickList.pageTitle')}</h1>

      <div id="pick-list-check-in">
      <Card padding="none" className="overflow-hidden print:hidden">
        <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800">
          <HoldPickCheckInPanel bind={checkInBind} onSuccess={() => void afterCheckIn()} />
          {checkInBind ? (
            <p className="mt-3 text-xs text-gray-500 dark:text-gray-400">
              <button
                type="button"
                className="underline hover:text-gray-700 dark:hover:text-gray-200"
                onClick={() => setCheckInBind(null)}
              >
                {t('holds.checkInClearBind')}
              </button>
            </p>
          ) : null}
        </div>
      </Card>
      </div>

      <Card padding="none" className="overflow-hidden">
        <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800 space-y-4 print:border-0 print:p-0">
          <div className="print:hidden">
            <CardHeader
              title={t('pickList.pageTitle')}
              subtitle={pickQuery.isSuccess ? t('pickList.count', { count: rows.length }) : undefined}
            />
          </div>
          <div className="max-w-xs print:hidden">
            <label className={formLabelClass()} htmlFor="pick-list-site">
              {t('pickList.siteLabel')}
            </label>
            <select
              id="pick-list-site"
              className={formControlClass({ className: 'w-full' })}
              value={siteId ?? ''}
              disabled={sitesQuery.isLoading || siteId === null}
              onChange={(event) => {
                const next = event.target.value;
                setSiteChoice(next);
                writePickListSiteChoice(browserStorage(), next);
              }}
            >
              <option value={PICK_LIST_ALL_SITES}>{t('pickList.allSites')}</option>
              {sites.map((site) => (
                <option key={site.id} value={site.id}>
                  {site.name?.trim() || site.id}
                </option>
              ))}
            </select>
          </div>
        </div>

        {pickQuery.isError ? (
          <div className="p-4 print:hidden">
            <QueryErrorBanner
              message={getApiErrorMessage(pickQuery.error, t) || t('pickList.loadError')}
              onRetry={() => void pickQuery.refetch()}
            />
          </div>
        ) : (
          <>
            <div className="print:hidden">
              <Table
                columns={columns}
                data={rows}
                keyExtractor={(row) => row.holdId}
                isLoading={pickQuery.isLoading || siteId === null}
                emptyMessage={t('pickList.empty')}
              />
            </div>
            <ul className="hidden print:block space-y-4 p-4">
              {rows.map((row) => (
                <li key={row.holdId}>
                  <HoldShelfSlip
                    title={row.title}
                    barcode={row.barcode}
                    callNumber={row.callNumber}
                    holdFor={row.holdFor}
                    pickupDeadline={row.pickupDeadline}
                  />
                </li>
              ))}
            </ul>
          </>
        )}
      </Card>

      <ConfirmDialog
        isOpen={pending !== null}
        onClose={() => {
          if (!notFoundMutation.isPending) setPending(null);
        }}
        onConfirm={() => {
          if (pending) notFoundMutation.mutate(pending.holdId);
        }}
        title={t('pickList.notFoundTitle')}
        message={t('pickList.notFoundConfirm', {
          title: pending?.title?.trim() || t('pickList.noTitle'),
          barcode: pending?.barcode || '—',
        })}
        confirmLabel={t('pickList.notFound')}
        confirmVariant="danger"
        isLoading={notFoundMutation.isPending}
      />
    </div>
  );
}
