import type { TFunction } from 'i18next';
import { getApiErrorMessage, getApiErrorRawMessage } from '@/utils/apiError';

export interface HeldSetAsideToast {
  variant: 'success' | 'error';
  message: string;
}

export interface HeldSetAsideDeps {
  /** Existing check-in: `POST /loans/items/{barcode}/return`. */
  returnByBarcode: (barcode: string) => Promise<unknown>;
  successMessage: string;
  errorMessage: (error: unknown) => string;
  showToast: (toast: HeldSetAsideToast) => void;
  closeDialog: () => void;
  clearBarcode: () => void;
  refocusBarcode: () => void;
}

/**
 * Set a held copy aside on the checkout screen.
 * Uses the same check-in that moves a shelf copy onto the holds shelf (`to_pick`).
 * The response is ignored: another reader's name is never shown, and checkout does not change tab.
 */
export async function commitHeldSetAside(barcode: string, deps: HeldSetAsideDeps): Promise<void> {
  const trimmed = barcode.trim();
  if (!trimmed) return;
  try {
    await deps.returnByBarcode(trimmed);
    deps.closeDialog();
    deps.clearBarcode();
    deps.showToast({ variant: 'success', message: deps.successMessage });
    deps.refocusBarcode();
  } catch (error: unknown) {
    deps.showToast({ variant: 'error', message: deps.errorMessage(error) });
  }
}

export interface DismissHeldSetAsideDeps {
  closeDialog: () => void;
  clearBarcode: () => void;
  refocusBarcode: () => void;
}

/** « Annuler » closes the popup and refocuses the barcode. The value stays. */
export function commitDismissHeldSetAside(deps: DismissHeldSetAsideDeps): void {
  deps.closeDialog();
  deps.refocusBarcode();
}

/** Server `message`, then the localized API error. A name on the body is not read. */
export function heldSetAsideServerMessage(error: unknown, t: TFunction): string {
  const raw = getApiErrorRawMessage(error);
  if (raw) return raw;
  return getApiErrorMessage(error, t) || t('loans.errorReturningLoan');
}

/**
 * `item_held_for_other` opens the set-aside dialog with a message and no other reader.
 * `item_to_pick_for_other` does not, and keeps the existing check-in screen.
 */
export function isInPlaceHeldSetAside(message: string | null | undefined): boolean {
  return Boolean(message?.trim());
}
