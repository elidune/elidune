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

/**
 * What the desk shows under “active loans” for the scanned reader.
 *
 * `GET /users/{id}/loans` for another reader requires users read and loans read
 * (#217). Loans write alone does not list those loans, and the minimized
 * `{id, cardNumber, initials}` body is only the caller’s own loans.
 * The reader record itself is users read, so a desk agent stays on the desk.
 */
export function patronActiveLoansPanel(canReadUsersRight: boolean): 'list' | 'desk' {
  return canReadUsersRight ? 'list' : 'desk';
}

/** Desk checkout may record a loan with loans write alone. Skip the list otherwise. */
export function deskMayLoadPatronLoans(canReadUsersRight: boolean): boolean {
  return patronActiveLoansPanel(canReadUsersRight) === 'list';
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
