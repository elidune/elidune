import { BookMarked, RefreshCw, Trash2 } from 'lucide-react';
import type { TFunction } from 'i18next';
import type { AccountTypeDefinition, PublicType, UserShort } from '@/types';
import { isSubscriptionExpired } from '@/utils/userSubscription';
import {
  LIST_ROW_DEFAULT,
  LIST_ROW_ICON_BTN_DANGER,
  LIST_ROW_RENEW_BTN,
  LIST_ROW_SUBSCRIPTION_EXPIRED,
} from '@/utils/listRowActionIconClass';
import UserListAvatar from './UserListAvatar';
import UserListProfileBadges from './UserListProfileBadges';

const RIGHT_GRID =
  'grid shrink-0 grid-cols-[minmax(0,max-content)_minmax(0,max-content)_5.75rem] items-center gap-x-8';

function rowFromActionTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return Boolean(target.closest('button, a, input, select, textarea, [data-row-action]'));
}

type UsersDesktopListProps = {
  users: UserShort[];
  accountTypes: AccountTypeDefinition[];
  publicTypes: PublicType[];
  emptyMessage: string;
  t: TFunction;
  onRowClick: (user: UserShort) => void;
  onDelete: (user: UserShort) => void;
  onRenew: (user: UserShort) => void;
};

export default function UsersDesktopList({
  users,
  accountTypes,
  publicTypes,
  emptyMessage,
  t,
  onRowClick,
  onDelete,
  onRenew,
}: UsersDesktopListProps) {
  if (users.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400">
        <p>{emptyMessage}</p>
      </div>
    );
  }

  return (
    <div className="min-w-0 divide-y divide-gray-100 dark:divide-gray-800">
        {users.map((user) => {
          const showRenewSubscription = isSubscriptionExpired(user.expiryAt);
          const loanCount = user.nbLoans ?? 0;
          const lateLoanCount = user.nbLateLoans || 0;
          const hasLateLoans = lateLoanCount > 0;
          const loanToneClass = hasLateLoans
            ? 'text-red-600 dark:text-red-400'
            : 'text-gray-400 dark:text-gray-500';
          const loanCountClass = hasLateLoans
            ? 'text-red-600 dark:text-red-400'
            : 'text-gray-900 dark:text-gray-100';
          return (
            <div
              key={user.id}
              role="button"
              tabIndex={0}
              onClick={(event) => {
                if (rowFromActionTarget(event.target)) return;
                onRowClick(user);
              }}
              onKeyDown={(event) => {
                if (event.target !== event.currentTarget) return;
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onRowClick(user);
                }
              }}
              className={[
                'flex cursor-pointer items-center gap-3 px-3 py-2 text-sm text-gray-900 transition-colors dark:text-gray-100',
                showRenewSubscription ? LIST_ROW_SUBSCRIPTION_EXPIRED : LIST_ROW_DEFAULT,
              ].join(' ')}
            >
              <div className="flex min-w-0 flex-1 items-center gap-3">
                <UserListAvatar
                  firstname={user.firstname}
                  lastname={user.lastname}
                  accountType={user.accountType}
                  accountTypes={accountTypes}
                />
                <div className="min-w-0 flex-1 space-y-0.5">
                  <p
                    className="truncate font-medium"
                    title={`${user.firstname ?? ''} ${user.lastname ?? ''}`.trim()}
                  >
                    {user.firstname} {user.lastname}
                  </p>
                  <UserListProfileBadges user={user} publicTypes={publicTypes} />
                </div>
              </div>

              <div className={RIGHT_GRID}>
                <div
                  className="flex items-center justify-end gap-1.5"
                  data-row-action={showRenewSubscription ? true : undefined}
                  onClick={showRenewSubscription ? (e) => e.stopPropagation() : undefined}
                  onKeyDown={showRenewSubscription ? (e) => e.stopPropagation() : undefined}
                >
                  {showRenewSubscription ? (
                    <button
                      type="button"
                      className={LIST_ROW_RENEW_BTN}
                      onClick={() => onRenew(user)}
                    >
                      <RefreshCw className="h-4 w-4 shrink-0" aria-hidden />
                      <span>{t('users.renewSubscriptionListAction')}</span>
                    </button>
                  ) : null}
                </div>
                <div className="flex justify-end">
                  <div
                    className="inline-flex items-center gap-2 whitespace-nowrap"
                    title={
                      hasLateLoans
                        ? t('loans.overdueCount', { count: lateLoanCount })
                        : undefined
                    }
                  >
                    <BookMarked className={['h-4 w-4 shrink-0', loanToneClass].join(' ')} />
                    <span className={loanCountClass}>{loanCount}</span>
                  </div>
                </div>
                <div
                  className="flex justify-end gap-1.5"
                  data-row-action
                  role="group"
                  aria-label={t('common.actions')}
                  onClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => e.stopPropagation()}
                >
                  <button
                    type="button"
                    className={LIST_ROW_ICON_BTN_DANGER}
                    title={t('common.delete')}
                    aria-label={t('common.delete')}
                    onClick={() => onDelete(user)}
                  >
                    <Trash2 className="h-4 w-4" aria-hidden />
                  </button>
                </div>
              </div>
            </div>
          );
        })}
    </div>
  );
}
