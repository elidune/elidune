import { BookMarked, ChevronRight, RefreshCw, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { AccountTypeDefinition, PublicType, UserShort } from '@/types';
import { isSubscriptionExpired } from '@/utils/userSubscription';
import {
  LIST_ROW_ICON_BTN_DANGER,
  LIST_ROW_RENEW_BTN,
  LIST_ROW_SUBSCRIPTION_EXPIRED,
} from '@/utils/listRowActionIconClass';
import UserListAvatar from './UserListAvatar';
import UserListProfileBadges from './UserListProfileBadges';

interface UserListCardProps {
  user: UserShort;
  publicTypes: PublicType[];
  accountTypes: AccountTypeDefinition[];
  onOpen: () => void;
  onDelete: () => void;
  onRenew: () => void;
  deleteLabel: string;
  renewLabel: string;
  actionsAriaLabel: string;
}

export default function UserListCard({
  user,
  publicTypes,
  accountTypes,
  onOpen,
  onDelete,
  onRenew,
  deleteLabel,
  renewLabel,
  actionsAriaLabel,
}: UserListCardProps) {
  const { t } = useTranslation();
  const loanCount = user.nbLoans ?? 0;
  const lateLoanCount = user.nbLateLoans || 0;
  const hasLateLoans = lateLoanCount > 0;
  const loanToneClass = hasLateLoans
    ? 'text-red-600 dark:text-red-400'
    : 'text-gray-400 dark:text-gray-500';
  const loanCountClass = hasLateLoans
    ? 'text-red-600 dark:text-red-400'
    : 'text-gray-600 dark:text-gray-300';
  const showRenewSubscription = isSubscriptionExpired(user.expiryAt);

  return (
    <div
      className={[
        'flex items-stretch border-b border-gray-100 transition-colors last:border-b-0 dark:border-gray-800',
        showRenewSubscription ? LIST_ROW_SUBSCRIPTION_EXPIRED : 'hover:bg-gray-50 dark:hover:bg-gray-800/50',
      ].join(' ')}
    >
      <button
        type="button"
        onClick={onOpen}
        className="flex-1 min-w-0 text-left flex items-center gap-3 py-2 pl-3 pr-2"
      >
        <UserListAvatar
          size="lg"
          firstname={user.firstname}
          lastname={user.lastname}
          accountType={user.accountType}
          accountTypes={accountTypes}
        />
        <div className="flex-1 min-w-0 space-y-0.5">
          <p className="font-medium text-gray-900 dark:text-white truncate">
            {user.firstname} {user.lastname}
          </p>
          <UserListProfileBadges user={user} publicTypes={publicTypes} />
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-gray-600 dark:text-gray-300">
            <span
              className="inline-flex items-center gap-1"
              title={
                hasLateLoans ? t('loans.overdueCount', { count: lateLoanCount }) : undefined
              }
            >
              <BookMarked className={['h-3.5 w-3.5 shrink-0', loanToneClass].join(' ')} />
              <span className={loanCountClass}>{loanCount}</span>
            </span>
          </div>
        </div>
        <div className="flex-shrink-0 self-center text-gray-400 dark:text-gray-500 pr-1">
          <ChevronRight className="h-5 w-5" aria-hidden />
        </div>
      </button>
      {showRenewSubscription ? (
        <div className="flex items-center px-2 border-l border-gray-100 dark:border-gray-800 shrink-0 self-stretch">
          <button
            type="button"
            className={LIST_ROW_RENEW_BTN}
            onClick={(e) => {
              e.stopPropagation();
              onRenew();
            }}
          >
            <RefreshCw className="h-4 w-4 shrink-0" aria-hidden />
            <span>{renewLabel}</span>
          </button>
        </div>
      ) : null}
      <div
        className="flex items-center gap-1.5 px-2 border-l border-gray-100 dark:border-gray-800 shrink-0"
        role="group"
        aria-label={actionsAriaLabel}
      >
        <button
          type="button"
          className={LIST_ROW_ICON_BTN_DANGER}
          title={deleteLabel}
          aria-label={deleteLabel}
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
        >
          <Trash2 className="h-4 w-4" aria-hidden />
        </button>
      </div>
    </div>
  );
}
