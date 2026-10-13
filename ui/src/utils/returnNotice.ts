import type { ItemCheckIn } from '@/types';

type Translate = (key: string, values?: Record<string, unknown>) => string;

export interface ReturnDeskNotice {
  level: 'informational' | 'warning';
  code: 'item_not_on_loan' | 'item_not_available_status';
  /** Existing status i18n key. Set for a warning. */
  statusLabelKey: string | null;
}

export interface ReturnScanFeedback {
  /**
   * `success` is a real return or a hold/transit.
   * `info` is a copy that was not on loan.
   * `none` is a blocked status: no success toast.
   */
  toast: 'success' | 'info' | 'none';
  notice: ReturnDeskNotice | null;
  /** Shelf instruction only when nothing else happened. */
  putBack: boolean;
}

/**
 * Status label already used on the copy.
 * A circulation exception wins over weeding so a lost copy stays "lost".
 * Declared missing has no standalone noun; the staff count phrase is the source.
 */
export function returnBlockedStatusLabelKey(
  circulationStatus: string | null | undefined,
  weedingStatus: string | null | undefined,
): string {
  switch ((circulationStatus ?? '').trim()) {
    case 'lost':
      return 'loans.exceptions.lost';
    case 'damaged':
      return 'loans.exceptions.damaged';
    case 'toSearch':
      return 'items.toSearch';
    case 'missing':
      return 'items.staffStatus.missing';
    default:
      break;
  }
  if ((weedingStatus ?? '').trim() === 'withdrawn') return 'weeding.status.withdrawn';
  return 'availability.unavailable';
}

/** Notice on a barcode check-in, or null when the body has none. */
export function returnDeskNotice(body: ItemCheckIn | null | undefined): ReturnDeskNotice | null {
  const notice = body?.notice;
  if (!notice) return null;
  if (notice.code === 'item_not_available_status' || notice.level === 'warning') {
    return {
      level: 'warning',
      code: 'item_not_available_status',
      statusLabelKey: returnBlockedStatusLabelKey(body?.circulationStatus, body?.weedingStatus),
    };
  }
  if (notice.code === 'item_not_on_loan' || notice.level === 'informational') {
    return { level: 'informational', code: 'item_not_on_loan', statusLabelKey: null };
  }
  return null;
}

/** Drop a leading count from the staff "N missing" phrase. */
export function returnStatusLabel(key: string, t: Translate): string {
  if (key === 'items.staffStatus.missing') {
    return t(key, { count: 1 }).replace(/^\d+\s+/, '').trim();
  }
  return t(key);
}

export function returnNoticeMessage(
  notice: ReturnDeskNotice,
  t: Translate,
  options?: { putBack?: boolean },
): string {
  if (notice.level === 'warning') {
    const status = returnStatusLabel(notice.statusLabelKey ?? 'availability.unavailable', t);
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
