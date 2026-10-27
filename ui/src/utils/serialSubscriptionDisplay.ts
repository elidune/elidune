import type { TFunction } from 'i18next';
import type {
  SerialFrequency,
  SerialIssue,
  SerialIssueStatus,
  SerialLengthKind,
  SerialSubscription,
  SerialSubscriptionStatus,
  SerialSubscriptionWrite,
} from '@/types';
import { getApiErrorRawMessage, getHttpStatus } from '@/utils/apiError';
import { formatCentsAsEuros } from '@/utils/lostFound';
import { serialFrequencyLabelKey } from '@/utils/serialFrequencyLabel';

export const CREATE_ITEMS_DEFAULT = true;

export interface SubscriptionFormValues {
  biblioId: string;
  biblioTitle: string;
  siteId: string;
  vendorId: string;
  fundId: string;
  costEuros: string;
  issuePriceEuros: string;
  frequencyId: string;
  numberingPatternId: string;
  firstIssueDate: string;
  endDate: string;
  x: string;
  y: string;
  z: string;
  innerX: string;
  innerY: string;
  innerZ: string;
  lengthKind: SerialLengthKind;
  lengthCount: string;
  graceDays: string;
  createItems: boolean;
  latestIssueLibraryUseOnly: boolean;
  irregularities: string;
  recurringSkips: string;
  callNumber: string;
  location: string;
  holdingsStatement: string;
  notes: string;
  status: SerialSubscriptionStatus;
}

export function emptySubscriptionForm(): SubscriptionFormValues {
  return {
    biblioId: '',
    biblioTitle: '',
    siteId: '',
    vendorId: '',
    fundId: '',
    costEuros: '',
    issuePriceEuros: '',
    frequencyId: '',
    numberingPatternId: '',
    firstIssueDate: '',
    endDate: '',
    x: '1',
    y: '0',
    z: '0',
    innerX: '0',
    innerY: '0',
    innerZ: '0',
    lengthKind: 'months',
    lengthCount: '12',
    graceDays: '',
    createItems: CREATE_ITEMS_DEFAULT,
    latestIssueLibraryUseOnly: true,
    irregularities: '',
    recurringSkips: '',
    callNumber: '',
    location: '',
    holdingsStatement: '',
    notes: '',
    status: 'active',
  };
}

export function subscriptionToForm(subscription: SerialSubscription): SubscriptionFormValues {
  return {
    biblioId: subscription.biblioId,
    biblioTitle: subscription.biblioTitle?.trim() || subscription.biblioId,
    siteId: subscription.siteId,
    vendorId: subscription.vendorId ?? '',
    fundId: subscription.fundId ?? '',
    costEuros: centsToEurosInput(subscription.costCents),
    issuePriceEuros: centsToEurosInput(subscription.issuePriceCents),
    frequencyId: subscription.frequencyId,
    numberingPatternId: subscription.numberingPatternId,
    firstIssueDate: subscription.firstIssueDate,
    endDate: subscription.endDate ?? '',
    x: String(subscription.x),
    y: String(subscription.y),
    z: String(subscription.z),
    innerX: String(subscription.innerX),
    innerY: String(subscription.innerY),
    innerZ: String(subscription.innerZ),
    lengthKind: subscription.length.kind,
    lengthCount: String(subscription.length.count),
    graceDays: subscription.graceDays == null ? '' : String(subscription.graceDays),
    createItems: subscription.createItems,
    latestIssueLibraryUseOnly: subscription.latestIssueLibraryUseOnly,
    irregularities: subscription.irregularities.join(', '),
    recurringSkips: formatRecurringSkips(subscription.recurringSkips),
    callNumber: subscription.callNumber ?? '',
    location: subscription.location ?? '',
    holdingsStatement: subscription.holdingsStatement ?? '',
    notes: subscription.notes ?? '',
    status: subscription.status,
  };
}

