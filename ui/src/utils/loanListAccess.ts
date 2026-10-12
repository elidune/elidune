import { getHttpStatus, isInvalidSessionError } from '@/utils/apiError';

/** Permission denial on GET /users/:id/loans. A dead session is not this case. */
export function isLoansForbidden(error: unknown): boolean {
  return getHttpStatus(error) === 403 && !isInvalidSessionError(error);
}

/** One retry for transient failures; a 403 will not succeed on retry. */
export function loansQueryRetry(failureCount: number, error: unknown): boolean {
  return !isLoansForbidden(error) && failureCount < 1;
}

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
 * Desk checkout may record a loan with loans-write alone.
 * GET /users/:id/loans also requires users-read, so skip it otherwise.
 */
export function deskMayLoadPatronLoans(canReadUsersRight: boolean): boolean {
  return canReadUsersRight;
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
