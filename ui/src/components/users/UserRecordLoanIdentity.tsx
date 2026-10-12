import { useTranslation } from 'react-i18next';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import type { LoanListItem } from '@/types';
import { isMinimizedBorrower } from '@/utils/readerIdentifier';

interface UserRecordLoanIdentityProps {
  loan: Pick<LoanListItem, 'biblio' | 'user'>;
  /** Users read right. A reader looking at their own loans does not get a record link. */
  linked: boolean;
  /** Modal shows either shape. The loan table shows an identifier only for the minimized shape. */
  placement: 'row' | 'detail';
}

/**
 * Borrower on `GET /users/:id/loans`.
 * Staff rows carry a name (`UserShortStaff`, no email or phone).
 * A reader's own loans carry `{ id, cardNumber, initials }`.
 */
export default function UserRecordLoanIdentity({ loan, linked, placement }: UserRecordLoanIdentityProps) {
  const { t } = useTranslation();
  const user = loan.user;
  if (!user) return null;
  if (placement === 'row' && !isMinimizedBorrower(user)) return null;
  const identifier = (
    <ReaderIdentifier
      user={user}
      linked={linked}
      className="font-medium text-gray-900 dark:text-white"
    />
  );
  if (placement === 'row') {
    return <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">{identifier}</p>;
  }
  return (
    <div>
      <p className="text-xs text-gray-500 dark:text-gray-400">{t('users.reader')}</p>
      <div className="mt-1">{identifier}</div>
    </div>
  );
}
