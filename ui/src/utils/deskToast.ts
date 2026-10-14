export interface DeskToast {
  message: string;
  variant?: 'info' | 'success' | 'error';
  duration?: number;
}

type ShowDeskToast = (toast: DeskToast) => void;

let show: ShowDeskToast | null = null;

/** `ToastProvider` registers the live toaster. Absent in tests that only render markup. */
export function bindDeskToast(next: ShowDeskToast | null): void {
  show = next;
}

/** Shows a toast when the desk toaster is mounted. No-op otherwise. */
export function showDeskToast(toast: DeskToast): void {
  show?.(toast);
}
