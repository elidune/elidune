import type { Hold, ItemTransit, Source, TransitStatus } from '@/types';

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
export function canStartHoldTransit(hold: Hold, active?: ItemTransit | null): boolean {
  if (hold.status !== 'pending') return false;
  if (!hold.pickupSiteId) return false;
  if (active && isActiveTransitStatus(active.status)) return false;
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
  hold: Hold,
  copySourceId?: string | null,
): boolean {
  if (!hold.pickupSiteId || !copySourceId) return false;
  return hold.pickupSiteId === copySourceId;
}
