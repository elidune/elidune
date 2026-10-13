import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import { borrowerAvatarInitials, borrowerShownToAgent, type BorrowerRef } from '@/utils/readerIdentifier';

interface ReturnBorrowerLineProps {
  user: BorrowerRef | null | undefined;
  /** Users read right. The return payload is `UserShortStaff` then, and minimized otherwise. */
  linked: boolean;
}

/** Borrower on a return recap. Staff shape has a name and no email or phone. */
export default function ReturnBorrowerLine({ user, linked }: ReturnBorrowerLineProps) {
  const initials = borrowerAvatarInitials(borrowerShownToAgent(user, linked));
  return (
    <div className="flex min-w-0 items-center gap-3">
      <div
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-indigo-100 text-xs font-bold uppercase text-indigo-800 dark:bg-indigo-900/55 dark:text-indigo-200"
        aria-hidden
      >
        {initials}
      </div>
      <ReaderIdentifier
        user={user}
        linked={linked}
        className="min-w-0 truncate text-base font-semibold text-gray-900 dark:text-white"
      />
    </div>
  );
}
