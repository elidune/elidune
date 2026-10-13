import { useTranslation } from 'react-i18next';
import { publicExpectedReturnLine } from '@/utils/copyLoanStatus';

interface PublicCopyReturnLineProps {
  availability?: string | null;
  expectedReturnDate?: string | null;
  className?: string;
}

/**
 * Expected return on an OPAC or reader copy row.
 * Renders nothing when the date key is absent. Never includes a borrower.
 */
export default function PublicCopyReturnLine({
  availability,
  expectedReturnDate,
  className = 'text-xs text-gray-600 dark:text-gray-300',
}: PublicCopyReturnLineProps) {
  const { t, i18n } = useTranslation();
  const text = publicExpectedReturnLine(t, { availability, expectedReturnDate }, i18n.language);
  if (!text) return null;
  return <p className={className}>{text}</p>;
}
