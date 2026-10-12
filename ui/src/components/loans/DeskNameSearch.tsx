import { Search } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import Typeahead from '@/components/common/Typeahead';
import PatronOptionLabel from '@/components/users/PatronOptionLabel';
import type { UserShort } from '@/types';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import { deskNameSearchVisible } from '@/utils/deskPatron';

interface DeskNameSearchProps {
  canReadUsers: boolean;
  value: string;
  onChange: (value: string) => void;
  items: UserShort[];
  onSelect: (user: UserShort) => void;
  loading: boolean;
  disabled: boolean;
  error: string | null;
  selectError: string | null;
  onRetry: () => void;
}

/** Name search calls `GET /users` and is omitted without users read, which would be 403. */
export default function DeskNameSearch({
  canReadUsers,
  value,
  onChange,
  items,
  onSelect,
  loading,
  disabled,
  error,
  selectError,
  onRetry,
}: DeskNameSearchProps) {
  const { t } = useTranslation();
  if (!deskNameSearchVisible(canReadUsers)) return null;
  return (
    <div>
      <Typeahead
        label={t('loans.searchByName')}
        value={value}
        onChange={onChange}
        items={items}
        getItemId={(user) => user.id}
        onSelect={onSelect}
        placeholder={t('loans.searchUserPlaceholder')}
        leftIcon={<Search className="h-4 w-4" />}
        loading={loading}
        disabled={disabled}
        variant="dropdown"
        renderItem={(user) => (
          <span className="flex items-center gap-3">
            <span className="flex-shrink-0 h-10 w-10 rounded-full bg-amber-100 dark:bg-amber-900/50 flex items-center justify-center">
              <span className="text-sm font-medium text-amber-700 dark:text-amber-400">
                {user.firstname?.[0] || '?'}
                {user.lastname?.[0] || ''}
              </span>
            </span>
            <span className="flex-1 min-w-0">
              <span className="block">
                <PatronOptionLabel user={user} />
              </span>
              {user.accountType ? (
                <span className="block text-sm text-gray-500 dark:text-gray-400">
                  {localizedAccountTypeLabel(t, user.accountType)}
                </span>
              ) : null}
            </span>
          </span>
        )}
      />
      {(error || selectError) && (
        <div
          role="alert"
          className="mt-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 flex flex-col sm:flex-row sm:items-center gap-2"
        >
          <p className="text-sm font-medium text-red-800 dark:text-red-200 flex-1">{selectError || error}</p>
          {error && (
            <Button type="button" size="sm" variant="secondary" onClick={onRetry}>
              {t('common.retry')}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
