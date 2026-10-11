import { formatUserShortName, patronOptionSecondary, type PatronNameSource } from '@/utils/userDisplay';

interface PatronOptionLabelProps {
  user: PatronNameSource;
}

/** Patron picker option: name, then sign-in or card number set apart from it. */
export default function PatronOptionLabel({ user }: PatronOptionLabelProps) {
  const name = formatUserShortName(user);
  const secondary = patronOptionSecondary(user);
  const showSecondary = secondary != null && secondary !== name;

  return (
    <span className="inline-flex min-w-0 flex-wrap items-baseline gap-x-1">
      <span className="font-medium text-gray-900 dark:text-white">{name || '—'}</span>
      {showSecondary ? (
        <span className="font-normal text-gray-500 dark:text-gray-400">
          <span aria-hidden="true"> · </span>
          <span className="font-mono text-xs">{secondary}</span>
        </span>
      ) : null}
    </span>
  );
}
