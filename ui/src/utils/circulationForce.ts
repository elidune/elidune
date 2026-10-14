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
