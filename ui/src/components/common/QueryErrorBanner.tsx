import { AlertCircle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import Button from './Button';

interface QueryErrorBannerProps {
  message: string;
  onRetry: () => void;
}

/** Inline error + retry, same pattern as HoldsPage / UsersPage. */
export default function QueryErrorBanner({ message, onRetry }: QueryErrorBannerProps) {
  const { t } = useTranslation();

  return (
    <div
      role="alert"
      className="rounded-lg border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3"
    >
      <div className="flex items-start gap-2 flex-1 min-w-0 text-sm text-red-800 dark:text-red-200">
        <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" aria-hidden />
        <span>{message}</span>
      </div>
      <Button type="button" size="sm" variant="secondary" onClick={onRetry}>
        {t('common.retry')}
      </Button>
    </div>
  );
}
