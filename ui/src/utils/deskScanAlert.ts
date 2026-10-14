/**
 * Non-blocking desk notice when a scanned copy is on loan to another reader
 * or on hold. Built from `GET /items/barcode/:barcode` (`availability`,
 * `holdFor`) and `GET /loans/:id/user`, or from a check-in body.
 */

import { isMinimizedBorrower, type BorrowerRef } from '@/utils/readerIdentifier';

export type DeskScanAlertKind = 'onLoan' | 'onHold' | 'alreadyOnLoan';

/**
 * Another borrower, safe to show on the public desk.
 * The card number is preferred. Initials are the fallback. The full name is not stored.
 */
export interface OtherBorrowerRef {
  userId: string | null;
  cardNumber: string | null;
  initials: string | null;
}

export interface DeskScanAlert {
  kind: DeskScanAlertKind;
  patronName: string | null;
  /** Set for a checkout conflict about another borrower. Never contains the full name. */
  otherBorrower?: OtherBorrowerRef | null;
}

export interface DeskCopySnapshot {
  availability?: string | null;
  loanId?: string | null;
  loanUserId?: string | null;
  loanUserName?: string | null;
  /** Library card: `cardNumber` when the loan user payload has it, otherwise `barcode`. */
  loanUserCard?: string | null;
  /** Server initials when the loan user payload sends them. No dots. */
  loanUserInitials?: string | null;
  loanUserFirstname?: string | null;
  loanUserLastname?: string | null;
  /** Full staff profile, or `{ id, cardNumber, initials }`. */
  holdFor?: BorrowerRef | null;
}

export interface DeskScanPatron {
  patronId?: string | null;
  /**
   * Users read right. When false, a name on the copy is dropped.
   * Card number and server initials stay. Omitted means the payload is shown as-is.
   */
  revealIdentity?: boolean;
}

/** Exact checkout messages that mean the copy is with another reader or on hold. */
export const CHECKOUT_ALREADY_BORROWED = 'Item is already borrowed';
export const CHECKOUT_HOLD_FOR_OTHER =
  'This copy has an active hold for another patron — only the queued patron may borrow it, or use force=true to override';

function clean(value: string | null | undefined): string | null {
  const text = value?.trim();
  return text ? text : null;
}

function firstLetter(value: string | null | undefined): string | null {
  const text = value?.trim();
  if (!text) return null;
  const char = [...text][0];
  return char ? char.toLocaleUpperCase() : null;
}

/** One or two initials. A full name is never returned. */
export function borrowerInitials(
  person:
    | { firstname?: string | null; lastname?: string | null }
    | string
    | null
    | undefined,
): string | null {
  if (typeof person === 'string') {
    const letters = person
      .trim()
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => firstLetter(part))
      .filter((letter): letter is string => Boolean(letter));
    return letters.join('') || null;
  }
  if (!person) return null;
  const letters = [firstLetter(person.firstname), firstLetter(person.lastname)].filter(
    (letter): letter is string => Boolean(letter),
  );
  return letters.join('') || null;
}

/** Uppercase initials with dots and spaces removed. "JD" stays "JD". */
function serverInitials(value: string | null | undefined): string | null {
  const text = value?.trim();
  if (!text) return null;
  const compact = [...text].filter((char) => char !== '.' && !/\s/u.test(char)).join('');
  if (!compact) return null;
  return compact.toLocaleUpperCase();
}

/**
 * Desk identifier for another borrower.
 * Card is `cardNumber`, or `barcode` when `cardNumber` is absent.
 * A card is shown as-is. Otherwise server `initials` are used. Names are only
 * a last resort for deriving initials, and the full name is never stored.
 */
export function otherBorrowerFromPerson(input: {
  userId?: string | null;
  cardNumber?: string | null;
  barcode?: string | null;
  initials?: string | null;
  firstname?: string | null;
  lastname?: string | null;
  fullName?: string | null;
}): OtherBorrowerRef | null {
  const userId = clean(input.userId);
  const cardNumber = clean(input.cardNumber) ?? clean(input.barcode);
  const initials =
    serverInitials(input.initials) ??
    borrowerInitials({ firstname: input.firstname, lastname: input.lastname }) ??
    borrowerInitials(input.fullName);
  if (!userId && !cardNumber && !initials) return null;
  return { userId, cardNumber, initials };
}

function payloadText(value: unknown): string | null {
  if (typeof value === 'number' && Number.isSafeInteger(value)) return String(value);
  return clean(typeof value === 'string' ? value : null);
}

function payloadRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

/**
 * Borrower object from `GET /loans/:id/user` or a 409 body.
 * Minimized: `{ id, cardNumber, initials }`. Legacy: `barcode`, `firstname`, `lastname`.
 */
