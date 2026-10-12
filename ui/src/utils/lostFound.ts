import type { LostFineAction, LostFineDeskStatus, LostFoundDisposition, LostReturnFields } from '@/types';

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

/** Format a cent amount as euros for the active UI locale. */
export function formatCentsAsEuros(cents: number, locale: string): string {
  const amount = Number.isFinite(cents) ? cents / 100 : 0;
  return new Intl.NumberFormat(locale, { style: 'currency', currency: 'EUR' }).format(amount);
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

/** What circulate/damaged will do to the replacement charge. Leave does not use this. */
export function lostFoundFeeMessage(fields: LostReturnFields, locale: string, t: Translate): string {
  const kind = lostFeeKind(fields);
  const charge = formatCentsAsEuros(cents(fields.fineAmount), locale);
  const paid = formatCentsAsEuros(cents(fields.paidAmount), locale);
  const rest = formatCentsAsEuros(Math.max(0, cents(fields.fineAmount) - cents(fields.paidAmount)), locale);
  if (kind === 'unpaid') return t('loans.lostFound.feeUnpaid', { amount: charge });
  if (kind === 'paid') return t('loans.lostFound.feePaid', { amount: paid });
  if (kind === 'partial') return t('loans.lostFound.feePartial', { paid, rest });
  return t('loans.lostFound.feeNone');
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

function feeDoneMessage(outcome: LostFoundOutcome, locale: string, t: Translate): string | null {
  const action = outcome.fineAction ?? 'none';
  if (action === 'none') return null;
  const refunded = formatCentsAsEuros(cents(outcome.refundedAmount), locale);
  const cancelled = formatCentsAsEuros(cents(outcome.cancelledAmount), locale);
  if (action === 'cancelled') return t('loans.lostFound.doneCancelled', { amount: cancelled });
  if (action === 'refunded') return t('loans.lostFound.doneRefunded', { amount: refunded });
  if (action === 'partial') return t('loans.lostFound.donePartial', { paid: refunded, rest: cancelled });
  return null;
}

/** Sentences for the toast after a successful lost-found call. */
export function lostFoundResolutionMessage(outcome: LostFoundOutcome, locale: string, t: Translate): string {
  const parts: string[] = [];
  if (outcome.disposition === 'leave') {
    parts.push(t('loans.lostFound.doneLeave'));
    return parts.join(' ');
  }
  parts.push(
    outcome.disposition === 'damaged'
      ? t('loans.lostFound.doneDamaged')
      : t('loans.lostFound.doneCirculate'),
  );
  const fee = feeDoneMessage(outcome, locale, t);
  if (fee) parts.push(fee);
  if (outcome.disposition === 'circulate' && outcome.hold != null) {
    parts.push(t('loans.lostFound.holdReady', { count: 1 }));
  }
  return parts.join(' ');
}
