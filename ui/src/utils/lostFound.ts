import type { LostFineAction, LostFineDeskStatus, LostFoundDisposition, LostReturnFields } from '@/types';
import { formatEuroAmount } from '@/utils/patronFines';

/** Dispositions accepted by `POST /loans/items/{itemId}/lost-found`. */
export const LOST_FOUND_DISPOSITIONS = ['circulate', 'damaged', 'leave'] as const satisfies readonly LostFoundDisposition[];

export type Translate = (key: string, values?: Record<string, unknown>) => string;

export interface LostFoundErrorInput {
  status: number | null;
  code: string | null;
  message: string | null;
}

export type LostFoundErrorRef =
  | { kind: 'alreadyProcessed' }
  | { kind: 'invalidAction' }
  | { kind: 'key'; key: string }
  | { kind: 'apiCode'; code: string };

/** Outcome fields the desk toast needs. Amounts are cents. */
export interface LostFoundOutcome {
  disposition: LostFoundDisposition;
  fineAction?: LostFineAction | null;
  refundedAmount?: number | null;
  cancelledAmount?: number | null;
  hold?: unknown | null;
}

export function isLostReturn(body: { wasLost?: boolean | null } | null | undefined): boolean {
  return body?.wasLost === true;
}

export function itemIdFromLostReturn(
  body: { loan?: { itemId?: string | null } | null },
  fallbackItemId?: string | null,
): string | null {
  const fromLoan = body.loan?.itemId?.trim();
  if (fromLoan) return fromLoan;
  const fallback = fallbackItemId?.trim();
  return fallback || null;
}

export function itemIdFromCopies(
  copies: Array<{ id?: string | null; barcode?: string | null }> | null | undefined,
  barcode?: string | null,
): string | null {
  const list = copies ?? [];
  const wanted = barcode?.trim();
  if (wanted) {
    const match = list.find((copy) => (copy.barcode ?? '').trim() === wanted && copy.id);
    if (match?.id) return match.id;
  }
  return list.find((copy) => copy.id)?.id ?? null;
}

/** Format a cent amount as euros. Display goes through `formatEuroAmount`. */
export function formatCentsAsEuros(cents: number, locale: string): string {
  const amount = Number.isFinite(cents) ? cents / 100 : 0;
  return formatEuroAmount(amount, locale);
}

function cents(value: number | null | undefined): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 0;
  return Math.round(value);
}

export function lostFeeKind(fields: LostReturnFields): LostFineDeskStatus | 'none' {
  if (fields.fineStatus === 'unpaid' || fields.fineStatus === 'paid' || fields.fineStatus === 'partial') {
    return fields.fineStatus;
  }
  return 'none';
}

export interface LostFeeAmounts {
  /** Replacement charge, cents. */
  charge?: number | null;
  /** Amount already collected, or the amount refunded, cents. */
  paid?: number | null;
  /** Unpaid remainder, or the amount cancelled, cents. */
  rest?: number | null;
}

function euroOrNull(value: number | null | undefined, locale: string): string | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return formatCentsAsEuros(value, locale);
}

/**
 * What happens to the reader's money for every action except damaged.
 * Amounts are included only when the API sent them (integer cents).
 */
export function lostFoundMoneyMessage(
  kind: LostFineDeskStatus | 'none',
  amounts: LostFeeAmounts,
  locale: string,
  t: Translate,
): string {
  if (kind === 'none') return t('loans.lostFound.feeNone');
  if (kind === 'unpaid') {
    const amount = euroOrNull(amounts.charge ?? amounts.rest, locale);
    return amount
      ? t('loans.lostFound.feeUnpaid', { amount })
      : t('loans.lostFound.feeUnpaidPlain');
  }
  if (kind === 'paid') {
    const amount = euroOrNull(amounts.paid ?? amounts.charge, locale);
    return amount ? t('loans.lostFound.feePaid', { amount }) : t('loans.lostFound.feePaidPlain');
  }
  const paid = euroOrNull(amounts.paid, locale);
  const rest = euroOrNull(amounts.rest, locale);
  if (paid && rest) return t('loans.lostFound.feePartial', { paid, rest });
  return t('loans.lostFound.feePartialPlain');
}

/** Money sentence for a lost return, before a disposition is chosen. */
export function lostFoundFeeMessage(fields: LostReturnFields, locale: string, t: Translate): string {
  const kind = lostFeeKind(fields);
  const charge = fields.fineAmount;
  const paid = fields.paidAmount;
  const rest =
    typeof charge === 'number' && typeof paid === 'number'
      ? Math.max(0, cents(charge) - cents(paid))
      : null;
  return lostFoundMoneyMessage(kind, { charge, paid, rest }, locale, t);
}

function notMarkedLost(message: string | null): boolean {
  return message?.toLowerCase().includes('not marked lost') === true;
}

/** Second resolve: the copy is no longer lost, so another refund is refused (422). */
export function isLostFoundAlreadyProcessed(input: LostFoundErrorInput): boolean {
  return notMarkedLost(input.message);
}

/** Unknown disposition. The server may answer 400 or 422; both are the same desk case. */
export function isLostFoundInvalidAction(input: LostFoundErrorInput): boolean {
  if (isLostFoundAlreadyProcessed(input)) return false;
  return input.status === 400 || input.status === 422;
}

/**
 * Map a lost-found API failure to a translation key.
 * Raw English server text is never shown.
 */
