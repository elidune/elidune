import { getApiErrorCode, getApiErrorRawMessage } from '@/utils/apiError';

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

const OVERRIDE_REASON_REQUIRED = 'override_reason_required';

function reasonRequiredFlag(source: unknown): unknown {
  if (!source || typeof source !== 'object' || Array.isArray(source)) return undefined;
  const record = source as Record<string, unknown>;
  if ('reasonRequired' in record) return record.reasonRequired;
  const response = record.response;
  if (!response || typeof response !== 'object' || Array.isArray(response)) return undefined;
  const data = (response as { data?: unknown }).data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) return undefined;
  return (data as { reasonRequired?: unknown }).reasonRequired;
}

/**
 * `reasonRequired` on a checkout or renewal 422.
 * Only boolean true counts. A missing field, false, or any other value does not.
 * The refusal code and the account type are not consulted.
 */
export function refusalReasonRequired(source: unknown): boolean {
  return reasonRequiredFlag(source) === true;
}

/** Trimmed staff reason, or undefined when missing or blank. */
export function trimmedOverrideReason(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim() ?? '';
  return trimmed.length > 0 ? trimmed : undefined;
}

/** The force confirm stays disabled until a required reason is non-blank. */
export function forceConfirmEnabled(reasonRequired: boolean, reason: string): boolean {
  if (!reasonRequired) return true;
  return trimmedOverrideReason(reason) !== undefined;
}

/**
 * Server `message` for 422 `override_reason_required`.
 * Null for every other error, so the force dialog stays open only for this code.
 */
export function overrideReasonRefusalMessage(error: unknown): string | null {
  if (getApiErrorCode(error) !== OVERRIDE_REASON_REQUIRED) return null;
  return getApiErrorRawMessage(error) ?? '';
}