/** Empty stays null. A typed 0 stays 0 cents. */
export function eurosToCents(raw: string): { ok: true; cents: number | null } | { ok: false } {
  const trimmed = raw.trim().replace(/\s/g, '').replace(',', '.');
  if (!trimmed) return { ok: true, cents: null };
  if (!/^\d+(\.\d{1,2})?$/.test(trimmed)) return { ok: false };
  const euros = Number(trimmed);
  if (!Number.isFinite(euros)) return { ok: false };
  return { ok: true, cents: Math.round(euros * 100) };
}

export function centsToEurosInput(cents: number | null | undefined): string {
  if (cents == null) return '';
  return (cents / 100).toFixed(2);
}

export function formatSerialCents(cents: number | null | undefined, locale: string): string {
  if (cents == null) return '—';
  return formatCentsAsEuros(cents, locale);
}

/**
 * Days after the planned date before an issue may be marked late.
 * Mirrors `serials::grace::default_grace_days`. Null for an irregular frequency.
 */
export function defaultGraceDays(
  unit: string | null | undefined,
  issuesPerUnit: number,
  unitsPerIssue: number,
): number | null {
  if (unit == null || issuesPerUnit < 1 || unitsPerIssue < 1) return null;
  const unitDays = unit === 'day' ? 1 : unit === 'week' ? 7 : unit === 'month' ? 30 : unit === 'year' ? 365 : null;
  if (unitDays == null) return null;
  const numerator = unitsPerIssue * unitDays;
  const denominator = issuesPerUnit;
  if (numerator < denominator) return 1;
  if (numerator < 4 * denominator) return 2;
  if (numerator < 10 * denominator) return 3;
  if (numerator < 20 * denominator) return 7;
  if (numerator < 45 * denominator) return 15;
  if (numerator < 120 * denominator) return 30;
  return 45;
}

export function frequencyGracePlaceholder(frequency: SerialFrequency | undefined): number | null {
  if (!frequency) return null;
  return defaultGraceDays(frequency.unit, frequency.issuesPerUnit, frequency.unitsPerIssue);
}

export function frequencyLabel(t: TFunction, frequency: SerialFrequency): string {
  const key = serialFrequencyLabelKey(frequency.unit, frequency.issuesPerUnit, frequency.unitsPerIssue);
  if (!key) return frequency.description;
  const label = t(key);
  return label === key ? frequency.description : label;
}

export function subscriptionStatusKey(status: SerialSubscriptionStatus): string {
  return `serials.status.${status}`;
}

export function issueStatusKey(status: SerialIssueStatus): string {
  return `serials.issueStatus.${status}`;
}

export function issueStatusVariant(
  status: SerialIssueStatus,
): 'default' | 'success' | 'warning' | 'danger' | 'info' {
  if (status === 'arrived') return 'success';
  if (status === 'expected') return 'info';
  if (status === 'late' || status === 'claimed') return 'warning';
  if (status === 'missing') return 'danger';
  return 'default';
}

/** Arrived regular issue with the newest planned date. Supplements never qualify. */
export function latestIssueId(issues: SerialIssue[]): string | null {
  const candidates = issues.filter(
    (issue) => !issue.supplement && issue.status === 'arrived' && issue.itemId,
  );
  if (candidates.length === 0) return null;
  const ranked = [...candidates].sort((a, b) => {
    if (a.plannedDate !== b.plannedDate) return a.plannedDate < b.plannedDate ? 1 : -1;
    const slotA = a.slot ?? -1;
    const slotB = b.slot ?? -1;
    if (slotA !== slotB) return slotB - slotA;
    return b.id.localeCompare(a.id, undefined, { numeric: true });
  });
  return ranked[0]?.id ?? null;
}

export function canReceiveIssue(status: SerialIssueStatus): boolean {
  return status === 'expected' || status === 'late';
}

