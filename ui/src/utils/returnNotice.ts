import type { ItemCheckIn } from '@/types';

type Translate = (key: string, values?: Record<string, unknown>) => string;

export interface ReturnDeskNotice {
  level: 'informational' | 'warning';
  code: 'item_not_on_loan' | 'item_not_available_status';
  circulationStatus: string | null;
  weedingStatus: string | null;
}

export interface ReturnScanFeedback {
  /**
   * `success` is a real return or a hold/transit.
   * `info` is a copy that was not on loan.
   * `none` is a warning notice: no success toast.
   */
  toast: 'success' | 'info' | 'none';
  notice: ReturnDeskNotice | null;
  /** Shelf instruction only when nothing else happened. */
  putBack: boolean;
}

/**
 * Existing label catalogs. The status string from the response is the key suffix.
 * This is not a list of blocked statuses.
 */
const STATUS_LABEL_NAMESPACES = [
  'loans.exceptions',
  'weeding.status',
  'availability',
  'items',
  'loans.closure',
  'opac',
] as const;

function clean(value: string | null | undefined): string | null {
  const text = value?.trim();
  return text ? text : null;
}

/** Catalog text, or null when that key is not translated. */
function catalogLabel(t: Translate, key: string, values?: Record<string, unknown>): string | null {
  const missing = `\u0000${key}`;
  const text = t(key, { ...values, defaultValue: missing }).trim();
  if (!text || text === key || text === missing) return null;
  return text;
}

/**
 * Label for one returned status token.
 * Resting values (`available`, `onShelf`) are not a copy exception.
 * Any other token uses the first existing status translation, then a generic fallback.
 */
export function returnStatusLabel(
  status: string | null | undefined,
  t: Translate,
): string | null {
  const value = clean(status);
  if (!value || value === 'available' || value === 'onShelf') return null;
  for (const namespace of STATUS_LABEL_NAMESPACES) {
    const label = catalogLabel(t, `${namespace}.${value}`);
    if (label) return label;
  }
  const counted = catalogLabel(t, `items.staffStatus.${value}`, { count: 1 });
  if (counted) {
    const word = counted.replace(/^\d+\s+/, '').trim();
    if (word) return word;
  }
  return t('loans.returnNoticeUnknownStatus', { status: value });
}

/** Circulation exception first, then weeding. Unknown tokens use the generic label. */
export function returnCopyStatusLabel(
  circulationStatus: string | null | undefined,
  weedingStatus: string | null | undefined,
  t: Translate,
): string {
  return (
    returnStatusLabel(circulationStatus, t) ??
    returnStatusLabel(weedingStatus, t) ??
    t('availability.unavailable')
  );
}

/** Notice on a barcode check-in, or null when the body has none. Level and code decide the tone. */
export function returnDeskNotice(body: ItemCheckIn | null | undefined): ReturnDeskNotice | null {
  const notice = body?.notice;
  if (!notice) return null;
  const statuses = {
    circulationStatus: clean(body?.circulationStatus),
    weedingStatus: clean(body?.weedingStatus),
  };
  if (notice.level === 'warning' || notice.code === 'item_not_available_status') {
    return { level: 'warning', code: 'item_not_available_status', ...statuses };
  }
  if (notice.level === 'informational' || notice.code === 'item_not_on_loan') {
    return { level: 'informational', code: 'item_not_on_loan', ...statuses };
  }
  return null;
}

export function returnNoticeMessage(
  notice: ReturnDeskNotice,
  t: Translate,
  options?: { putBack?: boolean },
): string {
  if (notice.level === 'warning') {
    const status = returnCopyStatusLabel(notice.circulationStatus, notice.weedingStatus, t);
    return t('loans.returnNoticeBlocked', { status });
  }
  const base = t('loans.returnNoticeNotOnLoan');
  if (!options?.putBack) return base;
  return `${base} ${t('loans.returnNoticePutBack')}`.trim();
}

/**
 * How the desk should react after `POST /loans/items/{barcode}/return`.
 * A warning never asks for the success toast.
 */
export function returnScanFeedback(
  body: ItemCheckIn | null | undefined,
  options: { pick: boolean; hasLoan: boolean },
): ReturnScanFeedback {
  const notice = returnDeskNotice(body);
  if (notice?.level === 'warning') {
    return { toast: 'none', notice, putBack: false };
  }
  if (options.pick || options.hasLoan) {
    return { toast: 'success', notice, putBack: false };
  }
  if (notice?.level === 'informational') {
    return { toast: 'info', notice, putBack: true };
  }
  return { toast: 'success', notice: null, putBack: false };
}