export function otherBorrowerFromPayload(payload: unknown): OtherBorrowerRef | null {
  const source = payloadRecord(payload);
  if (!source) return null;
  const userId = payloadText(source.id) ?? payloadText(source.userId);
  const cardNumber = payloadText(source.cardNumber);
  const barcode = payloadText(source.barcode);
  const initials = payloadText(source.initials);
  const firstname = payloadText(source.firstname);
  const lastname = payloadText(source.lastname);
  if (!cardNumber && !barcode && !initials && !firstname && !lastname) return null;
  return otherBorrowerFromPerson({
    userId,
    cardNumber,
    barcode,
    initials,
    firstname,
    lastname,
  });
}

/** Identifier shown for a checkout conflict. The full name is never the label. */
export function checkoutOnLoanCopy(alert: DeskScanAlert): {
  label: string;
  userId: string | null;
  kind: 'card' | 'initials';
} | null {
  const card = clean(alert.otherBorrower?.cardNumber);
  const userId = clean(alert.otherBorrower?.userId);
  if (card) return { label: card, userId, kind: 'card' };
  const initials = clean(alert.otherBorrower?.initials) ?? borrowerInitials(alert.patronName);
  if (!initials) return null;
  return { label: initials, userId, kind: 'initials' };
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
  const reveal = context.revealIdentity !== false;
  if (copy.availability === 'onLoan' && !samePatron(patronId, copy.loanUserId)) {
    if (!reveal) {
      return {
        kind: 'onLoan',
        patronName: null,
        otherBorrower: otherBorrowerFromPerson({
          userId: copy.loanUserId,
          cardNumber: copy.loanUserCard,
          initials: copy.loanUserInitials,
        }),
      };
    }
    return { kind: 'onLoan', patronName: clean(copy.loanUserName) };
  }
  if (copy.availability === 'onHold' && !samePatron(patronId, copy.holdFor?.id)) {
    const holdFor = copy.holdFor;
    if (holdFor && !isMinimizedBorrower(holdFor) && reveal) {
      return { kind: 'onHold', patronName: readerName(holdFor) };
    }
    return {
      kind: 'onHold',
      patronName: null,
      otherBorrower: otherBorrowerFromPerson({
        userId: holdFor?.id,
        cardNumber: holdFor?.cardNumber,
        barcode: holdFor?.barcode,
        initials: holdFor?.initials,
      }),
    };
  }
  return null;
}

export function readerName(
  person:
    | {
        firstname?: string | null;
        lastname?: string | null;
        initials?: string | null;
        cardNumber?: string | null;
      }
    | null
    | undefined,
  revealIdentity = true,
): string | null {
  if (!person) return null;
  if (!revealIdentity) {
    const card = typeof person.cardNumber === 'string' ? person.cardNumber.trim() : '';
    if (card) return card;
    const initials = typeof person.initials === 'string' ? person.initials.trim().replace(/\./g, '') : '';
    return initials || null;
  }
  const name = [person.firstname, person.lastname]
    .map((part) => (typeof part === 'string' ? part.trim() : ''))
    .filter(Boolean)
    .join(' ');
  if (name) return name;
  const initials = typeof person.initials === 'string' ? person.initials.trim() : '';
  if (initials) return initials;
  const card = typeof person.cardNumber === 'string' ? person.cardNumber.trim() : '';
  return card || null;
}

export interface CheckInScanBody {
  loan?: {
    user?: BorrowerRef | null;
  } | null;
  hold?: {
    userId?: string | null;
    user?: BorrowerRef | null;
  } | null;
}

/** Name when the payload is a full profile. Card or initials when it is minimized. */
function holdAlertFromUser(
  user: BorrowerRef | null | undefined,
  revealIdentity = true,
): DeskScanAlert {
  if (!revealIdentity || (user && isMinimizedBorrower(user))) {
    return {
      kind: 'onHold',
      patronName: null,
      otherBorrower: otherBorrowerFromPerson({
        userId: user?.id,
        cardNumber: user?.cardNumber,
        barcode: user?.barcode,
        initials: user?.initials,
      }),
    };
  }
  return { kind: 'onHold', patronName: readerName(user) };
}

/**
 * Notice after a return succeeded.
 * The loan in the body is the one just closed, so the pre-return “on loan” warning is stale.
 * A hold that the return readied is still worth showing.
 */
export function deskScanAlertAfterReturn(
  body: CheckInScanBody | null | undefined,
  revealIdentity = true,
): DeskScanAlert | null {
  if (!body?.hold) return null;
  return holdAlertFromUser(body.hold.user, revealIdentity);
}

/** Drop the checkout “still on loan” warning for the copy that was just returned. */
export function checkoutAlertAfterSuccessfulReturn<
  T extends { kind: string; barcode: string; loanId?: string | null },
