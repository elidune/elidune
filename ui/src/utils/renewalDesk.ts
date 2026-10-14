import type { TFunction } from 'i18next';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';

const ITEM_HELD_FOR_OTHER = 'item_held_for_other';

/**
 * What the desk shows for one renewal refusal.
 * `item_held_for_other` uses the checkout set-aside popup.
 * Force is offered only when the body says `forceable: true`.
 * Reason codes and account type are not consulted.
 */
export type RenewalDeskView =
  | { kind: 'setAside'; message: string; force: boolean }
  | { kind: 'force'; message: string }
  | { kind: 'sentence'; message: string };

function responseForceable(error: unknown): unknown {
  const data = (error as { response?: { data?: unknown } })?.response?.data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) return undefined;
  return (data as { forceable?: unknown }).forceable;
}

/** Localized refusal. Identity fields on the body are never read. */
export function renewalDeskMessage(error: unknown, t: TFunction): string {
  if (getApiErrorCode(error) === ITEM_HELD_FOR_OTHER) {
    return t('errors.apiCode.item_held_for_other');
  }
  return getApiErrorMessage(error, t) || t('loans.errorRenewingLoan');
}

/**
 * Same shape as checkout (`presentCheckoutRefusal`): a held-copy popup, a force
 * confirmation, or the refusal sentence. Force follows `forceable: true` alone.
 * A missing right comes back as `forceable: false`, or as a 403 on the forced call.
 */
export function presentRenewalRefusal(
  error: unknown,
  options: { alreadyForced: boolean; t: TFunction },
): RenewalDeskView {
  const code = getApiErrorCode(error);
  const force = !options.alreadyForced && responseForceable(error) === true;
  if (code === ITEM_HELD_FOR_OTHER) {
    return {
      kind: 'setAside',
      message: options.t('errors.apiCode.item_held_for_other'),
      force,
    };
  }
  const message = renewalDeskMessage(error, options.t);
  if (force) {
    return {
      kind: 'force',
      message: `${message}\n\n${options.t('loans.forceRenewConfirm')}`,
    };
  }
  return { kind: 'sentence', message };
}
