import type { TFunction } from 'i18next';
import type { ItemAvailability, ItemPublicAvailability, ItemWeedingStatus } from '@/types';
import { CIRCULATION_STATUS } from '@/utils/circulationStatus';
import { isItemWithdrawn } from '@/utils/weeding';

export type AvailabilityAudience = 'staff' | 'public';
export type AvailabilityBadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'info';
export type DisplayAvailability = ItemAvailability | 'onSiteOnly';

const STAFF_VALUES: readonly ItemAvailability[] = [
  'available',
  'onHold',
  'inTransit',
  'onLoan',
  'toSearch',
  'unavailable',
];

/** Blocking states, same precedence the server uses for a single copy. */
const SUMMARY_ORDER: readonly ItemAvailability[] = [
  'onLoan',
  'inTransit',
  'onHold',
  'toSearch',
  'unavailable',
];

export function isItemAvailability(value: string | null | undefined): value is ItemAvailability {
  return typeof value === 'string' && STAFF_VALUES.includes(value as ItemAvailability);
}

/**
 * Map a server value onto the pill the audience is allowed to see.
 * Public `toSearch` is not a public value; a leaked one is shown as `unavailable`.
 * `onSiteOnly` is the public name for a copy excluded from loan.
 */
export function normalizeAvailability(
  value: string | null | undefined,
  audience: AvailabilityAudience,
): DisplayAvailability {
  if (value === 'onSiteOnly') return 'onSiteOnly';
  if (value === 'toSearch' && audience === 'public') return 'unavailable';
  if (isItemAvailability(value)) return value;
  return 'unavailable';
}

export function availabilityLabelKey(status: DisplayAvailability | ItemPublicAvailability): string {
  return `availability.${status}`;
}

export function availabilityBadgeVariant(
  status: DisplayAvailability | ItemPublicAvailability,
): AvailabilityBadgeVariant {
  switch (status) {
    case 'available':
      return 'success';
    case 'onHold':
      return 'warning';
    case 'inTransit':
    case 'onSiteOnly':
      return 'info';
    case 'onLoan':
      return 'danger';
    case 'toSearch':
      return 'warning';
    default:
      return 'default';
  }
}

/** Borrowable copies the server marked `available`. `borrowable: false` stays out of N. */
export function countAvailableCopies(
  items?: ReadonlyArray<{ availability?: string | null; borrowable?: boolean | null }> | null,
): number {
  if (!items?.length) return 0;
  let count = 0;
  for (const item of items) {
    if (item.availability === 'available' && item.borrowable !== false) count += 1;
  }
  return count;
}

/**
 * "N available of M copies". `total` is the copy list length (the API has no
 * separate total). Available is never allowed to imply a total of zero.
 */
export function formatCopyAvailability(t: TFunction, available: number, total: number): string {
  const ready = Math.max(0, available);
  const copies = Math.max(0, total, ready);
  return t('items.availableOfTotal', {
    availablePart: t('items.availableCount', { count: ready }),
    totalPart: t('items.copyTotal', { count: copies }),
  });
}

/**
 * OPAC and reader search line. `M` is `items.length`: the server already omits
 * lost, declared missing, weeding-in-progress, and withdrawn copies. `N` counts
 * only borrowable `available` copies. `onSiteOnly` stays inside `M` and is
 * named after a middle dot, never inside `N`.
 */
export function copyAvailabilityLabel(
  t: TFunction,
  items?: ReadonlyArray<{ availability?: string | null; borrowable?: boolean | null }> | null,
): string | null {
  const total = items?.length ?? 0;
  const available = countAvailableCopies(items);
  const onSite = countCopiesWithAvailability(items, 'onSiteOnly');
  const copies = Math.max(total, available, onSite);
  if (copies === 0) return null;
  if (onSite === 0) return formatCopyAvailability(t, available, copies);
  return t('items.publicWithOnSite', {
    availablePart: t('items.availableCount', { count: available }),
    total: copies,
    onSitePart: t('items.onSiteAside', { count: onSite }),
  });
}

const STAFF_BUCKET_ORDER = [
  'available',
  'onLoan',
  'onHold',
  'inTransit',
  'onSiteOnly',
  'toSearch',
  'lost',
  'missing',
  'unavailable',
] as const;

export interface StaffCopyCountSource {
  availability?: string | null;
  weedingStatus?: string | null;
  borrowable?: boolean | null;
  circulationStatus?: number | null;
}

function circulationCode(value: number | null | undefined): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

/**
 * Staff partition of one copy. Lost (`1`) and declared missing (`5`) leave the
 * unavailable bucket. On-site consultation is `unavailable` with
 * `borrowable === false` and no circulation exception. A weeding candidate
 * keeps the server availability, including `available`.
 */
export function staffCopyBucket(item: StaffCopyCountSource): string {
  const availability = item.availability?.trim() ?? '';
  const circulation = circulationCode(item.circulationStatus);
  const withdrawn = item.weedingStatus === 'withdrawn';

  if (!withdrawn) {
    if (availability === 'available') return 'available';
    if (availability === 'onLoan') return 'onLoan';
    if (availability === 'onHold') return 'onHold';
    if (availability === 'inTransit') return 'inTransit';
  }
  if (availability === 'toSearch' || circulation === CIRCULATION_STATUS.TO_SEARCH) return 'toSearch';
  if (circulation === CIRCULATION_STATUS.LOST) return 'lost';
  if (circulation === CIRCULATION_STATUS.MISSING) return 'missing';
  if (
    !withdrawn &&
    availability === 'unavailable' &&
    item.borrowable === false &&
    (circulation === null || circulation === CIRCULATION_STATUS.AVAILABLE)
  ) {
    return 'onSiteOnly';
  }
  if (
    availability === 'available' ||
    availability === 'onLoan' ||
    availability === 'onHold' ||
    availability === 'inTransit'
  ) {
    return availability;
  }
  if (!availability) return 'unspecified';
  return 'unavailable';
}