/** Next row that can still be received, after `receivedId` in list order. */
export function nextReceivableIssueId(issues: SerialIssue[], receivedId: string): string | null {
  const index = issues.findIndex((issue) => issue.id === receivedId);
  const rest = index >= 0 ? issues.slice(index + 1).concat(issues.slice(0, index)) : issues;
  return rest.find((issue) => issue.id !== receivedId && canReceiveIssue(issue.status))?.id ?? null;
}

export function focusReceiveButton(issueId: string): void {
  const nodes = document.querySelectorAll<HTMLButtonElement>(`[data-serial-receive="${CSS.escape(issueId)}"]`);
  const visible = Array.from(nodes).find((node) => node.offsetParent !== null);
  const target = visible ?? nodes[0];
  if (!target) return;
  // Buttons use :focus-visible, which a script focus does not show. An inline
  // outline keeps the keyboard place visible. !important beats outline-none.
  target.style.setProperty('outline', '3px solid CanvasText', 'important');
  target.style.setProperty('outline-offset', '3px', 'important');
  try {
    target.focus({ focusVisible: true } as FocusOptions);
  } catch {
    target.focus();
  }
  window.setTimeout(() => {
    target.addEventListener('blur', () => {
      target.style.removeProperty('outline');
      target.style.removeProperty('outline-offset');
    }, { once: true });
  }, 0);
}

const MONTHS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] as const;

export function skipMonths(raw: string): number[] {
  const parsed = parseRecurringSkips(raw);
  if (!parsed.ok) return [];
  return [...new Set(parsed.skips.map((skip) => skip.month))].sort((left, right) => left - right);
}

/** Month checkboxes replace the free-text skip field. A toggle stores months only. */
export function toggleSkipMonth(raw: string, month: number): string {
  const selected = new Set(skipMonths(raw));
  if (selected.has(month)) selected.delete(month);
  else selected.add(month);
  return [...selected].sort((left, right) => left - right).join(', ');
}

export function monthChoices(): readonly number[] {
  return MONTHS;
}

export function monthName(month: number, locale: string): string {
  return new Date(2026, month - 1, 1).toLocaleDateString(locale, { month: 'long' });
}

function dateParts(value: string): { year: number; month: number; day: number } | null {
  const [year, month, day] = value.slice(0, 10).split('-').map(Number);
  if (!year || !month || !day) return null;
  return { year, month, day };
}

function longDate(value: string, locale: string, withYear: boolean): string {
  const parts = dateParts(value);
  if (!parts) return value;
  return new Date(parts.year, parts.month - 1, parts.day).toLocaleDateString(locale, {
    day: 'numeric',
    month: 'long',
    ...(withYear ? { year: 'numeric' as const } : {}),
  });
}

/** First and last planned dates for the short preview announcement. */
export function previewDateSpan(
  issues: Array<{ plannedDate: string }>,
  locale: string,
): { from: string; to: string } | null {
  const dates = issues.map((issue) => issue.plannedDate).filter(Boolean).sort();
  const from = dates[0];
  const to = dates[dates.length - 1];
  if (!from || !to) return null;
  const start = dateParts(from);
  const end = dateParts(to);
  const sameYear = start != null && end != null && start.year === end.year;
  return {
    from: longDate(from, locale, !sameYear),
    to: longDate(to, locale, true),
  };
}

export function formatIsoDate(value: string | null | undefined, locale: string): string {
  if (!value) return '—';
  const [year, month, day] = value.slice(0, 10).split('-').map(Number);
  if (!year || !month || !day) return value;
  return new Date(year, month - 1, day).toLocaleDateString(locale);
}

export function formatIsoDateTime(value: string | null | undefined, locale: string): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale);
}

export function isEmptyHorizonError(error: unknown): boolean {
  if (getHttpStatus(error) !== 422) return false;
  const message = (getApiErrorRawMessage(error) ?? '').toLowerCase();
  return message.includes('produced no issues') || message.includes('no issues');
}

