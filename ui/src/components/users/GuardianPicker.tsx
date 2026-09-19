import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Search } from 'lucide-react';
import { Typeahead } from '@/components/common';
import api from '@/services/api';
import type { PublicType, UserShort } from '@/types';
import { isEligibleGuardianCandidate } from '@/utils/legalGuardian';
import { formatUserShortName } from '@/utils/userDisplay';

export interface GuardianPickerProps {
  value: UserShort | null;
  onChange: (user: UserShort) => void;
  publicTypes: PublicType[];
  excludeUserId?: string;
  error?: string;
  /** Link the selected guardian to their patron file (edit / detail only). */
  showFileLink?: boolean;
}

export default function GuardianPicker({
  value,
  onChange,
  publicTypes,
  excludeUserId,
  error,
  showFileLink = false,
}: GuardianPickerProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState('');
  const [results, setResults] = useState<UserShort[]>([]);
  const [searching, setSearching] = useState(false);
  const seqRef = useRef(0);

  const query = draft.trim();
  const candidates = (query ? results : []).filter(
    (u) => u.id !== value?.id && isEligibleGuardianCandidate(u, publicTypes, excludeUserId)
  );
  const visibleSearching = query ? searching : false;

  useEffect(() => {
    const q = draft.trim();
    if (!q) {
      seqRef.current += 1;
      return;
    }

    const timer = window.setTimeout(() => {
      const seq = ++seqRef.current;
      setSearching(true);
      void (async () => {
        try {
          const res = await api.getUsers({ name: q, perPage: 10 });
          if (seq !== seqRef.current) return;
          setResults(res.items);
        } catch {
          if (seq !== seqRef.current) return;
          setResults([]);
        } finally {
          if (seq === seqRef.current) setSearching(false);
        }
      })();
    }, 300);

    return () => window.clearTimeout(timer);
  }, [draft]);

  const displayName = value ? formatUserShortName(value) : '';

  return (
    <div>
      <p className="text-sm font-medium text-gray-700 dark:text-gray-300">
        {t('users.legalGuardian')}
        <span className="text-red-600 dark:text-red-400 ml-0.5 font-medium" aria-hidden="true">
          *
        </span>
      </p>
      <p className="mt-0.5 mb-2 text-xs text-gray-500 dark:text-gray-400">{t('users.legalGuardianHelp')}</p>

      {value && (
        <div className="mb-2 flex items-center justify-between gap-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 px-3 py-2">
          <div className="min-w-0 flex items-center gap-2.5">
            <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-900/50 text-xs font-medium text-amber-700 dark:text-amber-400">
              {value.firstname?.[0] || '?'}
              {value.lastname?.[0] || ''}
            </span>
            <span className="text-sm font-medium text-gray-900 dark:text-white truncate">
              {displayName || t('users.guardianFallback', { id: value.id })}
            </span>
          </div>
          {showFileLink && (
            <Link
              to={`/users/${value.id}`}
              className="shrink-0 text-sm text-indigo-600 dark:text-indigo-400 hover:underline"
            >
              {t('users.openPatronFile')}
            </Link>
          )}
        </div>
      )}

      <Typeahead
        label={value ? t('users.legalGuardianReplace') : t('users.legalGuardianSearch')}
        value={draft}
        onChange={setDraft}
        items={candidates}
        getItemId={(u) => u.id}
        selectedId={value?.id}
        onSelect={(u) => {
          onChange(u);
          setDraft('');
          setResults([]);
        }}
        renderItem={(u) => (
          <>
            {formatUserShortName(u)}{' '}
            <span className="text-gray-500 font-mono text-xs">{u.id}</span>
          </>
        )}
        placeholder={t('users.searchPlaceholder')}
        leftIcon={<Search className="h-4 w-4" />}
        loading={visibleSearching}
        error={error}
      />
      {visibleSearching && <p className="text-xs text-gray-500 mt-1">{t('common.loading')}</p>}
    </div>
  );
}
