import { useEffect, useRef, type ReactNode } from 'react';

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
  className?: string;
};

function tabButtonClass(active: boolean, size: 'default' | 'compact'): string {
  const py = size === 'compact' ? 'py-2' : 'py-3';
  return [
    'flex min-h-[2.75rem] items-center gap-2 shrink-0 snap-start',
    py,
    'px-2 sm:px-3 text-sm font-medium border-b-2 transition-colors whitespace-nowrap',
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
  className,
}: SectionTabBarProps<T>) {
  const gap = size === 'compact' ? 'gap-x-3 sm:gap-x-4' : 'gap-x-4 sm:gap-x-6';
  const tabRefs = useRef(new Map<T, HTMLButtonElement>());
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const tabEl = tabRefs.current.get(activeTab);
    const scroller = scrollRef.current;
    if (!tabEl || !scroller) return;

    const tabStart = tabEl.offsetLeft;
    const tabEnd = tabStart + tabEl.offsetWidth;
    const viewStart = scroller.scrollLeft;
    const viewEnd = viewStart + scroller.clientWidth;
    const padding = 8;

    if (tabStart < viewStart + padding) {
      scroller.scrollTo({ left: Math.max(0, tabStart - padding), behavior: 'smooth' });
    } else if (tabEnd > viewEnd - padding) {
      scroller.scrollTo({
        left: tabEnd - scroller.clientWidth + padding,
        behavior: 'smooth',
      });
    }
  }, [activeTab]);

  return (
    <div className={['border-b border-gray-200 dark:border-gray-700', className].filter(Boolean).join(' ')}>
      <div
        ref={scrollRef}
        className="overflow-x-auto overflow-y-hidden overscroll-x-contain scroll-smooth [-webkit-overflow-scrolling:touch] [scrollbar-width:thin]"
        data-section-tab-scroll
      >
        <nav
          className={`-mb-px flex w-max min-w-full flex-nowrap items-end ${gap} snap-x snap-proximity px-1`}
          role="tablist"
          aria-label={ariaLabel}
        >
          {tabs.map((tab) => {
            const Icon = tab.icon;
            const active = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                ref={(node) => {
                  if (node) tabRefs.current.set(tab.id, node);
                  else tabRefs.current.delete(tab.id);
                }}
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
    </div>
  );
}
