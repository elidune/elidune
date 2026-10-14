import type { TFunction } from 'i18next';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';

const ITEM_HELD_FOR_OTHER = 'item_held_for_other';

const SET_ASIDE_CODES = [
  'set_aside_item_mismatch',
  'set_aside_wrong_title',
  'set_aside_not_oldest',
] as const;

export interface HeldSetAsideToast {
  variant: 'success' | 'error';
  message: string;
}

/** JSON body for `POST /loans/items/{barcode}/return`. Both ids are strings. */
export interface HeldSetAsideRequest {
  holdId: string;
  itemId: string;
}

export interface HeldSetAsideDeps {
  /** `details.holdId` from the `item_held_for_other` refusal. */
  holdId: string | null;
  /** Scanned copy. Looked up when this is blank. */
  itemId: string | null;
  resolveItemId?: (barcode: string) => Promise<string | null>;
  /** `POST /loans/items/{barcode}/return` with `{ holdId, itemId }`. */
  returnSetAside: (barcode: string, body: HeldSetAsideRequest) => Promise<unknown>;
  successMessage: (result: unknown) => string;
  errorMessage: (error: unknown) => string;
  showToast: (toast: HeldSetAsideToast) => void;
  closeDialog: () => void;
  clearBarcode: () => void;
  refocusBarcode: () => void;
}

const pendingBarcodes = new Set<string>();

function record(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function text(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  const trimmed = String(value).trim();
  return trimmed || null;
}

/**
 * Hold id from a checkout 422 `item_held_for_other`.
 * Only `details.holdId` is read. Card, initials, and name are ignored.
 */
export function heldForOtherHoldId(error: unknown): string | null {
  const data = record(record((error as { response?: unknown })?.response)?.data);
  if (!data || text(data.code) !== ITEM_HELD_FOR_OTHER) return null;
  return text(record(data.details)?.holdId);
}

/**
 * Desk sentence after a set-aside return.
 * A transit id means the copy left for `destinationSiteName`.
 * Another reader's name on the body is not read.
 */
export function heldSetAsideSuccessMessage(result: unknown, t: TFunction): string {
  const body = record(result);
  const transitId = text(body?.transitId);
  if (!transitId) return t('loans.heldSetAsideReady');
  const site = text(body?.destinationSiteName) || t('loans.pickSiteUnknown');
  return t('loans.heldSetAsideTransit', { site });
}

/**
 * Set a held copy aside on the checkout screen.
 * Posts `{ holdId, itemId }` and stays on checkout. A second call for the same
 * barcode waits out the first one. The response identity is never shown.
 */
export async function commitHeldSetAside(barcode: string, deps: HeldSetAsideDeps): Promise<void> {
  const trimmed = barcode.trim();
  if (!trimmed || pendingBarcodes.has(trimmed)) return;
  pendingBarcodes.add(trimmed);
  try {
    const holdId = text(deps.holdId);
    if (!holdId) {
      deps.showToast({ variant: 'error', message: deps.errorMessage(undefined) });
      return;
    }
    let itemId = text(deps.itemId);
    if (!itemId && deps.resolveItemId) {
      try {
        itemId = text(await deps.resolveItemId(trimmed));
      } catch {
        itemId = null;
      }
    }
    if (!itemId) {
      deps.showToast({ variant: 'error', message: deps.errorMessage(undefined) });
      return;
    }
    const result = await deps.returnSetAside(trimmed, { holdId, itemId });
    deps.closeDialog();
    deps.clearBarcode();
    deps.showToast({ variant: 'success', message: deps.successMessage(result) });
    deps.refocusBarcode();
  } catch (error: unknown) {
    deps.showToast({ variant: 'error', message: deps.errorMessage(error) });
  } finally {
    pendingBarcodes.delete(trimmed);
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

/**
 * Localized set-aside refusal. The three 422 codes have their own sentences.
 * Any other code uses the generic error, never the server text or a name.
 */
export function heldSetAsideServerMessage(error: unknown, t: TFunction): string {
  const code = getApiErrorCode(error);
  if (code && (SET_ASIDE_CODES as readonly string[]).includes(code)) {
    return t(`errors.apiCode.${code}`);
  }
  return getApiErrorMessage(error, t, { allowRawMessage: false }) || t('errors.generic');
}
