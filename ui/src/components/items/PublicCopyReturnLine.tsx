import { useTranslation } from 'react-i18next';
import { publicOnLoanLabel } from '@/utils/copyLoanStatus';

interface PublicCopyReturnLineProps {
  availability?: string | null;
  expectedReturnDate?: string | null;
  className?: string;
}

/**
 * On-loan line for an OPAC or reader copy row.
 * A missing date stays "On loan" with no timestamp. Never includes a borrower or `dueDate`.
 */
export default function PublicCopyReturnLine({
  availability,
  expectedReturnDate,
  className = 'text-xs text-gray-600 dark:text-gray-300',
}: PublicCopyReturnLineProps) {
  const { t, i18n } = useTranslation();
  const text = publicOnLoanLabel(t, { availability, expectedReturnDate }, i18n.language);
  if (!text) return null;
  return <p className={className}>{text}</p>;
}
