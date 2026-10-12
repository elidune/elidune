import type { DeskCardLookup, PatronBlockReason, PatronCardLookup, PatronLoanStatus, User } from '@/types';
import type { BorrowerRef } from '@/utils/readerIdentifier';

export function isDeskCardLookup(body: PatronCardLookup): body is DeskCardLookup {
  return !('firstname' in body) && !('login' in body) && !('email' in body);
}

function clean(value: string | null | undefined): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

/**
 * Reader shown after a card scan.
 * A name is kept when the payload has one (users read).
 * Otherwise the card is `cardNumber`, then `barcode`, then server initials with dots removed.
 */
export function deskPatronIdentity(input: {
  id: string;
  cardNumber?: string | null;
  barcode?: string | null;
  initials?: string | null;
  firstname?: string | null;
  lastname?: string | null;
  accountType?: string | null;
  email?: string | null;
  phone?: string | null;
  login?: string | null;
}): BorrowerRef {
  const firstname = clean(input.firstname);
  const lastname = clean(input.lastname);
  if (firstname || lastname) {
    return {
      id: input.id,
      firstname,
      lastname,
      barcode: clean(input.barcode),
      cardNumber: clean(input.cardNumber),
      accountType: clean(input.accountType),
      email: clean(input.email),
      phone: clean(input.phone),
      login: clean(input.login),
    };
  }
  return {
    id: input.id,
    cardNumber: clean(input.cardNumber) ?? clean(input.barcode),
    initials: clean(input.initials)?.replace(/\./g, '') ?? '',
  };
}

function asDeskUser(user: User): User {
  return {
    ...user,
    username: user.username || user.login || user.id,
  };
}

/** Split a `GET /users/by-card/:cardNumber` body into the loan user id and the visible identity. */
export function patronFromCardScan(body: PatronCardLookup): {
  user: User;
  identity: BorrowerRef;
  loanStatus: PatronLoanStatus;
} {
  if (!isDeskCardLookup(body)) {
    const { loanStatus, ...user } = body;
    const full = asDeskUser(user);
    return {
      user: full,
      identity: deskPatronIdentity({
        id: full.id,
        barcode: full.barcode,
        firstname: full.firstname,
        lastname: full.lastname,
        accountType: full.accountType,
        email: full.email,
        phone: full.phone,
        login: full.login,
      }),
      loanStatus,
    };
  }
  return {
    loanStatus: body.loanStatus,
    identity: deskPatronIdentity({
      id: body.id,
      cardNumber: body.cardNumber,
      initials: body.initials,
    }),
    user: {
      id: body.id,
      username: body.cardNumber ?? body.id,
    },
  };
}

/** `overdue_loans` is reported on the card scan and does not block checkout. */
export function checkoutBlockingReasons(reasons: readonly PatronBlockReason[]): PatronBlockReason[] {
  return reasons.filter((code) => code !== 'overdue_loans');
}

/** Name search is `GET /users?name=` and returns 403 without users read. */
export function deskNameSearchVisible(canReadUsersRight: boolean): boolean {
  return canReadUsersRight;
}

/**
 * Card-scan failure copy. Shown once, inline. A toast would duplicate a 403.
 * 404 is a neutral unknown card, without echoing the scanned value.
 */
export function presentCardScanError(
  status: number | null,
  translated: string,
  unknownCard: string,
): { message: string; toast: false } {
  return {
    message: status === 404 ? unknownCard : translated,
    toast: false,
  };
}