>(alert: T | null, returned: { barcode?: string | null; loanId?: string | null }): T | null {
  if (!alert) return null;
  if (alert.kind !== 'onLoan' && alert.kind !== 'alreadyOnLoan') return alert;
  const barcode = returned.barcode?.trim();
  const loanId = returned.loanId?.trim();
  if (barcode && alert.barcode === barcode) return null;
  if (loanId && alert.loanId && alert.loanId === loanId) return null;
  return alert;
}

/** Notice from a check-in body when the barcode lookup did not run. */
export function deskScanAlertFromCheckIn(
  body: CheckInScanBody | null | undefined,
  revealIdentity = true,
): DeskScanAlert | null {
  if (!body) return null;
  if (body.loan) {
    if (!revealIdentity) {
      return {
        kind: 'onLoan',
        patronName: null,
        otherBorrower: otherBorrowerFromPerson({
          userId: body.loan.user?.id,
          cardNumber: body.loan.user?.cardNumber,
          barcode: body.loan.user?.barcode,
          initials: body.loan.user?.initials,
        }),
      };
    }
    return { kind: 'onLoan', patronName: readerName(body.loan.user) };
  }
  if (body.hold) return holdAlertFromUser(body.hold.user, revealIdentity);
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

/** API code for a copy on the pick list of another reader. */
const ITEM_TO_PICK_FOR_OTHER = 'item_to_pick_for_other';

/**
 * Specific checkout notice for a refusal, or null when the desk should show the generic error.
 * A non-null result replaces the business-rule banner and the error toast.
 */
export function checkoutScanNoticeFromRefusal(input: {
  code?: string | null;
  message?: string | null;
  patronName?: string | null;
  cardNumber?: string | null;
  initials?: string | null;
}): DeskScanAlert | null {
  // The set-aside dialog (« Mettre de côté » / « Annuler ») handles this code.
  if (input.code?.trim() === 'item_held_for_other') return null;
  if (input.code?.trim() === ITEM_TO_PICK_FOR_OTHER) {
    return {
      kind: 'onHold',
      patronName: null,
      otherBorrower: otherBorrowerFromPerson({
        cardNumber: input.cardNumber,
        initials: input.initials,
      }),
    };
  }
  const fromMessage = deskScanAlertFromCheckoutMessage(input.message);
  if (!fromMessage) return null;
  const name = clean(input.patronName);
  if (name && !fromMessage.patronName) return { ...fromMessage, patronName: name };
  return fromMessage;
}

/** Thrown after the yellow scan notice is shown, so the form does not also toast the generic error. */
export class CheckoutScanNoticeShown extends Error {
  constructor() {
    super('checkout-scan-notice');
    this.name = 'CheckoutScanNoticeShown';
  }
}

export function isCheckoutScanNoticeShown(error: unknown): boolean {
  return error instanceof CheckoutScanNoticeShown;
}

type Translate = (key: string, values?: Record<string, unknown>) => string;

/** Where the notice is shown. Checkout blocks the loan; return only informs. */
export type DeskScanPlace = 'checkout' | 'return';

/**
 * The notice never lends the copy.
 * On checkout, an on-loan copy can switch to the return screen. An on-hold copy has no action.
 */
export function deskScanNoticeAction(
  alert: DeskScanAlert,
  place: DeskScanPlace = 'checkout',
): 'return' | 'renew' | null {
  if (place !== 'checkout') return null;
  if (alert.kind === 'onLoan') return 'return';
  if (alert.kind === 'alreadyOnLoan') return 'renew';
  return null;
}

export function deskScanAlertMessage(
  alert: DeskScanAlert,
  t: Translate,
  place: DeskScanPlace = 'checkout',
): string {
  if (alert.kind === 'alreadyOnLoan') {
    return t('errors.apiCode.item_already_on_loan_to_borrower');
  }
  if (alert.kind === 'onLoan') {
    if (place === 'return') {
      return alert.patronName
        ? t('loans.scanAlertSeenOnLoan', { name: alert.patronName })
        : t('loans.scanAlertSeenOnLoanUnknown');
    }
    const copy = checkoutOnLoanCopy(alert);
    if (!copy) return t('loans.scanAlertOnLoanUnknown');
    const key = copy.kind === 'card' ? 'loans.scanAlertOnLoanCard' : 'loans.scanAlertOnLoan';
    return t(key, { identifier: copy.label });
  }
  if (alert.otherBorrower) {
    const copy = checkoutOnLoanCopy({ ...alert, patronName: null });
    if (copy) {
      const key = copy.kind === 'card' ? 'loans.scanAlertOnHoldCard' : 'loans.scanAlertOnHoldInitials';
      return t(key, { identifier: copy.label });
    }
  }
  return alert.patronName
    ? t('loans.scanAlertOnHold', { name: alert.patronName })
    : t('loans.scanAlertOnHoldUnknown');
}
