import { useTranslation } from 'react-i18next';
import type { AccountTypeDefinition } from '@/types';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import { formControlClass } from '@/utils/formControl';

interface AccountTypeSelectProps {
  id: string;
  value: string;
  accountTypes: readonly AccountTypeDefinition[];
  onChange: (code: string) => void;
  invalid?: boolean;
  loadError?: string | null;
}

/** Account-type picker. Options are the rows returned by GET /account-types. */
export default function AccountTypeSelect({
  id,
  value,
  accountTypes,
  onChange,
  invalid = false,
  loadError = null,
}: AccountTypeSelectProps) {
  const { t } = useTranslation();
  const known = accountTypes.some((row) => row.code.toLowerCase() === value.trim().toLowerCase());

  return (
    <>
      <select
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        required
        aria-required="true"
        aria-invalid={invalid || !!loadError}
        className={formControlClass({ error: invalid || !!loadError, className: 'w-full min-w-0' })}
      >
        {value && !known ? (
          <option value={value}>{localizedAccountTypeLabel(t, value, accountTypes)}</option>
        ) : null}
        {accountTypes.map((row) => (
          <option key={row.code} value={row.code}>
            {localizedAccountTypeLabel(t, row.code, accountTypes)}
          </option>
        ))}
      </select>
      {loadError ? (
        <p role="alert" className="mt-1 text-sm text-red-600 dark:text-red-400">
          {loadError}
        </p>
      ) : null}
    </>
  );
}
