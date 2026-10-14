import type {
  AudienceUnpaidThresholdMode,
  CirculationFinePolicy,
  FineRule,
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

/** Display string for the global amount, only when that mode is `amount`. Euro decimal, not a currency symbol. */
export function globalThresholdLabel(policy: CirculationFinePolicy | undefined): string | null {
  if (!policy || policy.unpaidThresholdMode !== 'amount' || policy.unpaidThresholdCents == null) {
    return null;
  }
  const label = centsToInput(policy.unpaidThresholdCents);
  return label === '' ? null : label;
}

export function isDefaultFineRule(rule: { mediaType?: string | null }): boolean {
  return rule.mediaType == null || rule.mediaType === '';
}

/** PUT `/fines/rules` for the default (no media type) overdue rule. */
export type UpsertFineRuleBody = {
  mediaType: null;
  dailyRate: string;
  maxAmount: string | null;
  graceDays: number;
};

export type FineSettingsDraft = {
  /** `reminders.accrue_fines`. Late-fine accrual, not lost/damage billing. */
  enabled: boolean;
  dailyRate: string;
  blockMode: GlobalUnpaidThresholdMode;
  threshold: string;
  graceDays: number | null;
  /** Euro decimal of the stored cap, when the default rule has one. */
  maxAmount: string | null;
  hasMediaRules: boolean;
};

function graceDaysOf(rule: FineRule | undefined): number {
  const days = rule?.graceDays;
  return typeof days === 'number' && Number.isInteger(days) && days >= 0 ? days : 0;
}

function preservedMaxAmount(rule: FineRule | undefined): string | null {
  if (rule?.maxAmount == null || rule.maxAmount === '') return null;
  const parsed = parseMoneyAmountInput(String(rule.maxAmount), false);
  if (!parsed.ok || parsed.value == null) return null;
  return parsed.value;
}

/** Draft for Settings → Loans. `accrueFines` is the on/off flag; the rate comes from the default rule. */
export function fineSettingsDraft(
  policy: CirculationFinePolicy,
  accrueFines: boolean,
  rules: FineRule[],
): FineSettingsDraft {
  const rule = rules.find(isDefaultFineRule);
  return {
    enabled: accrueFines,
    dailyRate: moneyAmountToInput(rule?.dailyRate),
    blockMode: policy.unpaidThresholdMode,
    threshold: centsToInput(policy.unpaidThresholdCents),
    graceDays: rule ? graceDaysOf(rule) : null,
    maxAmount: preservedMaxAmount(rule),
    hasMediaRules: rules.some((row) => !isDefaultFineRule(row)),
  };
}

export type FineSettingsSavePlan =
  | { ok: false; field: 'dailyRate' | 'threshold' }
  | {
      ok: true;
      policy: GlobalUnpaidThresholdBody;
      /**
       * `null` leaves `reminders.accrue_fines` unchanged (the caller cannot write admin config).
       * `false` also forces the global blocking mode to `none`.
       */
      accrueFines: boolean | null;
      rule: UpsertFineRuleBody | null;
    };

/**
 * Disabling fines stops scheduled late fines (`accrue_fines`) and the global blocking threshold.
 * The default daily rate stays stored so turning fines back on restores it.
 * Lost and damaged copies are staff charges and are not part of this payload.
 */
export function fineSettingsSavePlan(input: {
  enabled: boolean;
  canControlAccrual: boolean;
  dailyRate: string;
  blockMode: GlobalUnpaidThresholdMode;
  threshold: string;
  rules: FineRule[];
}): FineSettingsSavePlan {
  if (!input.enabled) {
    return {
      ok: true,
      policy: { unpaidThresholdMode: 'none' },
      accrueFines: input.canControlAccrual ? false : null,
      rule: null,
    };
  }
  const rate = parseMoneyAmountInput(input.dailyRate, false);
  if (!rate.ok || rate.value == null) return { ok: false, field: 'dailyRate' };
  const policy = globalUnpaidThresholdBody(input.blockMode, input.threshold);
  if (!policy.ok) return { ok: false, field: 'threshold' };
  const rule = input.rules.find(isDefaultFineRule);
  return {
    ok: true,
    policy: policy.body,
    accrueFines: input.canControlAccrual ? true : null,
    rule: {
      mediaType: null,
      dailyRate: rate.value,
      maxAmount: preservedMaxAmount(rule),
      graceDays: graceDaysOf(rule),
    },
  };
}
