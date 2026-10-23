import type { ReactNode } from 'react';

type ListPageLayoutProps = {
  children: ReactNode;
  className?: string;
};

/**
 * List/search pages: on lg+ fills the shell height so only `ScrollableListRegion fill` scrolls.
 * On mobile, `contents` keeps a single page scroll via `#app-page-scroll`.
 */
export default function ListPageLayout({ children, className = '' }: ListPageLayoutProps) {
  return (
    <div
      className={[
        'flex flex-col gap-4 max-lg:contents lg:min-h-0 lg:flex-1 lg:overflow-hidden',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {children}
    </div>
  );
}
