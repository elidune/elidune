import { useMemo, useState } from 'react';
import Typeahead from './Typeahead';
import { libraryTimeZoneOptions } from '@/utils/timeZones';

export interface TimezoneSelectProps {
  label: string;
  value: string;
  onChange: (timezone: string) => void;
  hint?: string;
  error?: string;
  placeholder?: string;
  disabled?: boolean;
}

/**
 * Searchable IANA time zone combobox. The closed field shows `value`.
 * Typing filters the shared Typeahead list; an exact match is committed on blur.
 */
export default function TimezoneSelect({
  label,
  value,
  onChange,
  hint,
  error,
  placeholder,
  disabled,
}: TimezoneSelectProps) {
  const zones = useMemo(() => libraryTimeZoneOptions(value), [value]);
  const [editing, setEditing] = useState(false);
  const [query, setQuery] = useState('');

  const display = editing ? query : value;
  const items = useMemo(() => {
    if (!editing) return [];
    const q = query.trim().toLowerCase();
    if (!q) return zones;
    return zones.filter((zone) => zone.toLowerCase().includes(q));
  }, [editing, query, zones]);

  const commitQuery = () => {
    const typed = query.trim();
    const match = zones.find((zone) => zone.toLowerCase() === typed.toLowerCase());
    if (match && match !== value) onChange(match);
    setEditing(false);
  };

  return (
    <Typeahead
      label={label}
      hint={hint}
      error={error}
      placeholder={placeholder}
      disabled={disabled}
      value={display}
      onChange={(next) => {
        setEditing(true);
        setQuery(next);
      }}
      onFocus={() => {
        setEditing(true);
        setQuery(value);
      }}
      onBlur={commitQuery}
      items={items}
      getItemId={(zone) => zone}
      selectedId={value}
      variant="stack"
      onSelect={(zone) => {
        onChange(zone);
        setQuery(zone);
        setEditing(false);
      }}
      renderItem={(zone) => zone}
    />
  );
}
