import type { TFunction } from 'i18next';
import type { ItemAvailability, ItemPublicAvailability, ItemWeedingStatus } from '@/types';
import { CIRCULATION_STATUS } from '@/utils/circulationStatus';
import { isItemWithdrawn } from '@/utils/weeding';

export type AvailabilityAudience = 'staff' | 'public';
export type AvailabilityBadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'info';

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
 * Public responses never contain `toSearch`; a leaked value is shown as `unavailable`.
 */
export function normalizeAvailability(
  value: string | null | undefined,
  audience: AvailabilityAudience,
): ItemAvailability {
  if (value === 'toSearch' && audience === 'public') return 'unavailable';
  if (isItemAvailability(value)) return value;
  return 'unavailable';
}

export function availabilityLabelKey(status: ItemAvailability | ItemPublicAvailability): string {
  return `availability.${status}`;
}

export function availabilityBadgeVariant(
  status: ItemAvailability | ItemPublicAvailability,
): AvailabilityBadgeVariant {
  switch (status) {
    case 'available':
      return 'success';
    case 'onHold':
      return 'warning';
    case 'inTransit':
      return 'info';
    case 'onLoan':
      return 'danger';
    case 'toSearch':
      return 'warning';
    default:
      return 'default';
  }
}

/** Copies the server marked `available`. Do not recompute this from other fields. */
export function countAvailableCopies(
  items?: ReadonlyArray<{ availability?: string | null }> | null,
): number {
  if (!items?.length) return 0;
  let count = 0;
  for (const item of items) {
    if (item.availability === 'available') count += 1;
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

/** Search-result copy line. Null when the notice has no copies. */
export function copyAvailabilityLabel(
  t: TFunction,
  items?: ReadonlyArray<{ availability?: string | null }> | null,
): string | null {
  const total = items?.length ?? 0;
  const available = countAvailableCopies(items);
  if (Math.max(total, available) === 0) return null;
  return formatCopyAvailability(t, available, total);
}

/**
 * Search-result pill. Any `available` copy wins; otherwise the first blocking
 * state present on the server field.
 */
export function summaryAvailability(
  items: ReadonlyArray<{ availability?: string | null }> | null | undefined,
  audience: AvailabilityAudience,
): ItemAvailability {
  if (!items?.length) return 'unavailable';
  const statuses = items.map((item) => normalizeAvailability(item.availability, audience));
  if (statuses.some((status) => status === 'available')) return 'available';
  for (const status of SUMMARY_ORDER) {
    if (statuses.some((value) => value === status)) return status;
  }
  return 'unavailable';
}

export interface CopyLocationLike {
  availability?: string | null;
  callNumber?: string | null;
  sourceName?: string | null;
}

/** Prefer a copy the server marked available; otherwise the first copy with a location. */
export function primaryCopyLocation(items?: CopyLocationLike[] | null): {
  callNumber: string | null;
  sourceName: string | null;
} {
  if (!items?.length) return { callNumber: null, sourceName: null };
  const preferred =
    items.find((item) => item.availability === 'available') ??
    items.find((item) => item.callNumber || item.sourceName) ??
    items[0];
  return {
    callNumber: preferred?.callNumber?.trim() || null,
    sourceName: preferred?.sourceName?.trim() || null,
  };
}

export function uniqueCopySites(items?: CopyLocationLike[] | null): string[] {
  if (!items?.length) return [];
  const seen = new Set<string>();
  const sites: string[] = [];
  for (const item of items) {
    const name = item.sourceName?.trim();
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