export interface PredictionBody {
  frequencyId: string;
  patternId: string;
  firstIssueDate: string;
  starting: { x: number; y: number; z: number };
  inner: { x: number; y: number; z: number };
  startSlot: number;
  length: { kind: SerialLengthKind; count: number };
  irregularities: number[];
  recurringSkips: Array<{ month: number; day?: number }>;
  combinedIssues: Array<{ months: number[]; monthName: string }>;
}

export function predictionBody(values: SubscriptionFormValues): PredictionBody | null {
  if (!values.frequencyId || !values.numberingPatternId || !values.firstIssueDate) return null;
  const count = Number(values.lengthCount);
  const x = Number(values.x);
  const y = Number(values.y);
  const z = Number(values.z);
  if (!Number.isInteger(count) || count < 1) return null;
  if (!Number.isInteger(x) || !Number.isInteger(y) || !Number.isInteger(z)) return null;
  const skips = parseRecurringSkips(values.recurringSkips);
  const slots = parseSlotList(values.irregularities);
  if (!skips.ok || !slots.ok) return null;
  return {
    frequencyId: values.frequencyId,
    patternId: values.numberingPatternId,
    firstIssueDate: values.firstIssueDate,
    starting: { x, y, z },
    inner: {
      x: integerOrZero(values.innerX),
      y: integerOrZero(values.innerY),
      z: integerOrZero(values.innerZ),
    },
    startSlot: 1,
    length: { kind: values.lengthKind, count },
    irregularities: slots.slots,
    recurringSkips: skips.skips,
    combinedIssues: [],
  };
}

export type SubscriptionWriteResult =
  | { ok: true; body: SerialSubscriptionWrite }
  | { ok: false; errors: Record<string, 'required' | 'invalidAmount' | 'invalidNumber'> };

export function buildSubscriptionWrite(
  values: SubscriptionFormValues,
  existing?: SerialSubscription | null,
): SubscriptionWriteResult {
  const errors: Record<string, 'required' | 'invalidAmount' | 'invalidNumber'> = {};
  if (!values.biblioId) errors.biblioId = 'required';
  if (!values.siteId) errors.siteId = 'required';
  if (!values.frequencyId) errors.frequencyId = 'required';
  if (!values.numberingPatternId) errors.numberingPatternId = 'required';
  if (!values.firstIssueDate) errors.firstIssueDate = 'required';
  const cost = eurosToCents(values.costEuros);
  const price = eurosToCents(values.issuePriceEuros);
  if (!cost.ok) errors.costEuros = 'invalidAmount';
  if (!price.ok) errors.issuePriceEuros = 'invalidAmount';
  if (price.ok && price.cents != null && price.cents > 2_147_483_647) errors.issuePriceEuros = 'invalidAmount';
  const count = Number(values.lengthCount);
  if (!Number.isInteger(count) || count < 1) errors.lengthCount = 'invalidNumber';
  const x = Number(values.x);
  const y = Number(values.y);
  const z = Number(values.z);
  if (!Number.isInteger(x)) errors.x = 'invalidNumber';
  if (!Number.isInteger(y)) errors.y = 'invalidNumber';
  if (!Number.isInteger(z)) errors.z = 'invalidNumber';
  const skips = parseRecurringSkips(values.recurringSkips);
  const slots = parseSlotList(values.irregularities);
  if (!skips.ok) errors.recurringSkips = 'invalidNumber';
  if (!slots.ok) errors.irregularities = 'invalidNumber';
  let graceDays: number | null = null;
  if (values.graceDays.trim()) {
    const grace = Number(values.graceDays);
    if (!Number.isInteger(grace) || grace < 0) errors.graceDays = 'invalidNumber';
    else graceDays = grace;
  }
  if (Object.keys(errors).length > 0 || !cost.ok || !price.ok || !skips.ok || !slots.ok) {
    return { ok: false, errors };
  }
  const locked = (existing?.issueCount ?? 0) > 0;
  return {
    ok: true,
    body: {
      biblioId: values.biblioId,
      siteId: values.siteId,
      vendorId: values.vendorId || null,
      fundId: values.fundId || null,
      costCents: cost.cents,
      issuePriceCents: price.cents,
      frequencyId: locked && existing ? existing.frequencyId : values.frequencyId,
      numberingPatternId: locked && existing ? existing.numberingPatternId : values.numberingPatternId,
      firstIssueDate: locked && existing ? existing.firstIssueDate : values.firstIssueDate,
      endDate: blank(locked && existing ? existing.endDate : values.endDate),
      x: locked && existing ? existing.x : x,
      y: locked && existing ? existing.y : y,
      z: locked && existing ? existing.z : z,
      innerX: locked && existing ? existing.innerX : integerOrZero(values.innerX),
      innerY: locked && existing ? existing.innerY : integerOrZero(values.innerY),
      innerZ: locked && existing ? existing.innerZ : integerOrZero(values.innerZ),
      length: locked && existing ? existing.length : { kind: values.lengthKind, count },
      irregularities: locked && existing ? existing.irregularities : slots.slots,
      recurringSkips: locked && existing ? existing.recurringSkips : skips.skips,
      combinedIssues: existing?.combinedIssues ?? [],
      graceDays,
      status: values.status,
      createItems: values.createItems,
      latestIssueLibraryUseOnly: values.latestIssueLibraryUseOnly,
      holdingsStatement: blank(values.holdingsStatement),
      callNumber: blank(values.callNumber),
      location: blank(values.location),
      notes: blank(values.notes),
      startDate: existing?.startDate ?? null,
      publicNote: existing?.publicNote ?? null,
      internalNote: existing?.internalNote ?? null,
    },
  };
}

