/**
 * Non-blocking desk notice when a scanned copy is on loan to another reader
 * or on hold. Built from `GET /items/barcode/:barcode` (`availability`,
 * `holdUserName`) and `GET /loans/:id/user`, or from a check-in body.
 */

export type DeskScanAlertKind = 'onLoan' | 'onHold';

export interface DeskScanAlert {
  kind: DeskScanAlertKind;
  patronName: string | null;
}

export interface DeskCopySnapshot {
  availability?: string | null;
  loanUserId?: string | null;
  loanUserName?: string | null;
  holdUserId?: string | null;
  holdUserName?: string | null;
}

export interface DeskScanPatron {
  patronId?: string | null;
}

/** Exact checkout messages that mean the copy is with another reader or on hold. */
export const CHECKOUT_ALREADY_BORROWED = 'Item is already borrowed';
export const CHECKOUT_HOLD_FOR_OTHER =
  'This copy has an active hold for another patron — only the queued patron may borrow it, or use force=true to override';

function clean(value: string | null | undefined): string | null {
  const text = value?.trim();
  return text ? text : null;
}

function samePatron(deskPatronId: string | null, otherId: string | null | undefined): boolean {
  const other = clean(otherId);
  return Boolean(deskPatronId && other && deskPatronId === other);
}

/**
 * Alert when the copy is on loan or on hold for someone other than the reader
 * at the checkout desk. On return, omit `patronId`: any loan or hold is reported.
 * `onLoan` wins when both could be set; the server availability field is exclusive.
 */
export function deskScanAlert(
  copy: DeskCopySnapshot | null | undefined,
  context: DeskScanPatron = {},
): DeskScanAlert | null {
  if (!copy) return null;
  const patronId = clean(context.patronId);
  if (copy.availability === 'onLoan' && !samePatron(patronId, copy.loanUserId)) {
    return { kind: 'onLoan', patronName: clean(copy.loanUserName) };
  }
  if (copy.availability === 'onHold' && !samePatron(patronId, copy.holdUserId)) {
    return { kind: 'onHold', patronName: clean(copy.holdUserName) };
  }
  return null;
}

export function readerName(
  person: { firstname?: string | null; lastname?: string | null } | null | undefined,
): string | null {
  if (!person) return null;
  const name = [person.firstname, person.lastname]
    .map((part) => (typeof part === 'string' ? part.trim() : ''))
    .filter(Boolean)
    .join(' ');
  return name || null;
}

export interface CheckInScanBody {
  loan?: {
    user?: { id?: string | null; firstname?: string | null; lastname?: string | null } | null;
  } | null;
  hold?: {
    userId?: string | null;
    user?: { firstname?: string | null; lastname?: string | null } | null;
  } | null;
}

/** Notice from a check-in body when the barcode lookup did not run. */
export function deskScanAlertFromCheckIn(body: CheckInScanBody | null | undefined): DeskScanAlert | null {
  if (!body) return null;
  if (body.loan) {
    return { kind: 'onLoan', patronName: readerName(body.loan.user) };
  }
  if (body.hold) {
    return {
      kind: 'onHold',
      patronName: readerName(body.hold.user),
    };
  }
  return null;
}

/** Fallback when checkout is refused before a copy snapshot is available. */
export function deskScanAlertFromCheckoutMessage(message: string | null | undefined): DeskScanAlert | null {
  const text = message?.trim();
  if (!text) return null;
  if (text === CHECKOUT_ALREADY_BORROWED) return { kind: 'onLoan', patronName: null };
  if (text === CHECKOUT_HOLD_FOR_OTHER || text.includes('active hold for another patron')) {
    return { kind: 'onHold', patronName: null };
  }
  return null;
}

type Translate = (key: string, values?: Record<string, unknown>) => string;

export function deskScanAlertMessage(alert: DeskScanAlert, t: Translate): string {
  if (alert.kind === 'onLoan') {
    return alert.patronName
      ? t('loans.scanAlertOnLoan', { name: alert.patronName })
      : t('loans.scanAlertOnLoanUnknown');
  }
  return alert.patronName
    ? t('loans.scanAlertOnHold', { name: alert.patronName })
    : t('loans.scanAlertOnHoldUnknown');
}
