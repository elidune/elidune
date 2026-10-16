import type { Location } from 'react-router-dom';

/** When true, sidebar sections may collapse extra items to reduce scrolling (from 1024×768 and smaller). */
export const SIDEBAR_NAV_COMPACT_MEDIA = '(max-width: 1024px), (max-height: 768px)';

/** Visible item count when a section is collapsed (roughly the first two-thirds, at least 2). */
export function navSectionCollapsedVisibleCount(itemCount: number): number {
  if (itemCount <= 2) return itemCount;
  const twoThirds = Math.floor((itemCount * 2) / 3);
  return Math.max(2, Math.min(twoThirds, itemCount - 1));
}

export function isSidebarNavItemActive(href: string, location: Location): boolean {
  if (href === '/home') return location.pathname === '/home';
  if (href === '/catalog') {
    return location.pathname === '/catalog' || location.pathname.startsWith('/biblios/');
  }
  if (href.includes('?')) {
    return `${location.pathname}${location.search}` === href;
  }
  return location.pathname === href || location.pathname.startsWith(`${href}/`);
}
