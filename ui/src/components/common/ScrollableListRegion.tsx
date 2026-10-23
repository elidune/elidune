import type { ReactNode } from 'react';

/**
 * Root class for scroll containers so features like IntersectionObserver can use
 * `getListScrollRoot()` site-wide.
 */
export const APP_LIST_SCROLL_ROOT_CLASS = 'app-list-scroll';

const SCROLL_CLASSES = `${APP_LIST_SCROLL_ROOT_CLASS} min-h-0 max-lg:overflow-visible max-lg:max-h-none lg:overflow-auto lg:min-h-0`;

/** Grows inside a flex column parent so the list scrolls instead of the page (lg+). */
const SCROLL_FILL_CLASSES = `${APP_LIST_SCROLL_ROOT_CLASS} min-h-0 max-lg:overflow-visible lg:flex-1 lg:min-h-0 lg:overflow-auto`;

interface ScrollableListRegionProps {
  children: ReactNode;
  className?: string;
  'aria-label'?: string;
  /** Use when the parent is a flex column with bounded height; this region takes remaining space (lg+). */
  fill?: boolean;
}

/**
 * Desktop (`lg+`): fixed-height band — page chrome stays static, only this region scrolls.
 * Mobile: flows with the page; scroll via the main app shell (`#app-page-scroll`).
 */
export default function ScrollableListRegion({
  children,
  className,
  'aria-label': ariaLabel,
  fill = false,
}: ScrollableListRegionProps) {
  const base = fill ? SCROLL_FILL_CLASSES : SCROLL_CLASSES;
  return (
    <div className={className ? `${base} ${className}` : base} aria-label={ariaLabel}>
      {children}
    </div>
  );
}
