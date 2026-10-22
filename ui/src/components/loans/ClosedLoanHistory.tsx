import { useTranslation } from 'react-i18next';
import type { MyLoan } from '@/types';
import { closureReasonText } from '@/utils/loanClosure';
import LoanDocumentRow from '@/components/loans/LoanDocumentRow';

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
            <div className="flex-1 min-w-0">
              <LoanDocumentRow
                mediaType={loan.mediaType}
                size="comfortable"
                title={
                  <p className="font-medium text-gray-900 dark:text-white truncate">
                    {loan.title || t('items.notSpecified')}
                  </p>
                }
                details={
                  <p className="text-sm text-gray-500 dark:text-gray-400 font-mono">{loan.barcode || '-'}</p>
                }
              />
            </div>
            {reason ? (
              <p className="text-sm font-medium text-gray-900 dark:text-white shrink-0" data-closure-reason={loan.closureReason ?? ''}>
                {reason}
              </p>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
