import { useEffect, type RefObject } from 'react';

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function getFocusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter((el) => {
    if (el.getAttribute('aria-hidden') === 'true') return false;
    if (el.closest('[aria-hidden="true"]')) return false;
    const style = window.getComputedStyle(el);
    return style.display !== 'none' && style.visibility !== 'hidden';
  });
}

export function isTopmostDialog(el: HTMLElement): boolean {
  const dialogs = document.querySelectorAll<HTMLElement>('[role="dialog"][aria-modal="true"]');
  return dialogs[dialogs.length - 1] === el;
}

export function noModalDialogOpen(): boolean {
  return document.querySelectorAll('[role="dialog"][aria-modal="true"]').length === 0;
}

interface UseFocusTrapOptions {
  enabled: boolean;
  containerRef: RefObject<HTMLElement | null>;
  onEscape?: () => void;
  shouldHandle?: (root: HTMLElement) => boolean;
  /** Skip the close control when focusing the first field (modals). */
  skipCloseControl?: boolean;
}

export function useFocusTrap({
  enabled,
  containerRef,
  onEscape,
  shouldHandle,
  skipCloseControl = false,
}: UseFocusTrapOptions) {
  useEffect(() => {
    if (!enabled) return;

    const root = containerRef.current;
    if (!root) return;

    const previouslyFocused =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;

    const focusInitial = () => {
      const items = getFocusable(root);
      const preferred = skipCloseControl
        ? items.find((el) => el.getAttribute('data-modal-close') !== 'true')
        : items[0];
      (preferred ?? items[0] ?? root).focus();
    };

    const frame = requestAnimationFrame(focusInitial);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (shouldHandle && !shouldHandle(root)) return;

      if (e.key === 'Escape') {
        if (onEscape) {
          e.preventDefault();
          onEscape();
        }
        return;
      }

      if (e.key !== 'Tab') return;

      const items = getFocusable(root);
      if (items.length === 0) {
        e.preventDefault();
        return;
      }

      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;

      if (e.shiftKey) {
        if (active === first || !root.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last || !root.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown);

    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener('keydown', handleKeyDown);
      if (previouslyFocused && document.contains(previouslyFocused) && typeof previouslyFocused.focus === 'function') {
        previouslyFocused.focus();
      }
    };
  }, [enabled, containerRef, onEscape, shouldHandle, skipCloseControl]);
}
