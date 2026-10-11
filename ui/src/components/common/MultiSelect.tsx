import {
  KeyboardEvent,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Check, ChevronDown, X } from 'lucide-react';
import { formControlClass, formLabelClass } from '@/utils/formControl';

export interface MultiSelectOption {
  value: string;
  label: string;
}

export interface MultiSelectProps {
  id?: string;
  label: string;
  options: MultiSelectOption[];
  value: string[];
  onChange: (next: string[]) => void;
  /** Selecting this value clears every other value, and selecting any other value clears it. */
  exclusiveValue?: string;
  placeholder?: string;
  hint?: string;
  /** Second helper line, included in aria-describedby (shown under the hint). */
  note?: string;
  error?: string;
  required?: boolean;
  disabled?: boolean;
}

function nextMultiSelectValue(
  current: string[],
  option: string,
  exclusiveValue?: string,
): string[] {
  if (exclusiveValue != null && option === exclusiveValue) {
    return current.length === 1 && current[0] === exclusiveValue ? [] : [exclusiveValue];
  }
  const base = exclusiveValue ? current.filter((item) => item !== exclusiveValue) : current;
  if (base.includes(option)) return base.filter((item) => item !== option);
  return [...base, option];
}

export default function MultiSelect({
  id,
  label,
  options,
  value,
  onChange,
  exclusiveValue,
  placeholder,
  hint,
  note,
  error,
  required = false,
  disabled = false,
}: MultiSelectProps) {
  const { t } = useTranslation();
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const listId = `${inputId}-listbox`;
  const hintId = hint ? `${inputId}-hint` : undefined;
  const noteId = note ? `${inputId}-note` : undefined;
  const errorId = error ? `${inputId}-error` : undefined;
  const describedBy = [errorId, noteId, hintId].filter(Boolean).join(' ') || undefined;

  const rootRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(-1);
  const [menuBox, setMenuBox] = useState<{ top: number; left: number; width: number; maxHeight: number } | null>(
    null,
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return options;
    return options.filter((option) => option.label.toLowerCase().includes(q));
  }, [options, query]);

  const safeIndex = activeIndex >= 0 && activeIndex < filtered.length ? activeIndex : -1;

  const labelFor = (optionValue: string) =>
    options.find((option) => option.value === optionValue)?.label ?? optionValue;

  useEffect(() => {
    if (!open) return;
    const placeMenu = () => {
      const el = rootRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const gap = 4;
      const spaceBelow = window.innerHeight - rect.bottom - gap;
      const spaceAbove = rect.top - gap;
      const openUp = spaceBelow < 200 && spaceAbove > spaceBelow;
      const maxHeight = Math.max(120, Math.min(240, openUp ? spaceAbove : spaceBelow));
      setMenuBox({
        top: openUp ? Math.max(8, rect.top - gap - maxHeight) : rect.bottom + gap,
        left: rect.left,
        width: rect.width,
        maxHeight,
      });
    };
    placeMenu();
    window.addEventListener('resize', placeMenu);
    window.addEventListener('scroll', placeMenu, true);
    return () => {
      window.removeEventListener('resize', placeMenu);
      window.removeEventListener('scroll', placeMenu, true);
    };
  }, [open, filtered.length]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || listRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', onPointerDown);
    return () => document.removeEventListener('mousedown', onPointerDown);
  }, [open]);

  const toggleOption = (optionValue: string) => {
    onChange(nextMultiSelectValue(value, optionValue, exclusiveValue));
    setQuery('');
    inputRef.current?.focus();
  };

  const removeAt = (index: number) => {
    onChange(value.filter((_, itemIndex) => itemIndex !== index));
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      if (!open) setOpen(true);
      setActiveIndex((current) => {
        if (filtered.length === 0) return -1;
        const base = current < 0 || current >= filtered.length ? -1 : current;
        return base < 0 ? 0 : (base + 1) % filtered.length;
      });
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      if (!open) setOpen(true);
      setActiveIndex((current) => {
        if (filtered.length === 0) return -1;
        const base = current < 0 || current >= filtered.length ? 0 : current;
        return (base - 1 + filtered.length) % filtered.length;
      });
      return;
    }
    if (event.key === 'Home' && open && filtered.length > 0) {
      event.preventDefault();
      setActiveIndex(0);
      return;
    }
    if (event.key === 'End' && open && filtered.length > 0) {
      event.preventDefault();
      setActiveIndex(filtered.length - 1);
      return;
    }
    if (event.key === 'Enter' && open && safeIndex >= 0) {
      event.preventDefault();
      const picked = filtered[safeIndex];
      if (picked) toggleOption(picked.value);
      return;
    }
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      event.stopPropagation();
      event.nativeEvent.stopImmediatePropagation();
      setOpen(false);
      return;
    }
    if (event.key === 'Backspace' && query === '' && value.length > 0) {
      event.preventDefault();
      removeAt(value.length - 1);
    }
  };

  const activeOptionId = safeIndex >= 0 ? `${listId}-opt-${safeIndex}` : undefined;

  const list = open && menuBox
    ? createPortal(
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          aria-multiselectable="true"
          aria-labelledby={`${inputId}-label`}
          style={{
            position: 'fixed',
            top: menuBox.top,
            left: menuBox.left,
            width: menuBox.width,
            maxHeight: menuBox.maxHeight,
          }}
          className="z-[200] overflow-y-auto rounded-lg border border-gray-300 bg-white py-1 shadow-lg dark:border-gray-700 dark:bg-gray-900"
        >
          {filtered.length === 0 ? (
            <li className="px-4 py-2 text-sm text-gray-500 dark:text-gray-400">{t('common.noResults')}</li>
          ) : (
            filtered.map((option, index) => {
              const selected = value.includes(option.value);
              const active = index === safeIndex;
              return (
                <li
                  key={option.value}
                  id={`${listId}-opt-${index}`}
                  role="option"
                  aria-selected={selected}
                  className={`flex cursor-pointer items-center gap-2 px-3 py-2 text-sm ${
                    active
                      ? 'bg-amber-50 text-gray-900 dark:bg-amber-900/30 dark:text-gray-100'
                      : 'text-gray-900 dark:text-gray-100'
                  }`}
                  onMouseEnter={() => setActiveIndex(index)}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => toggleOption(option.value)}
                >
                  <Check className={`h-4 w-4 shrink-0 ${selected ? 'text-amber-600 dark:text-amber-400' : 'opacity-0'}`} aria-hidden />
                  <span className="min-w-0 truncate">{option.label}</span>
                </li>
              );
            })
          )}
        </ul>,
        document.body,
      )
    : null;

  return (
    <div className="w-full">
      <label id={`${inputId}-label`} htmlFor={inputId} className={formLabelClass()}>
        {label}
        {required && (
          <span className="ml-0.5 font-medium text-red-600 dark:text-red-400" aria-hidden="true">
            *
          </span>
        )}
      </label>
      <div
        ref={rootRef}
        className={formControlClass({
          error: !!error,
          autoHeight: true,
          className: `relative flex w-full cursor-text flex-wrap items-center gap-1.5 !px-2 focus-within:border-amber-500 focus-within:ring-2 dark:focus-within:ring-amber-500/40 ${
            error
              ? 'focus-within:border-red-500 focus-within:ring-red-500/20'
              : 'focus-within:ring-amber-500/20'
          } ${disabled ? 'pointer-events-none' : ''}`,
        })}
        onMouseDown={(event) => {
          if (disabled) return;
          if ((event.target as HTMLElement).closest('button')) return;
          inputRef.current?.focus();
          setOpen(true);
          setActiveIndex((current) => (current < 0 ? 0 : current));
        }}
      >
        {value.map((optionValue, index) => (
          <span
            key={`${optionValue}-${index}`}
            className="inline-flex max-w-full items-center gap-1 rounded-md bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-900 dark:bg-amber-900/40 dark:text-amber-100"
          >
            <span className="truncate">{labelFor(optionValue)}</span>
            <button
              type="button"
              className="rounded p-0.5 text-amber-800 hover:bg-amber-200/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 dark:text-amber-100 dark:hover:bg-amber-800/60"
              aria-label={t('common.removeNamed', { name: labelFor(optionValue) })}
              disabled={disabled}
              onClick={() => removeAt(index)}
            >
              <X className="h-3 w-3" aria-hidden />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          id={inputId}
          type="text"
          role="combobox"
          disabled={disabled}
          autoComplete="off"
          placeholder={value.length === 0 ? placeholder : undefined}
          value={query}
          aria-invalid={error ? true : undefined}
          aria-expanded={open}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={open ? activeOptionId : undefined}
          aria-describedby={describedBy}
          className="embedded-field h-7 min-w-[6rem] flex-1 border-0 bg-transparent p-0 text-sm text-gray-900 shadow-none outline-none ring-0 placeholder:text-gray-400 focus:border-0 focus:shadow-none focus:outline-none focus:ring-0 focus-visible:border-0 focus-visible:shadow-none focus-visible:outline-none focus-visible:ring-0 dark:text-gray-100 dark:placeholder:text-gray-500"
          onChange={(event) => {
            setQuery(event.target.value);
            setOpen(true);
            setActiveIndex(0);
          }}
          onBlur={() => setOpen(false)}
          onKeyDown={handleKeyDown}
        />
        <ChevronDown className="pointer-events-none h-4 w-4 shrink-0 text-gray-400" aria-hidden />
      </div>
      {list}
      {hint && !error && (
        <p id={hintId} className="mt-1 text-xs text-gray-500 dark:text-gray-400">
          {hint}
        </p>
      )}
      {note && (
        <p id={noteId} className="mt-1 text-xs text-gray-500 dark:text-gray-400">
          {note}
        </p>
      )}
      {error && (
        <p id={errorId} className="mt-1 text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      )}
    </div>
  );
}
