import { useTranslation } from 'react-i18next';
import type { MyLoan } from '@/types';
import { closureReasonText } from '@/utils/loanClosure';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';

interface ClosedLoanHistoryProps {
  loans: MyLoan[];
}

/** The caller's own closed loans from `GET /me/loans?archived=true`. */
export default function ClosedLoanHistory({ loans }: ClosedLoanHistoryProps) {
  const { t, i18n } = useTranslation();
  return (
    <div className="space-y-3">
      {loans.map((loan) => {
        const reason = closureReasonText(loan, t, i18n.language);
        return (
          <div
            key={loan.id}
            className="flex flex-col sm:flex-row sm:items-center gap-4 p-4 rounded-xl border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50"
          >
            <LoanMediaTypeBadge mediaType={loan.mediaType} size="comfortable" />
            <div className="flex-1 min-w-0">
              <p className="font-medium text-gray-900 dark:text-white truncate">
                {loan.title || t('items.notSpecified')}
              </p>
              <p className="text-sm text-gray-500 dark:text-gray-400 font-mono">{loan.barcode || '-'}</p>
            </div>
            {reason ? (
              <p className="text-sm font-medium text-gray-900 dark:text-white" data-closure-reason={loan.closureReason ?? ''}>
                {reason}
              </p>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
