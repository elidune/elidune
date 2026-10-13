import type {
  AudienceUnpaidThresholdMode,
  CirculationFinePolicy,
  GlobalUnpaidThresholdMode,
} from '@/types';

/** Normalize a money amount from the API (string or number) for display/input. */
export function moneyAmountToInput(value: string | number | null | undefined): string {
  if (value == null || value === '') return '';
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value.toFixed(2) : '';
  }
  const trimmed = String(value).trim();
  if (!trimmed) return '';
  const n = Number(trimmed.replace(',', '.'));
  return Number.isFinite(n) ? n.toFixed(2) : trimmed;
}

export function centsToInput(cents: number | null | undefined): string {
  if (cents == null) return '';
  return moneyAmountToInput(cents / 100);
}

export type ParsedMoneyAmount =
  | { ok: true; value: string }
  | { ok: true; value: null }
  | { ok: false };

/**
 * Parse a locale-tolerant money field.
 * Empty → null (inherit / omit). Rejects negatives and non-numeric values.
 */
export function parseMoneyAmountInput(raw: string, allowEmpty: boolean): ParsedMoneyAmount {
  const trimmed = raw.trim().replace(',', '.');
  if (!trimmed) {
    return allowEmpty ? { ok: true, value: null } : { ok: false };
  }
  if (!/^\d+(\.\d{1,4})?$/.test(trimmed)) return { ok: false };
  const n = Number(trimmed);
  if (!Number.isFinite(n) || n < 0) return { ok: false };
  return { ok: true, value: n.toFixed(2) };
}

export function eurosInputToCents(raw: string): { ok: true; cents: number } | { ok: false } {
  const parsed = parseMoneyAmountInput(raw, false);
  if (!parsed.ok || parsed.value == null) return { ok: false };
  return { ok: true, cents: Math.round(Number(parsed.value) * 100) };
}

export function normalizeFinePolicy(data: unknown): CirculationFinePolicy {
  const o = data && typeof data === 'object' ? (data as Record<string, unknown>) : {};
  const mode = o.unpaidThresholdMode ?? o.unpaid_threshold_mode;
  const centsRaw = o.unpaidThresholdCents ?? o.unpaid_threshold_cents;
  const cents = typeof centsRaw === 'number' ? centsRaw : null;
  if (mode === 'amount') {
    return { unpaidThresholdMode: 'amount', unpaidThresholdCents: cents };
  }
  return { unpaidThresholdMode: 'none' };
}

export type GlobalUnpaidThresholdBody =
  | { unpaidThresholdMode: 'none' }
  | { unpaidThresholdMode: 'amount'; unpaidThresholdCents: number };

export type AudienceUnpaidThresholdBody =
  | { unpaidThresholdMode: 'inherit' | 'none' }
  | { unpaidThresholdMode: 'amount'; unpaidThresholdCents: number };

export type AudienceThresholdWrite =
  | { ok: true; body?: AudienceUnpaidThresholdBody }
  | { ok: false };

/**
 * Body for PUT /fines/policy.
 * Mode `amount` without a valid euro amount is refused locally (the API returns 422).
 * Mode `none` never sends cents or the legacy `unpaidFineThreshold` field.
 */
export function globalUnpaidThresholdBody(
  mode: GlobalUnpaidThresholdMode,
  euros: string,
): { ok: true; body: GlobalUnpaidThresholdBody } | { ok: false } {
  if (mode !== 'amount') return { ok: true, body: { unpaidThresholdMode: 'none' } };
  const parsed = eurosInputToCents(euros);
  if (!parsed.ok) return { ok: false };
  return {
    ok: true,
    body: { unpaidThresholdMode: 'amount', unpaidThresholdCents: parsed.cents },
  };
}

function audienceAmountBody(
  mode: AudienceUnpaidThresholdMode,
  euros: string,
): AudienceThresholdWrite {
  if (mode !== 'amount') return { ok: true, body: { unpaidThresholdMode: mode } };
  const parsed = eurosInputToCents(euros);
  if (!parsed.ok) return { ok: false };
  return {
    ok: true,
    body: { unpaidThresholdMode: 'amount', unpaidThresholdCents: parsed.cents },
  };
}

/** Create always sends a mode. `amount` includes integer cents; other modes omit cents. */
export function audienceUnpaidThresholdForCreate(
  mode: AudienceUnpaidThresholdMode,
  euros: string,
): AudienceThresholdWrite {
  return audienceAmountBody(mode, euros);
}

/**
 * PUT omits the mode when it is unchanged (the API leaves mode and cents alone).
 * Changing only the amount still sends `unpaidThresholdMode: "amount"` with the new cents.
 * Cents are never sent as null, and never sent for `inherit` or `none`.
 */
export function audienceUnpaidThresholdForUpdate(
  previous: { mode: AudienceUnpaidThresholdMode; cents?: number | null },
  mode: AudienceUnpaidThresholdMode,
  euros: string,
): AudienceThresholdWrite {
  if (mode !== 'amount') {
    if (previous.mode === mode) return { ok: true };
    return { ok: true, body: { unpaidThresholdMode: mode } };
  }
  const parsed = eurosInputToCents(euros);
  if (!parsed.ok) return { ok: false };
  if (previous.mode === 'amount' && previous.cents === parsed.cents) return { ok: true };
  return {
    ok: true,
    body: { unpaidThresholdMode: 'amount', unpaidThresholdCents: parsed.cents },
  };
}

/** Display string for the global amount, only when that mode is `amount`. */
export function globalThresholdLabel(policy: CirculationFinePolicy | undefined): string | null {
  if (!policy || policy.unpaidThresholdMode !== 'amount' || policy.unpaidThresholdCents == null) {
    return null;
  }
  const label = centsToInput(policy.unpaidThresholdCents);
  return label === '' ? null : label;
}
