/** Copy-level status derived only from existing OPAC/item fields. */
export type OpacCopyStatus = 'available' | 'borrowed' | 'notForLoan' | 'unavailable';
export type AvailabilityBadgeVariant = 'default' | 'success' | 'warning' | 'danger' | 'info';

export interface OpacCopyLike {
  borrowable?: boolean | null;
  borrowed?: boolean;
  callNumber?: string | null;
  sourceName?: string | null;
}

export function copyAvailabilityStatus(item: OpacCopyLike): OpacCopyStatus {
  if (item.borrowed) return 'borrowed';
  if (item.borrowable === false) return 'notForLoan';
  if (item.borrowable === true) return 'available';
  return 'unavailable';
}

/** Title-level rollup: prefer any available copy, then on loan, then not-for-loan. */
export function titleAvailabilityStatus(items?: OpacCopyLike[] | null): OpacCopyStatus {
  if (!items?.length) return 'unavailable';
  const statuses = items.map(copyAvailabilityStatus);
  if (statuses.some((s) => s === 'available')) return 'available';
  if (statuses.some((s) => s === 'borrowed')) return 'borrowed';
  if (statuses.some((s) => s === 'notForLoan')) return 'notForLoan';
  return 'unavailable';
}

export function availabilityLabelKey(status: OpacCopyStatus): string {
  switch (status) {
    case 'available':
      return 'opac.available';
    case 'borrowed':
      return 'opac.borrowed';
    case 'notForLoan':
      return 'opac.notForLoan';
    default:
      return 'opac.unavailable';
  }
}

export function availabilityBadgeVariant(status: OpacCopyStatus): AvailabilityBadgeVariant {
  switch (status) {
    case 'available':
      return 'success';
    case 'borrowed':
      return 'danger';
    case 'notForLoan':
      return 'warning';
    default:
      return 'default';
  }
}

/** Prefer an available copy’s call number / site; otherwise the first copy with data. */
export function primaryCopyLocation(items?: OpacCopyLike[] | null): {
  callNumber: string | null;
  sourceName: string | null;
} {
  if (!items?.length) return { callNumber: null, sourceName: null };
  const preferred =
    items.find((i) => copyAvailabilityStatus(i) === 'available') ??
    items.find((i) => i.callNumber || i.sourceName) ??
    items[0];
  const callNumber = preferred?.callNumber?.trim() || null;
  const sourceName = preferred?.sourceName?.trim() || null;
  return { callNumber, sourceName };
}

export function uniqueCopySites(items?: OpacCopyLike[] | null): string[] {
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
