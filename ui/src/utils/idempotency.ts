/** HTTP header name for desk mutation retries (checkout / renew). */
export const IDEMPOTENCY_HEADER = 'Idempotency-Key';

/** Stable key for one user action. Reuse on retry of the same action; generate a new one for a new action. */
export function newIdempotencyKey(): string {
  return crypto.randomUUID();
}
