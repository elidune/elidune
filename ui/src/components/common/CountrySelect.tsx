import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import Typeahead from './Typeahead';
import {
  countryCodes,
  formatCountryName,
} from '@/utils/countryTimezone';

export interface CountrySelectProps {
  label: string;
  /** ISO 3166-1 alpha-2, or empty when the stored value is not a known code. */
  value: string;
  onChange: (code: string) => void;
  error?: string;
  placeholder?: string;
  required?: boolean;
  disabled?: boolean;
}

/** Searchable country combobox. The stored value is the ISO alpha-2 code. */
export default function CountrySelect({
  label,
  value,
  onChange,
  error,
  placeholder,
  required,
  disabled,
}: CountrySelectProps) {
  const { i18n } = useTranslation();
  const language = i18n.resolvedLanguage || i18n.language || 'en';
  const [editing, setEditing] = useState(false);
  const [query, setQuery] = useState('');

  const codes = useMemo(
    () =>
      countryCodes().sort((a, b) =>
        formatCountryName(a, language).localeCompare(formatCountryName(b, language), language),
      ),
    [language],
  );

  const selectedName = value ? formatCountryName(value, language) : '';
  const display = editing ? query : selectedName;
  const items = useMemo(() => {
    if (!editing) return [];
    const q = query.trim().toLowerCase();
    const matches = q
      ? codes.filter((code) => {
          const name = formatCountryName(code, language).toLowerCase();
          return name.includes(q) || code.toLowerCase().includes(q);
        })
      : codes;
    return matches.slice(0, 80);
  }, [codes, editing, language, query]);

  return (
    <Typeahead
      label={label}
      value={display}
      placeholder={placeholder}
      error={error}
      required={required}
      disabled={disabled}
      selectedId={value || null}
      variant="stack"
      onChange={(next) => {
        setEditing(true);
        setQuery(next);
      }}
      onFocus={() => {
        setEditing(true);
        setQuery(selectedName);
      }}
      onBlur={() => {
        const typed = query.trim().toLowerCase();
        const exact = codes.find((code) => {
          const name = formatCountryName(code, language).toLowerCase();
          return name === typed || code.toLowerCase() === typed;
        });
        if (exact) onChange(exact);
        setEditing(false);
      }}
      items={items}
      getItemId={(code) => code}
      onSelect={(code) => {
        onChange(code);
        setQuery(formatCountryName(code, language));
        setEditing(false);
      }}
      renderItem={(code) => formatCountryName(code, language)}
    />
  );
}
