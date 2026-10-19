import type { ReactNode } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';

export interface CollapsibleFormSectionProps {
  title: string;
  /** Shown under the title when the section is collapsed. */
  summary?: string | null;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}

export default function CollapsibleFormSection({
  title,
  summary,
  open,
  onToggle,
  children,
}: CollapsibleFormSectionProps) {
  return (
    <section className="rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/60 overflow-hidden">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className="w-full flex items-center justify-between gap-2 px-3 py-2 text-left hover:bg-gray-100/80 dark:hover:bg-gray-800 transition-colors"
      >
        <span className="min-w-0 flex-1">
          <span className="block text-xs font-semibold text-gray-600 dark:text-gray-300 uppercase tracking-wider">
            {title}
          </span>
          {!open && summary ? (
            <span className="mt-0.5 block text-xs font-normal normal-case tracking-normal text-gray-500 dark:text-gray-400 truncate">
              {summary}
            </span>
          ) : null}
        </span>
        {open ? (
          <ChevronDown className="h-4 w-4 shrink-0 text-gray-500" aria-hidden />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0 text-gray-500" aria-hidden />
        )}
      </button>
      {open ? (
        <div className="px-3 pb-3 pt-2 space-y-2.5 border-t border-gray-200/80 dark:border-gray-700/80">
          {children}
        </div>
      ) : null}
    </section>
  );
}