export function classifyLostFoundError(input: LostFoundErrorInput): LostFoundErrorRef {
  if (input.code === 'authentication_failed') {
    return { kind: 'key', key: 'errors.apiCode.authentication_failed' };
  }
  if (input.status === 403 || input.code === 'authorization_failed') {
    return { kind: 'key', key: 'loans.lostFound.errorForbidden' };
  }
  if (isLostFoundAlreadyProcessed(input)) {
    return { kind: 'alreadyProcessed' };
  }
  if (isLostFoundInvalidAction(input)) {
    return { kind: 'invalidAction' };
  }
  if (input.status === 404 || input.code === 'not_found') {
    return { kind: 'key', key: 'loans.lostFound.errorNotFound' };
  }
  if (input.status == null && input.message === 'Network Error') {
    return { kind: 'key', key: 'errors.network' };
  }
  if (input.code) return { kind: 'apiCode', code: input.code };
  return { kind: 'key', key: 'loans.lostFound.errorGeneric' };
}

export function lostFoundErrorMessage(input: LostFoundErrorInput, t: Translate): string {
  const ref = classifyLostFoundError(input);
  if (ref.kind === 'alreadyProcessed') return t('loans.lostFound.alreadyProcessed');
  if (ref.kind === 'invalidAction') return t('loans.lostFound.invalidAction');
  if (ref.kind === 'apiCode') {
    const key = `errors.apiCode.${ref.code}`;
    const translated = t(key);
    if (translated !== key) return translated;
    return t('loans.lostFound.errorGeneric');
  }
  return t(ref.key);
}

/**
 * Only the damaged disposition uses this sentence.
 * Circulate, leave, and dismiss describe the charge instead.
 */
export const FEES_UNCHANGED_KEY = 'loans.lostFound.feesUnchanged';

export function feesUnchangedLabel(t: Translate): string {
  return t(FEES_UNCHANGED_KEY);
}

function joinSentences(lead: string, tail: string): string {
  const left = lead.trim();
  const right = tail.trim();
  if (!left) return right;
  if (!right || left.includes(right)) return left;
  return `${left} ${right}`;
}

/** Hint under one desk choice. Damaged is the only line that says fees stay as they are. */
export function lostFoundChoiceHint(
  disposition: LostFoundDisposition,
  fields: LostReturnFields,
  locale: string,
  t: Translate,
): string {
  if (disposition === 'damaged') {
    return joinSentences(t('loans.lostFound.damagedHint'), feesUnchangedLabel(t));
  }
  const hint =
    disposition === 'leave' ? t('loans.lostFound.leaveHint') : t('loans.lostFound.circulateHint');
  return joinSentences(hint, lostFoundFeeMessage(fields, locale, t));
}

export type LostFoundPanelAction = LostFoundDisposition | 'dismissed' | null;

/** Label for the return-summary “action taken” row. `null` means the desk has not chosen yet. */
export function lostFoundActionLabel(action: LostFoundPanelAction, t: Translate): string {
  if (action === 'circulate') return t('loans.lostFound.circulate');
  if (action === 'damaged') return t('loans.lostFound.damaged');
  if (action === 'leave') return t('loans.lostFound.leave');
  if (action === 'dismissed') return t('loans.lostFound.summaryActionDismissed');
  return t('loans.lostFound.summaryActionPending');
}

/**
 * Fee line for the alert and the return summary.
 * Damaged always uses the single unchanged-fees label.
 * Every other action states the charge: cancelled, refunded, or partial.
 */
export function lostFoundPanelFee(
  action: LostFoundPanelAction,
  fields: LostReturnFields,
  outcome: LostFoundOutcome | null,
  locale: string,
  t: Translate,
): string {
  if (action === 'damaged') return feesUnchangedLabel(t);
  if (action === 'circulate' && outcome) {
    return feeDoneMessage(outcome, locale, t) ?? t('loans.lostFound.feeNone');
  }
  return lostFoundFeeMessage(fields, locale, t);
}

function feeDoneMessage(outcome: LostFoundOutcome, locale: string, t: Translate): string | null {
  const action = outcome.fineAction ?? 'none';
  if (action === 'none') return null;
  if (action === 'cancelled') {
    return lostFoundMoneyMessage('unpaid', { charge: outcome.cancelledAmount, rest: outcome.cancelledAmount }, locale, t);
  }
  if (action === 'refunded') {
    return lostFoundMoneyMessage('paid', { paid: outcome.refundedAmount, charge: outcome.refundedAmount }, locale, t);
  }
  if (action === 'partial') {
    return lostFoundMoneyMessage(
      'partial',
      { paid: outcome.refundedAmount, rest: outcome.cancelledAmount },
      locale,
      t,
    );
  }
  return null;
}

/** Sentences for the toast after a successful lost-found call. */
export function lostFoundResolutionMessage(outcome: LostFoundOutcome, locale: string, t: Translate): string {
  const parts: string[] = [];
  if (outcome.disposition === 'damaged') {
    parts.push(joinSentences(t('loans.lostFound.doneDamaged'), feesUnchangedLabel(t)));
    return parts.join(' ');
  }
  if (outcome.disposition === 'leave') {
    parts.push(t('loans.lostFound.doneLeave'));
    return parts.join(' ');
  }
  parts.push(t('loans.lostFound.doneCirculate'));
  const fee = feeDoneMessage(outcome, locale, t);
  if (fee) parts.push(fee);
  if (outcome.disposition === 'circulate' && outcome.hold != null) {
    parts.push(t('loans.lostFound.holdReady', { count: 1 }));
  }
  return parts.join(' ');
}
