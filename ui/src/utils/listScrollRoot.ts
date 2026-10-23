import { APP_LIST_SCROLL_ROOT_CLASS } from '@/components/common/ScrollableListRegion';

/** Nearest scrollport for list infinite-scroll (list band on desktop, page shell on mobile). */
export function getListScrollRoot(from: Element | null): Element | null {
  if (!from) return null;

  let node: Element | null = from;
  while (node && node !== document.documentElement) {
    if (node instanceof HTMLElement && node.classList.contains(APP_LIST_SCROLL_ROOT_CLASS)) {
      const { overflowY } = getComputedStyle(node);
      const canScrollY = overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'overlay';
      if (canScrollY && node.scrollHeight > node.clientHeight + 1) {
        return node;
      }
    }
    node = node.parentElement;
  }

  return document.getElementById('app-page-scroll');
}
