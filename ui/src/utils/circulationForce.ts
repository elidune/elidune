/**
 * Renewal 422 bodies do not include `forceable`.
 * Staff `force` skips only these three. Checkout never consults this set:
 * a loan refusal is forceable only when the 422 says `forceable: true`.
 */
const RENEWAL_FORCE_CODES = new Set([
  'account_blocked',
  'subscription_expired',
  'fines_over_threshold',
]);

/** Whether a renewal refusal may be retried with staff `force`. */
export function renewalMayForce(code: string | null | undefined): boolean {
  const key = code?.trim();
  return Boolean(key && RENEWAL_FORCE_CODES.has(key));
}

/**
 * Loan refusal (`POST /loans`, including a batch row).
 * Only the server boolean counts. A missing field is not forceable.
 * Account type and role names are not consulted: the server already
 * computed the flag for this caller.
 * `canForce` is the loans-write right.
 */
export function loanRefusalMayForce(forceable: unknown, canForce: boolean): boolean {
  return canForce && forceable === true;
}
