/** Where a staff « Forcer » action can appear. `card_lost` is never one of them. */
export type CirculationForceSurface = 'loan' | 'batch' | 'renewal';

/** Checkout and batch-create may retry these codes with `force`. `card_lost` is absent. */
const LOAN_FORCEABLE = new Set(['business_rule_violation']);

/**
 * Renewal `force` skips a blocked account, an expired subscription, and unpaid fines.
 * It does not skip `card_lost`, and it does not move a due date that would stay put.
 */
const RENEWAL_FORCEABLE = new Set([
  'account_blocked',
  'subscription_expired',
  'fines_over_threshold',
]);

/**
 * Whether the desk may offer a force retry.
 * `card_lost` is refused for every surface, including an administrator.
 */
export function circulationMayForce(
  surface: CirculationForceSurface,
  code: string | null | undefined,
): boolean {
  if (!code || code === 'card_lost') return false;
  if (surface === 'renewal') return RENEWAL_FORCEABLE.has(code);
  return LOAN_FORCEABLE.has(code);
}
