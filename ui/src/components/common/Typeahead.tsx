import { KeyboardEvent, ReactNode, useEffect, useId, useState } from 'react';
import Input from './Input';

export interface TypeaheadProps<T> {
  value: string;
  onChange: (value: string) => void;
  items: T[];
  getItemId: (item: T) => string;
  onSelect: (item: T) => void;
  renderItem: (item: T) => ReactNode;
  label?: string;
  placeholder?: string;
  leftIcon?: ReactNode;
  disabled?: boolean;
  loading?: boolean;
  selectedId?: string | null;
  /** `dropdown` overlays results; `stack` lists them below the field. */
  variant?: 'dropdown' | 'stack';
}

/**
 * Combobox-lite: Input + listbox with arrow/Enter/Escape. Not a full WAI-ARIA combobox rewrite.
 */
export default function Typeahead<T>({
  value,
  onChange,
  items,
  getItemId,
  onSelect,
  renderItem,
  label,
  placeholder,
  leftIcon,
  disabled,
  loading,
  selectedId,
  variant = 'stack',
}: TypeaheadProps<T>) {
  const listId = useId();
  const [activeIndex, setActiveIndex] = useState(-1);

  useEffect(() => {
    setActiveIndex(-1);
  }, [items]);

  const open = items.length > 0;
  const activeItem = activeIndex >= 0 ? items[activeIndex] : undefined;
  const activeOptionId = activeItem ? `${listId}-opt-${getItemId(activeItem)}` : undefined;

  const move = (delta: number) => {
    if (items.length === 0) return;
    setActiveIndex((current) => {
      if (current < 0) return delta > 0 ? 0 : items.length - 1;
      return (current + delta + items.length) % items.length;
    });
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      move(1);
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      move(-1);
      return;
    }
    if (event.key === 'Home' && items.length > 0) {
      event.preventDefault();
      setActiveIndex(0);
      return;
    }
    if (event.key === 'End' && items.length > 0) {
      event.preventDefault();
      setActiveIndex(items.length - 1);
      return;
    }
    if (event.key === 'Enter' && open) {
      const picked = activeItem ?? items[0];
      if (picked) {
        event.preventDefault();
        onSelect(picked);
      }
      return;
    }
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      setActiveIndex(-1);
    }
  };

  const listClassName =
    variant === 'dropdown'
      ? 'absolute z-10 w-full mt-1 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg max-h-60 overflow-y-auto'
      : 'mt-2 max-h-40 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700 divide-y divide-gray-200 dark:divide-gray-700';

  const optionBase =
    variant === 'dropdown'
      ? 'w-full px-4 py-3 text-left hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50'
      : 'w-full text-left px-3 py-2 text-sm hover:bg-gray-100 dark:hover:bg-gray-800';

  return (
    <div className={variant === 'dropdown' ? 'relative' : undefined}>
      <Input
        label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        leftIcon={leftIcon}
        disabled={disabled}
        aria-busy={loading}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={activeOptionId}
      />
      {open && (
        <ul id={listId} role="listbox" className={listClassName}>
          {items.map((item, index) => {
            const id = getItemId(item);
            const active = index === activeIndex;
            const selected = selectedId != null && selectedId === id;
            return (
              <li key={id} role="presentation">
                <button
                  type="button"
                  id={`${listId}-opt-${id}`}
                  role="option"
                  aria-selected={selected || active}
                  disabled={disabled}
                  onClick={() => onSelect(item)}
                  className={`${optionBase} ${
                    selected ? 'bg-amber-50 dark:bg-amber-900/30' : ''
                  } ${active ? 'bg-gray-100 dark:bg-gray-800' : ''}`}
                >
                  {renderItem(item)}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
