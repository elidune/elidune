import type { HoldListItem, ItemTransit, Source, TransitPatron, TransitStatus } from '@/types';

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
  copySourceId?: string | null,
): boolean {
  if (hold.status !== 'pending') return false;
  if (!hold.pickupSiteId) return false;
  if (active && isActiveTransitStatus(active.status)) return false;
  const sourceId = clean(copySourceId) ?? clean(active?.sourceId);
  if (copyAlreadyAtPickupSite(hold.pickupSiteId, sourceId)) return false;
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

/**
 * Barcode for the existing check-in (`POST /loans/items/{barcode}/return`) after
 * `transit_same_site`. Only a barcode already on this screen qualifies. A missing
 * barcode, or any other error, does not offer set-aside.
 */
export function sameSiteSetAsideBarcode(
  errorCode: string | null | undefined,
  copies: Array<{ id?: string | null; barcode?: string | null }>,
  itemId?: string | null,
): string | null {
  if (errorCode !== 'transit_same_site') return null;
  const id = clean(itemId);
  if (!id) return null;
  const match = copies.find((copy) => clean(copy.id) === id);
  return clean(match?.barcode);
}

/** True when the copy's `sourceId` is the hold pickup site. Missing ids do not match. */
export function copyAlreadyAtPickupSite(
  pickupSiteId?: string | null,
  copySourceId?: string | null,
): boolean {
  const pickup = clean(pickupSiteId);
  const source = clean(copySourceId);
  if (!pickup || !source) return false;
  return pickup === source;
}

/** Barcode on the transit. No item-id fallback. Missing on older servers. */
export function transitListBarcode(transit: ItemTransit): string | null {
  return clean(transit.barcode);
}

/** Bibliographic title on the transit. */
export function transitListTitle(transit: ItemTransit): string | null {
  return clean(transit.title);
}

/**
 * Reader cell for a transit. Card number and initials only.
 * Null patron, a blank patron, or initials that contain a space become an empty cell.
 */
export function transitListPatron(transit: ItemTransit): TransitPatron | null {
  const patron = transit.patron;
  if (!patron) return null;
  const cardNumber = clean(patron.cardNumber);
  const initials = clean(patron.initials)?.replace(/\./g, '') ?? '';
  if (!cardNumber && !initials) return null;
  if (/\s/u.test(initials)) return cardNumber ? { cardNumber, initials: '' } : null;
  return { cardNumber, initials };
}

function clean(value: string | null | undefined): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed || null;
}
