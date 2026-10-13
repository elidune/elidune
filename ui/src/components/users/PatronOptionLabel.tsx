import { useTranslation } from 'react-i18next';
import { formatUserShortName, patronOptionSecondary, type PatronNameSource } from '@/utils/userDisplay';

interface PatronOptionLabelProps {
  user: PatronNameSource;
}

/** Patron picker option: the name and the sign-in stay in separate slots. */
export default function PatronOptionLabel({ user }: PatronOptionLabelProps) {
  const { t } = useTranslation();
  const name = formatUserShortName(user);
  const secondary = patronOptionSecondary(user);
  const showSecondary =
    secondary != null && secondary.localeCompare(name, undefined, { sensitivity: 'accent' }) !== 0;

  return (
    <span className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
      <span data-patron-name className="font-medium text-gray-900 dark:text-white">
        {name || '—'}
      </span>
      {showSecondary ? (
        <span
          data-patron-id
          className="inline-flex items-baseline gap-1 font-normal text-gray-500 dark:text-gray-400"
        >
          <span className="sr-only">{t('profile.loginUsername')}</span>
          <span aria-hidden="true" className="select-none px-0.5">
            ·
          </span>
          <span className="font-mono text-xs tracking-wide">{secondary}</span>
        </span>
      ) : null}
    </span>
  );
}
