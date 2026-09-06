import type { CreateHold, Hold, ItemShort } from '@/types';

/** Staff `/holds` status chips. List is already active-only from the API. */
export type StaffHoldStatusFilter = 'all' | 'ready' | 'pending' | 'expiringSoon';

/** Ready holds whose pickup deadline falls in this window are “expiring soon”. */
export const HOLD_EXPIRING_SOON_MS = 48 * 60 * 60 * 1000;

/** Max rows fetched so staff filters/sort can run on the full active queue. */
export const STAFF_HOLDS_FETCH_CAP = 200;

/** Title-level until fulfillment assigns a copy (`itemId` null). */
export function isTitleLevelHold(h: Hold): boolean {
  return h.itemId == null || h.itemId === '';
}

/** Pending hold with a pinned specimen on the same biblio FIFO. */
export function isPinnedCopyHold(h: Hold): boolean {
  return !isTitleLevelHold(h) && h.status === 'pending';
}

/** Display kind: unassigned title queue vs staff-pinned copy vs allocated copy. */
export type HoldScopeKind = 'title' | 'pinned' | 'allocated';

export function holdScopeKind(h: Hold): HoldScopeKind {
  if (isTitleLevelHold(h)) return 'title';
  if (h.status === 'pending') return 'pinned';
  return 'allocated';
}

export type HoldPlacementScope = 'title' | 'copy';

/** Always send `biblioId`; set `itemId` only when pinning a copy. */
export function buildCreateHold(input: {
  userId: string;
  biblioId: string;
  scope: HoldPlacementScope;
  itemId?: string | null;
  notes?: string | null;
  pickupSiteId?: string | null;
}): CreateHold {
  const notes = input.notes?.trim() || undefined;
  const pickupSiteId = input.pickupSiteId?.trim() || undefined;
  return {
    userId: input.userId,
    biblioId: input.biblioId,
    itemId: input.scope === 'copy' && input.itemId ? input.itemId : undefined,
    notes,
    pickupSiteId,
  };
}

/** The held specimen: empty for an unassigned title-level hold. */
export function holdSpecimenItem(h: Hold): ItemShort | undefined {
  return h.biblio?.items?.[0];
}

export function holdPrimaryDocumentLabel(h: Hold): string {
  const title = h.biblio?.title?.trim();
  if (title) return title;
  const spec = holdSpecimenItem(h);
  return spec?.barcode?.trim() || spec?.callNumber?.trim() || h.itemId || h.biblioId;
}

/** Barcode / call number when a copy is pinned or allocated. */
export function holdSecondaryDocumentLabel(h: Hold): string | null {
  const spec = holdSpecimenItem(h);
  const sub = spec?.barcode?.trim() || spec?.callNumber?.trim() || h.itemId || null;
  return sub || null;
}

export function isHoldExpiringSoon(hold: Hold, nowMs: number = Date.now()): boolean {
  if (hold.status !== 'ready' || !hold.expiresAt) return false;
  const expires = new Date(hold.expiresAt).getTime();
  if (Number.isNaN(expires)) return false;
  return expires >= nowMs && expires - nowMs <= HOLD_EXPIRING_SOON_MS;
}

export function matchesStaffHoldFilter(
  hold: Hold,
  filter: StaffHoldStatusFilter,
  nowMs: number = Date.now(),
): boolean {
  switch (filter) {
    case 'ready':
      return hold.status === 'ready';
    case 'pending':
      return hold.status === 'pending';
    case 'expiringSoon':
      return isHoldExpiringSoon(hold, nowMs);
    case 'all':
    default:
      return true;
  }
}

/** Ready-to-shelf first, then soonest pickup deadline, then queue position. */
export function compareHoldsReadyFirst(a: Hold, b: Hold): number {
  const readyRank = (h: Hold) => (h.status === 'ready' ? 0 : 1);
  const ra = readyRank(a);
  const rb = readyRank(b);
  if (ra !== rb) return ra - rb;

  if (a.status === 'ready' && b.status === 'ready') {
    const ea = a.expiresAt ? new Date(a.expiresAt).getTime() : Number.POSITIVE_INFINITY;
    const eb = b.expiresAt ? new Date(b.expiresAt).getTime() : Number.POSITIVE_INFINITY;
    if (ea !== eb) return ea - eb;
  }

  if (a.position !== b.position) return a.position - b.position;
  return new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime();
}

export function applyStaffHoldList(
  holds: Hold[],
  filter: StaffHoldStatusFilter,
  nowMs: number = Date.now(),
): Hold[] {
  return holds.filter((h) => matchesStaffHoldFilter(h, filter, nowMs)).sort(compareHoldsReadyFirst);
}
