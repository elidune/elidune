import { useTranslation } from 'react-i18next';
import QueryErrorBanner from '@/components/common/QueryErrorBanner';
import { getApiErrorMessage } from '@/utils/apiError';
import { isLoansForbidden } from '@/utils/loanListAccess';

interface LoansAccessNoticeProps {
  error?: unknown;
  denied?: boolean;
  onRetry: () => void;
  fallbackMessage: string;
}

/** One loans-list failure. A 403 is a neutral status, not an alert plus a toast. */
export default function LoansAccessNotice({
  error,
  denied = false,
  onRetry,
  fallbackMessage,
}: LoansAccessNoticeProps) {
  const { t } = useTranslation();
  if (denied || isLoansForbidden(error)) {
    return (
      <p role="status" className="text-sm text-gray-600 dark:text-gray-400">
        {t('loans.loansNotAllowed')}
      </p>
    );
  }
  return (
    <QueryErrorBanner
      message={getApiErrorMessage(error, t) || fallbackMessage}
      onRetry={onRetry}
    />
  );
}