/**
 * Staff search line. `M` is every embedded copy. Lost and missing are split
 * out of `unavailable` with `circulationStatus`. Weeding candidate and
 * withdrawn stay an extra clause.
 */
export function staffCopyAvailabilityLabel(
  t: TFunction,
  items?: ReadonlyArray<StaffCopyCountSource> | null,
): string | null {
  if (!items?.length) return null;

  const counts = new Map<string, number>();
  let candidates = 0;
  let withdrawn = 0;
  for (const item of items) {
    const status = staffCopyBucket(item);
    counts.set(status, (counts.get(status) ?? 0) + 1);
    if (item.weedingStatus === 'candidate') candidates += 1;
    if (item.weedingStatus === 'withdrawn') withdrawn += 1;
  }

  const parts: string[] = [];
  const seen = new Set<string>();
  for (const status of STAFF_BUCKET_ORDER) {
    const count = counts.get(status) ?? 0;
    if (count === 0) continue;
    seen.add(status);
    parts.push(t(`items.staffStatus.${status}`, { count }));
  }
  for (const [status, count] of counts) {
    if (seen.has(status) || count === 0) continue;
    parts.push(
      status === 'unspecified'
        ? t('items.staffStatus.unspecified', { count })
        : t('items.staffStatus.other', { count, status }),
    );
  }

  const weedingParts: string[] = [];
  if (candidates > 0) weedingParts.push(t('items.staffStatus.candidate', { count: candidates }));
  if (withdrawn > 0) weedingParts.push(t('items.staffStatus.withdrawn', { count: withdrawn }));

  return t('items.staffCopyBreakdown', {
    totalPart: t('items.copyNoun', { count: items.length }),
    breakdown: parts.join(', '),
    weedingClause: weedingParts.length
      ? t('items.staffWeedingClause', { weeding: weedingParts.join(', ') })
      : '',
  });
}

function countCopiesWithAvailability(
  items: ReadonlyArray<{ availability?: string | null }> | null | undefined,
  availability: string,
): number {
  if (!items?.length) return 0;
  let count = 0;
  for (const item of items) {
    if (item.availability === availability) count += 1;
  }
  return count;
}

const PUBLIC_SUMMARY_ORDER: readonly DisplayAvailability[] = ['onLoan', 'onSiteOnly', 'unavailable'];

/**
 * Search-result pill. Any `available` copy wins; otherwise the first blocking
 * state present on the server field. Public `onSiteOnly` is shown when no
 * loanable copy is available and none is on loan.
 */
export function summaryAvailability(
  items: ReadonlyArray<{ availability?: string | null }> | null | undefined,
  audience: AvailabilityAudience,
): DisplayAvailability {
  if (!items?.length) return 'unavailable';
  const statuses = items.map((item) => normalizeAvailability(item.availability, audience));
  if (statuses.some((status) => status === 'available')) return 'available';
  const order = audience === 'public' ? PUBLIC_SUMMARY_ORDER : SUMMARY_ORDER;
  for (const status of order) {
    if (statuses.some((value) => value === status)) return status;
  }
  return 'unavailable';
}

export interface CopyLocationLike {
  availability?: string | null;
  callNumber?: string | null;
  siteName?: string | null;
}

/** Prefer a copy the server marked available; otherwise the first copy with a location. */
export function primaryCopyLocation(items?: CopyLocationLike[] | null): {
  callNumber: string | null;
  siteName: string | null;
} {
  if (!items?.length) return { callNumber: null, siteName: null };
  const preferred =
    items.find((item) => item.availability === 'available') ??
    items.find((item) => item.callNumber || item.siteName) ??
    items[0];
  return {
    callNumber: preferred?.callNumber?.trim() || null,
    siteName: preferred?.siteName?.trim() || null,
  };
}

export function uniqueCopySites(items?: CopyLocationLike[] | null): string[] {
  if (!items?.length) return [];
  const seen = new Set<string>();
  const sites: string[] = [];
  for (const item of items) {
    const name = item.siteName?.trim();
    if (!name || seen.has(name)) continue;
    seen.add(name);
    sites.push(name);
  }
  return sites;
}

/**
 * Staff-only reason under an `unavailable` pill. Public notices stay on the
 * neutral label and must not surface this.
 */
export function staffUnavailableDetailKey(item: {
  availability?: string | null;
  weedingStatus?: ItemWeedingStatus | null;
  circulationStatus?: number | string | null;
  borrowable?: boolean | null;
}): string | null {
  if (normalizeAvailability(item.availability, 'staff') !== 'unavailable') return null;
  if (isItemWithdrawn(item)) return 'weeding.status.withdrawn';
  const value = item.circulationStatus;
  if (value === CIRCULATION_STATUS.LOST || value === 'lost') return 'loans.exceptions.lost';
  if (value === CIRCULATION_STATUS.DAMAGED || value === 'damaged') return 'loans.exceptions.damaged';
  if (value === CIRCULATION_STATUS.CLAIMED_RETURNED || value === 'claimedReturned') {
    return 'loans.exceptions.claimedReturned';
  }
  if (item.borrowable === false) return 'items.incompleteNotCirculable';
  return null;
}
