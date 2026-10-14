import axios from 'axios';
import type { TFunction } from 'i18next';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { loanRefusalMayForce } from '@/utils/circulationForce';
import { isCheckoutScanNoticeShown } from '@/utils/deskScanAlert';
import { itemStatusActionMessage, readCheckoutItemStatus } from '@/utils/itemStatusAction';
import { isCheckoutDeclined } from '@/utils/toPickDesk';

const ITEM_STATUS_ACTION_REQUIRED = 'item_status_action_required';
const ITEM_HELD_FOR_OTHER = 'item_held_for_other';
/** Already on the holds shelf for another reader. */
const ITEM_TO_PICK_FOR_OTHER = 'item_to_pick_for_other';

export interface CheckoutDeskActions {
  /** « Forcer le prêt ». */
  force: boolean;
  /** « Mettre de côté ». */
  setAside: boolean;
}

/**
 * Actions under a checkout refusal.
 * `item_to_pick_for_other` is already set aside: the sentence only, even when
 * `forceable` is true. `item_held_for_other` keeps set-aside and never force.
 * A status action has neither button.
 * Any remaining code offers force only when the server `forceable` is true.
 */
export function checkoutDeskActions(
  code: string | null | undefined,
  forceable: unknown,
  canForce = true,
): CheckoutDeskActions {
  const key = code?.trim() ?? '';
  if (key === ITEM_TO_PICK_FOR_OTHER) return { force: false, setAside: false };
  if (key === ITEM_HELD_FOR_OTHER) return { force: false, setAside: true };
  if (key === ITEM_STATUS_ACTION_REQUIRED) return { force: false, setAside: false };
  return { force: loanRefusalMayForce(forceable, canForce), setAside: false };
}

function refusalForceable(error: unknown): unknown {
  if (!axios.isAxiosError(error)) return undefined;
  const data = error.response?.data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) return undefined;
  return (data as { forceable?: unknown }).forceable;
}

/** Same decision as `checkoutDeskActions`, read from a loan 422. */
export function checkoutDeskActionsFromError(error: unknown, canForce = true): CheckoutDeskActions {
  return checkoutDeskActions(getApiErrorCode(error), refusalForceable(error), canForce);
}

/**
 * Desk checkout copy.
 * A known `errors.apiCode.*` wins, including codes renewal also uses
 * (`subscription_expired`, `fines_over_threshold`), so the desk does not show
 * the patron renewal sentence. `item_status_action_required` uses the
 * `itemStatus` sentence. An unmapped body is the generic error,
 * never the English server sentence.
 */
export function deskCheckoutMessage(error: unknown, t: TFunction): string {
  const code = getApiErrorCode(error);
  if (code === ITEM_STATUS_ACTION_REQUIRED) {
    return itemStatusActionMessage(readCheckoutItemStatus(error), t);
  }
  if (code) {
    const key = `errors.apiCode.${code}`;
    const translated = t(key);
    if (translated !== key) return translated;
  }
  return getApiErrorMessage(error, t, { allowRawMessage: false }) || t('loans.errorCreatingLoan');
}

/** Confirmation dialog: localized refusal, then the force question. */
export function forceBorrowDialogMessage(error: unknown, t: TFunction): string {
  return `${deskCheckoutMessage(error, t)}\n\n${t('loans.forceBorrowConfirm')}`;
}

/**
 * Toast / inline error after checkout.
 * Cancel and an already-shown scan notice stay silent.
 */
export function deskBorrowErrorText(error: unknown, t: TFunction): string | null {
  if (isCheckoutDeclined(error) || isCheckoutScanNoticeShown(error)) return null;
  return deskCheckoutMessage(error, t);
}
