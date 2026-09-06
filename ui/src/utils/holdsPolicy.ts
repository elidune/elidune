import type { HoldQuota, HoldsPolicy } from '@/types';

export const MIN_MAX_ACTIVE_HOLDS = 1;
export const MAX_MAX_ACTIVE_HOLDS = 1000;
export const DEFAULT_MAX_ACTIVE_HOLDS = 20;

export type ParsedHoldCap =
  | { ok: true; value: number }
  | { ok: true; value: null }
  | { ok: false };

/** Empty → null (inherit / omit). Must be an integer in 1–1000. */
export function parseHoldCapInput(raw: string, allowEmpty: boolean): ParsedHoldCap {
  const trimmed = raw.trim();
  if (!trimmed) {
    return allowEmpty ? { ok: true, value: null } : { ok: false };
  }
  if (!/^\d+$/.test(trimmed)) return { ok: false };
  const n = Number(trimmed);
  if (!Number.isInteger(n) || n < MIN_MAX_ACTIVE_HOLDS || n > MAX_MAX_ACTIVE_HOLDS) {
    return { ok: false };
  }
  return { ok: true, value: n };
}

export function holdCapToInput(value: number | string | null | undefined): string {
  if (value == null || value === '') return '';
  const n = typeof value === 'number' ? value : Number(value);
  return Number.isInteger(n) && n >= MIN_MAX_ACTIVE_HOLDS ? String(n) : '';
}

export function normalizeHoldsPolicy(data: unknown): HoldsPolicy {
  const o = data && typeof data === 'object' ? (data as Record<string, unknown>) : {};
  const raw = o.maxActiveHolds ?? o.max_active_holds;
  const parsed = parseHoldCapInput(String(raw ?? ''), false);
  return {
    maxActiveHolds: parsed.ok && parsed.value != null ? parsed.value : DEFAULT_MAX_ACTIVE_HOLDS,
  };
}

export function normalizeHoldQuota(data: unknown): HoldQuota {
  const o = data && typeof data === 'object' ? (data as Record<string, unknown>) : {};
  const userId = String(o.userId ?? o.user_id ?? '');
  const maxActiveHolds = Number(o.maxActiveHolds ?? o.max_active_holds ?? DEFAULT_MAX_ACTIVE_HOLDS);
  const activeHolds = Number(o.activeHolds ?? o.active_holds ?? 0);
  const remaining = Number(o.remaining ?? Math.max(0, maxActiveHolds - activeHolds));
  return { userId, maxActiveHolds, activeHolds, remaining };
}
