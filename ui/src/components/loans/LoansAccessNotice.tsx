import { useTranslation } from 'react-i18next';
import QueryErrorBanner from '@/components/common/QueryErrorBanner';
import { getApiErrorMessage } from '@/utils/apiError';
import { isLoansForbidden } from '@/utils/loanListAccess';

interface LoansAccessNoticeProps {
  error?: unknown;
  denied?: boolean;
  /** Overrides the loans sentence. The reader record uses its own wording. */
  message?: string;
  onRetry: () => void;
  fallbackMessage: string;
}

/** One loans-list failure. A 403 is an access sentence, not an empty list or a toast. */
export default function LoansAccessNotice({
  error,
  denied = false,
  message,
  onRetry,
  fallbackMessage,
}: LoansAccessNoticeProps) {
  const { t } = useTranslation();
  if (denied || isLoansForbidden(error)) {
    return (
      <p role="status" className="text-sm text-gray-600 dark:text-gray-400">
        {message ?? t('loans.loansNotAllowed')}
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
