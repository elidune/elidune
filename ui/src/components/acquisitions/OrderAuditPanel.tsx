import { useTranslation } from 'react-i18next';
import { Badge, ListSkeleton, QueryErrorBanner } from '@/components/common';
import { useOrderAuditQuery } from '@/hooks/acquisitions/useOrdersQuery';
import { getApiErrorMessage } from '@/utils/apiError';

export default function OrderAuditPanel({ orderId }: { orderId: string }) {
  const { t, i18n } = useTranslation();
  const query = useOrderAuditQuery(orderId);

  if (query.isLoading && query.data === undefined) {
    return <ListSkeleton rows={4} />;
  }

  if (query.isError) {
    return (
      <QueryErrorBanner
        message={getApiErrorMessage(query.error, t) || t('acquisitions.audit.errorLoad')}
        onRetry={() => void query.refetch()}
      />
    );
  }

  const entries = query.data?.entries ?? [];
  if (entries.length === 0) {
    return <p className="py-6 text-center text-sm text-gray-500">{t('acquisitions.audit.empty')}</p>;
  }

  return (
    <ul className="divide-y divide-gray-100 dark:divide-gray-800">
      {entries.map((entry) => (
        <li key={entry.id} className="py-3">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-mono text-xs text-gray-700 dark:text-gray-300">{entry.eventType}</span>
            <Badge variant={entry.outcome === 'failure' ? 'danger' : 'success'} size="sm">
              {entry.outcome === 'failure'
                ? t('settings.audit.outcomeFailure')
                : t('settings.audit.outcomeSuccess')}
            </Badge>
          </div>
          <p className="mt-1 text-xs text-gray-500">
            {entry.createdAt
              ? new Date(entry.createdAt).toLocaleString(i18n.language)
              : '—'}
            {entry.errorCode ? ` · ${entry.errorCode}` : ''}
          </p>
        </li>
      ))}
    </ul>
  );
}
