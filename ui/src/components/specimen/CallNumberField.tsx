import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  type CallNumberOption,
  validateCallNumber,
} from '@/utils/callNumber';
import { formControlClass, formLabelClass } from '@/utils/formControl';

const CUSTOM_VALUE = '__custom__';

interface CallNumberFieldProps {
  value: string;
  onChange: (value: string) => void;
  /** Generated formats (Dewey / genre / legacy). */
  suggestions?: CallNumberOption[];
  label?: string;
  placeholder?: string;
  excludeSpecimenId?: string;
  disabled?: boolean;
  inputId?: string;
}

function optionLabel(
  t: (key: string, opts?: Record<string, string>) => string,
  opt: CallNumberOption,
): string {
  const key =
    opt.kind === 'dewey'
      ? 'items.callNumberOptionDewey'
      : opt.kind === 'genre'
        ? 'items.callNumberOptionGenre'
        : 'items.callNumberOptionLegacy';
  return t(key, { value: opt.value });
}

export default function CallNumberField({
  value,
  onChange,
  suggestions = [],
  label,
  placeholder,
  disabled,
  inputId,
}: CallNumberFieldProps) {
  const { t } = useTranslation();
  const [forceCustom, setForceCustom] = useState(false);
  /** Once the user picks an option or types, stop following suggestion updates. */
  const lockedByUser = useRef(false);
  const initialized = useRef(false);

  const matchingSuggestion = useMemo(
    () => suggestions.find((s) => s.value === value) ?? null,
    [suggestions, value],
  );

  const isCustom = forceCustom || !matchingSuggestion;
  const selectValue = isCustom ? CUSTOM_VALUE : matchingSuggestion!.value;

  // Keep the preferred suggestion (first in list) while the field is still automatic.
  useEffect(() => {
    if (disabled || suggestions.length === 0) return;

    if (!initialized.current) {
      initialized.current = true;
      // Preserve an existing value (edit copy / restored draft).
      if (value.trim() !== '') {
        lockedByUser.current = true;
        return;
      }
    }

    if (lockedByUser.current || forceCustom) return;

    const preferred = suggestions[0]!.value;
    if (value === preferred) return;
    setForceCustom(false);
    onChange(preferred);
  }, [suggestions, value, disabled, onChange, forceCustom]);

  const validationError =
    value !== '' && !validateCallNumber(value) ? t('items.callNumberInvalid') : undefined;

  const handleSelectChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    lockedByUser.current = true;
    const next = e.target.value;
    if (next === CUSTOM_VALUE) {
      setForceCustom(true);
      return;
    }
    setForceCustom(false);
    onChange(next);
  };

  const handleCustomChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    lockedByUser.current = true;
    setForceCustom(true);
    onChange(e.target.value);
  };

  const selectId = inputId ? `${inputId}-select` : undefined;
  const customId = inputId ?? undefined;

  return (
    <div className="w-full space-y-2">
      {label && <label className={formLabelClass()}>{label}</label>}
      <select
        id={selectId}
        value={selectValue}
        onChange={handleSelectChange}
        disabled={disabled}
        className={formControlClass({ className: 'w-full' })}
        aria-label={label ?? t('items.callNumber')}
      >
        {suggestions.map((opt) => (
          <option key={`${opt.kind}:${opt.value}`} value={opt.value}>
            {optionLabel(t, opt)}
          </option>
        ))}
        <option value={CUSTOM_VALUE}>{t('items.callNumberCustom')}</option>
      </select>
      {isCustom && (
        <input
          id={customId}
          value={value}
          onChange={handleCustomChange}
          placeholder={placeholder}
          disabled={disabled}
          className={formControlClass({
            error: !!validationError,
            className: 'w-full',
          })}
        />
      )}
      {validationError && (
        <p className="text-sm text-red-600 dark:text-red-400">{validationError}</p>
      )}
    </div>
  );
}
