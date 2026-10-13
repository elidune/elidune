import { getHttpStatus, isInvalidSessionError } from '@/utils/apiError';

/** Permission denial on GET /users/:id/loans. A dead session is not this case. */
export function isLoansForbidden(error: unknown): boolean {
  return getHttpStatus(error) === 403 && !isInvalidSessionError(error);
}

/** One retry for transient failures; a 403 will not succeed on retry. */
export function loansQueryRetry(failureCount: number, error: unknown): boolean {
  return !isLoansForbidden(error) && failureCount < 1;
}

/** GET /users/:id returns 403 once. A second attempt on mount is the same denial. */
export const userRecordQueryRetry = loansQueryRetry;

export type PatronLoansRead<T> =
  | { ok: true; page: T }
  | { ok: false; denied: boolean; error: unknown };

/** Never throws. Callers decide whether a denial is a notice or a load error. */
export async function readPatronLoans<T>(load: () => Promise<T>): Promise<PatronLoansRead<T>> {
  try {
    return { ok: true, page: await load() };
  } catch (error) {
    return { ok: false, denied: isLoansForbidden(error), error };
  }
}

/** Current loans. `archived` is omitted: that flag is history and 403s without users read. */
export function currentUserLoansQuery(page: number, perPage: number): {
  page: number;
  perPage: number;
} {
  return { page, perPage };
}

/**
 * Closed loans (`archived=true`). Null without users read, so the request is not sent.
 * The server answers that call with `authorization_failed`.
 */
export function archivedUserLoansQuery(
  page: number,
  perPage: number,
  canReadUsersRight: boolean,
): { archived: true; page: number; perPage: number } | null {
  if (!canReadUsersRight) return null;
  return { archived: true, page, perPage };
}

/** Past-loans tab. Loans read alone does not open history. */
export function loanHistoryVisible(canReadUsersRight: boolean, canReadLoansRight: boolean): boolean {
  return canReadUsersRight && canReadLoansRight;
}

export function emptyLoanPage<T>(): {
  items: T[];
  total: number;
  page: number;
  perPage: number;
  pageCount: number;
} {
  return { items: [], total: 0, page: 1, perPage: 0, pageCount: 0 };
}

/**
 * Borrower shown on a current-loan row.
 * Users read keeps the payload. Otherwise only `id`, `cardNumber`, and `initials`.
 * A staff `barcode` is accepted as the card. Names are not turned into initials.
 */
export function deskBorrower(
  user: {
    id?: string | number | null;
    cardNumber?: string | null;
    barcode?: string | null;
    initials?: string | null;
    firstname?: string | null;
    lastname?: string | null;
    email?: string | null;
    phone?: string | null;
  } | null | undefined,
  revealIdentity: boolean,
): { id: string; cardNumber: string | null; initials: string } | typeof user {
  if (!user || revealIdentity) return user;
  const card = clean(user.cardNumber) ?? clean(user.barcode);
  const initials = clean(user.initials)?.replace(/\./g, '') ?? '';
  return { id: user.id == null ? '' : String(user.id), cardNumber: card, initials };
}

function clean(value: string | null | undefined): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

export interface CheckoutListMatch {
  id: string;
  biblio?: { title?: string | null } | null;
  itemIdentification?: string | null;
  expiryAt?: string | null;
}

/** Recap from the created loan when the patron loan list did not load. */
export function checkoutRecap(input: {
  created: { id: string; expiryAt: string };
  barcode: string;
  patronName: string;
  noTitle: string;
  listed: CheckoutListMatch | null;
}): { title: string; barcode: string; dueDate: string; patronName: string } {
  const title = input.listed?.biblio?.title?.trim() || input.noTitle;
  const barcode = input.listed?.itemIdentification?.trim() || input.barcode;
  const dueDate = input.listed?.expiryAt || input.created.expiryAt;
  return {
    title,
    barcode,
    dueDate,
    patronName: input.patronName,
  };
}
