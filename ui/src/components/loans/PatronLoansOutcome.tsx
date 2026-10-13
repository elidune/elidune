import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

interface PatronLoansOutcomeProps {
  /** Permission denial from GET /users/:id/loans. Not an empty list. */
  denied: boolean;
  itemCount: number;
  emptyMessage: string;
  children: ReactNode;
}

/**
 * A 403 renders an access sentence and no rows.
 * The empty-list sentence is only a successful response with zero items.
 */
export default function PatronLoansOutcome({
  denied,
  itemCount,
  emptyMessage,
  children,
}: PatronLoansOutcomeProps) {
  const { t } = useTranslation();
  if (denied) {
    return (
      <p role="status" className="px-4 py-12 text-center text-sm text-gray-600 dark:text-gray-400">
        {t('loans.loansNotAllowed')}
      </p>
    );
  }
  if (itemCount === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
        {emptyMessage}
      </div>
    );
  }
  return <>{children}</>;
}
