/** Where a staff « Forcer le prêt » action can appear. */
export type CirculationForceSurface = 'loan' | 'batch' | 'renewal';

/**
 * Single map from a 422 `code` to the desk surfaces staff `force` may retry.
 *
 * Checkout (`loan`) and batch loan share the six codes `POST /loans` returns
 * for rules staff force lifts: `subscription_expired`, `fines_over_threshold`,
 * `max_loans`, `max_media_loans`, `item_not_loanable`, `item_held_for_other`.
 *
 * Absent, and therefore not forceable:
 * - `business_rule_violation` (deleted or inactive account, withdrawn copy, copy in transit)
 * - `card_lost`
 * - `item_to_pick_for_other` (the to-pick dialog is separate)
 * - `hold_waiting` and the other renewal codes force does not skip
 *
 * Renewal keeps `account_blocked`, `subscription_expired`, and `fines_over_threshold`.
 */
export const CIRCULATION_FORCEABLE: Record<string, readonly CirculationForceSurface[]> = {
  subscription_expired: ['loan', 'batch', 'renewal'],
  fines_over_threshold: ['loan', 'batch', 'renewal'],
  max_loans: ['loan', 'batch'],
  max_media_loans: ['loan', 'batch'],
  item_not_loanable: ['loan', 'batch'],
  item_held_for_other: ['loan', 'batch'],
  account_blocked: ['renewal'],
};

/**
 * Whether the desk may offer a force retry for this code.
 * `canForce` is the caller's loans-write right. A forceable code without that right stays a message.
 */
export function circulationMayForce(
  surface: CirculationForceSurface,
  code: string | null | undefined,
  canForce = true,
): boolean {
  if (!canForce) return false;
  const key = code?.trim();
  if (!key) return false;
  return CIRCULATION_FORCEABLE[key]?.includes(surface) ?? false;
}
