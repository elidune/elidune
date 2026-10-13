import type { TFunction } from 'i18next';
import { getApiErrorMessage } from '@/utils/apiError';
import { isCheckoutScanNoticeShown } from '@/utils/deskScanAlert';
import { isCheckoutDeclined } from '@/utils/toPickDesk';

/**
 * Desk checkout copy.
 * API codes use `errors.apiCode.*`. An unmapped body is the generic error,
 * never the English server sentence.
 */
export function deskCheckoutMessage(error: unknown, t: TFunction): string {
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
