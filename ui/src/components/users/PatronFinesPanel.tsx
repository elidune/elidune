import { useTranslation } from 'react-i18next';
import Badge from '@/components/common/Badge';
import QueryErrorBanner from '@/components/common/QueryErrorBanner';
import Table from '@/components/common/Table';
import type { Fine, FineStatus } from '@/types';
import { fineNote, fineReasonLabel, fineStatusLabel, formatEuroAmount } from '@/utils/patronFines';

interface PatronFinesPanelProps {
  fines: Fine[];
  totalUnpaid?: string | number | null;
  isLoading: boolean;
  errorMessage: string | null;
  onRetry: () => void;
}

function statusVariant(status: FineStatus): 'default' | 'success' | 'warning' | 'danger' | 'info' {
  switch (status) {
    case 'pending':
      return 'danger';
    case 'partial':
      return 'warning';
    case 'paid':
      return 'success';
    case 'refunded':
      return 'info';
    case 'waived':
    case 'cancelled':
      return 'default';
  }
}

/** Fines on the staff reader record. Amounts come from `GET /users/{id}/fines`. */
export default function PatronFinesPanel({
  fines,
  totalUnpaid,
  isLoading,
  errorMessage,
  onRetry,
}: PatronFinesPanelProps) {
  const { t, i18n } = useTranslation();
  const locale = i18n.language;

  if (errorMessage) {
    return <QueryErrorBanner message={errorMessage} onRetry={onRetry} />;
  }

  return (
    <div className="space-y-3">
      {!isLoading && (
        <p className="px-4 pt-4 text-sm font-medium text-gray-900 dark:text-white" data-fines-total>
          {t('users.fines.totalUnpaid', { amount: formatEuroAmount(totalUnpaid, locale) })}
        </p>
      )}
      <Table
        columns={[
          {
            key: 'reason',
            header: t('users.fines.columns.reason'),
            render: (fine) => {
              const note = fineNote(fine);
              return (
                <div data-fine-reason={fine.chargeType ?? 'unknown'}>
                  <span>{fineReasonLabel(fine.chargeType, t)}</span>
                  {note ? (
                    <span className="block text-xs text-gray-500 dark:text-gray-400">{note}</span>
                  ) : null}
                </div>
              );
            },
          },
          {
            key: 'amount',
            header: t('users.fines.columns.amount'),
            align: 'right',
            render: (fine) => (
              <span data-fine-amount={fine.id}>{formatEuroAmount(fine.amount, locale)}</span>
            ),
          },
          {
            key: 'paid',
            header: t('users.fines.columns.paid'),
            align: 'right',
            render: (fine) => formatEuroAmount(fine.paidAmount, locale),
          },
          {
            key: 'refunded',
            header: t('users.fines.columns.refunded'),
            align: 'right',
            render: (fine) => formatEuroAmount(fine.refundedAmount, locale),
          },
          {
            key: 'status',
            header: t('users.fines.columns.status'),
            render: (fine) => (
              <Badge variant={statusVariant(fine.status)} size="sm">
                <span data-fine-status={fine.status}>{fineStatusLabel(fine.status, t)}</span>
              </Badge>
            ),
          },
        ]}
        data={fines}
        keyExtractor={(fine) => fine.id}
        isLoading={isLoading}
        emptyMessage={t('users.fines.empty')}
      />
    </div>
  );
}
