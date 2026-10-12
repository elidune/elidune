import Badge from '@/components/common/Badge';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import { borrowerAvatarInitials, type BorrowerRef } from '@/utils/readerIdentifier';

interface OverdueBorrowerSummaryProps {
  user: BorrowerRef | null | undefined;
  count: number;
  linked: boolean;
}

/** Group header for `GET /loans/overdue`. Contact columns exist only on the full shape. */
export default function OverdueBorrowerSummary({ user, count, linked }: OverdueBorrowerSummaryProps) {
  const initials = borrowerAvatarInitials(user);
  return (
    <>
      <div
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-rose-100 text-sm font-bold text-rose-800 dark:bg-rose-900/50 dark:text-rose-200"
        aria-hidden
      >
        {initials}
      </div>
      <div className="min-w-0 flex-1">
        <ReaderIdentifier
          user={user}
          linked={linked}
          showContact
          className={
            linked
              ? 'font-semibold text-gray-900 dark:text-white hover:text-indigo-600 dark:hover:text-indigo-400'
              : 'font-semibold text-gray-900 dark:text-white'
          }
          onClick={(event) => event.stopPropagation()}
          aside={
            <span className="shrink-0">
              <Badge variant="danger" size="sm">
                {count}
              </Badge>
            </span>
          }
        />
      </div>
    </>
  );
}
