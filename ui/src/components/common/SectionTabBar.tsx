import type { ReactNode } from 'react';

export type SectionTabItem<T extends string = string> = {
  id: T;
  label: ReactNode;
  icon?: React.ComponentType<{ className?: string }>;
  leading?: ReactNode;
};

export type SectionTabBarProps<T extends string = string> = {
  tabs: SectionTabItem<T>[];
  activeTab: T;
  onSelectTab: (tab: T) => void;
  ariaLabel: string;
  /** Nested panels (e.g. public type editor). */
  size?: 'default' | 'compact';
  scrollable?: boolean;
  className?: string;
};

function tabButtonClass(active: boolean, size: 'default' | 'compact'): string {
  const py = size === 'compact' ? 'py-2' : 'py-3';
  return [
    'flex items-center gap-2 shrink-0',
    py,
    'px-1 text-sm font-medium border-b-2 transition-colors whitespace-nowrap',
    active
      ? 'border-amber-500 text-amber-600 dark:text-amber-400'
      : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200',
  ].join(' ');
}

export default function SectionTabBar<T extends string>({
  tabs,
  activeTab,
  onSelectTab,
  ariaLabel,
  size = 'default',
  scrollable = false,
  className,
}: SectionTabBarProps<T>) {
  const gap = size === 'compact' ? 'gap-4' : 'gap-6';
  const navClass = scrollable
    ? `-mb-px flex ${gap} overflow-x-auto`
    : `-mb-px flex flex-wrap ${gap}`;

  return (
    <div className={['border-b border-gray-200 dark:border-gray-700', className].filter(Boolean).join(' ')}>
      <nav className={navClass} role="tablist" aria-label={ariaLabel}>
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const active = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={active}
              aria-controls={`section-panel-${tab.id}`}
              id={`section-tab-${tab.id}`}
              onClick={() => onSelectTab(tab.id)}
              className={tabButtonClass(active, size)}
            >
              {tab.leading ?? (Icon ? <Icon className="h-4 w-4 shrink-0" aria-hidden /> : null)}
              {tab.label}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