function blank(value: string | null | undefined): string | null {
  const trimmed = (value ?? '').trim();
  return trimmed ? trimmed : null;
}

function integerOrZero(raw: string): number {
  const n = Number(raw);
  return Number.isInteger(n) && n >= 0 ? n : 0;
}

export function parseSlotList(raw: string): { ok: true; slots: number[] } | { ok: false } {
  const parts = raw.split(/[,;\s]+/).map((part) => part.trim()).filter(Boolean);
  const slots: number[] = [];
  for (const part of parts) {
    if (!/^\d+$/.test(part)) return { ok: false };
    const n = Number(part);
    if (n < 1) return { ok: false };
    slots.push(n);
  }
  return { ok: true, slots };
}

export function parseRecurringSkips(
  raw: string,
): { ok: true; skips: Array<{ month: number; day?: number }> } | { ok: false } {
  const parts = raw.split(/[,;\n]+/).map((part) => part.trim()).filter(Boolean);
  const skips: Array<{ month: number; day?: number }> = [];
  for (const part of parts) {
    const match = /^(\d{1,2})(?:\/(\d{1,2}))?$/.exec(part);
    if (!match) return { ok: false };
    const month = Number(match[1]);
    const day = match[2] != null ? Number(match[2]) : undefined;
    if (month < 1 || month > 12) return { ok: false };
    if (day != null && (day < 1 || day > 31)) return { ok: false };
    skips.push(day == null ? { month } : { month, day });
  }
  return { ok: true, skips };
}

function formatRecurringSkips(skips: Array<{ month: number; day?: number | null }>): string {
  return skips
    .map((skip) => (skip.day != null ? `${skip.month}/${skip.day}` : String(skip.month)))
    .join(', ');
}

export function barcodeFromNotice(
  notice: { items?: Array<{ id?: string | null; barcode?: string | null }> },
  itemId: string,
): string | null {
  const barcode = notice.items?.find((item) => item.id === itemId)?.barcode?.trim();
  return barcode ? barcode : null;
}
