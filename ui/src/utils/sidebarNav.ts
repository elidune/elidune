import type { Location } from 'react-router-dom';

/** When true, sidebar nav uses section accordions (from 1024×768 and smaller). */
export const SIDEBAR_NAV_COMPACT_MEDIA = '(max-width: 1024px), (max-height: 768px)';

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
