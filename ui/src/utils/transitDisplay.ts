import type { HoldListItem, ItemTransit, LoanBorrowerMinimized, Source, TransitStatus } from '@/types';
import { minimizedHoldFor } from '@/utils/holdPatron';

export function isActiveTransitStatus(status: TransitStatus): boolean {
  return status === 'requested' || status === 'inTransit';
}

export function sourceLabel(sources: Source[] | undefined, id?: string | null): string | null {
  if (!id) return null;
  const match = sources?.find((s) => s.id === id);
  const name = match?.name?.trim();
  return name || id;
}

export function defaultPickupSiteId(sources: Source[]): string {
  if (sources.length === 0) return '';
  return sources.find((s) => s.default)?.id ?? sources[0].id;
}

/** Multi-site desks must choose a pickup site; a single site is pre-selected. */
export function pickupSiteRequired(sources: Source[]): boolean {
  return sources.length >= 2;
}

export function indexTransitsByHoldId(transits: ItemTransit[]): Map<string, ItemTransit> {
  const map = new Map<string, ItemTransit>();
  for (const transit of transits) {
    if (transit.holdId && isActiveTransitStatus(transit.status)) {
      map.set(transit.holdId, transit);
    }
  }
  return map;
}

/** Pending hold with a pickup site and no active transfer — staff may move a copy. */
export function canStartHoldTransit(
  hold: HoldListItem,
  active?: ItemTransit | null,
  sources: readonly Source[] = [],
): boolean {
  if (hold.status !== 'pending') return false;
  if (!hold.pickupSiteId) return false;
  if (active && isActiveTransitStatus(active.status)) return false;
  if (copyAlreadyAtPickupSite(hold, allocatedCopySourceId(hold, sources))) return false;
  return true;
}

export function canShipTransit(transit: ItemTransit): boolean {
  return transit.status === 'requested';
}

export function canReceiveTransit(transit: ItemTransit): boolean {
  return transit.status === 'inTransit';
}

export function canCancelTransit(transit: ItemTransit): boolean {
  return isActiveTransitStatus(transit.status);
}

/** Hide “start transit” when the allocated copy is already at the pickup site. */
export function copyAlreadyAtPickupSite(
  hold: HoldListItem,
  copySourceId?: string | null,
): boolean {
  if (!hold.pickupSiteId || !copySourceId) return false;
  return hold.pickupSiteId === copySourceId;
}

/**
 * Site of the allocated copy.
 * The hold list sends `sourceName` on the embedded copy, not `sourceId`.
 * A unique site name from `GET /sources` is the id. Ambiguous or missing names stay unknown.
 */
export function allocatedCopySourceId(
  hold: HoldListItem,
  sources: readonly Source[],
): string | null {
  const name = allocatedCopy(hold)?.sourceName?.trim();
  if (!name) return null;
  const matches = sources.filter((source) => source.name?.trim() === name);
  return matches.length === 1 ? matches[0].id : null;
}

/** Barcode of the allocated copy (`biblio.items[].barcode`). No item-id fallback. */
export function transitListBarcode(hold: HoldListItem | null | undefined): string | null {
  const barcode = allocatedCopy(hold)?.barcode?.trim();
  return barcode || null;
}

/** Bibliographic title (`biblio.title`). */
export function transitListTitle(hold: HoldListItem | null | undefined): string | null {
  const title = hold?.biblio?.title?.trim();
  return title || null;
}

/**
 * Reader for a transit row: card number and initials only.
 * A staff hold may include the name; it is dropped here.
 */
export function transitListReader(
  hold: HoldListItem | null | undefined,
): LoanBorrowerMinimized | null {
  if (!hold?.user || typeof hold.user !== 'object') return null;
  return minimizedHoldFor(hold.user as unknown as Record<string, unknown>, hold.userId);
}

function allocatedCopy(hold: HoldListItem | null | undefined) {
  if (!hold?.itemId) return undefined;
  const items = hold.biblio?.items ?? [];
  return items.find((item) => item.id === hold.itemId) ?? items[0];
}
