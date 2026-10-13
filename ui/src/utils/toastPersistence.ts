export interface PersistentToast {
  variant: 'info' | 'success' | 'error';
  /** Auto-dismiss after ms. `0` stays until it is closed. */
  duration: number;
}

/** Error toasts stay on screen (`duration` 0). Info and success still time out. */
export function isPersistentErrorToast(toast: PersistentToast): boolean {
  return toast.variant === 'error' && toast.duration === 0;
}

/** Drop sticky error toasts. Other toasts, including timed errors, stay. */
export function withoutPersistentErrorToasts<T extends PersistentToast>(toasts: T[]): T[] {
  const next = toasts.filter((toast) => !isPersistentErrorToast(toast));
  return next.length === toasts.length ? toasts : next;
}
