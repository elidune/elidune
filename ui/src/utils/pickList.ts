import type { HoldPickListRow } from '@/types';
import { minimizedHoldFor } from '@/utils/holdPatron';

function text(value: unknown): string | null {
  if (value == null) return null;
  const s = String(value).trim();
  return s || null;
}

function record(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

/**
 * Map one pick-list JSON object onto the desk row.
 * `holdFor` is stored minimized. A name on the payload is discarded.
 */
export function normalizePickListRow(raw: unknown): HoldPickListRow | null {
  const row = record(raw);
  if (!row) return null;
  const holdId = text(row.holdId) ?? text(row.id);
  const itemId = text(row.itemId);
  const nested = record(row.holdFor);
  const holdFor = minimizedHoldFor(nested, text(row.userId));
  if (!holdId || !itemId || !holdFor) return null;

  const pickDueAt = text(row.pickDueAt);
  let overdue = row.overdue === true;
  if (row.overdue == null && pickDueAt) {
    const dueMs = Date.parse(pickDueAt);
    if (!Number.isNaN(dueMs)) overdue = dueMs < Date.now();
  }

  return {
    holdId,
    itemId,
    barcode: text(row.barcode),
    callNumber: text(row.callNumber),
    title: text(row.title),
    userId: holdFor.id,
    holdFor,
    pickupSiteId: text(row.pickupSiteId),
    pickupSiteName: text(row.pickupSiteName),
    pickStartedAt: text(row.pickStartedAt),
    pickDueAt,
    pickupDeadline: text(row.pickupDeadline),
    overdue,
  };
}

/** Call number order. Rows without a call number stay at the end. */
export function sortPickListByCallNumber(rows: HoldPickListRow[]): HoldPickListRow[] {
  return [...rows].sort((a, b) => {
    const ca = a.callNumber ?? '';
    const cb = b.callNumber ?? '';
    if (!ca && cb) return 1;
    if (ca && !cb) return -1;
    const cmp = ca.localeCompare(cb, undefined, { numeric: true, sensitivity: 'base' });
    if (cmp !== 0) return cmp;
    return a.holdId.localeCompare(b.holdId);
  });
}

export function normalizePickList(raw: unknown): HoldPickListRow[] {
  const list = Array.isArray(raw) ? raw : [];
  const rows: HoldPickListRow[] = [];
  for (const item of list) {
    const row = normalizePickListRow(item);
    if (row) rows.push(row);
  }
  return sortPickListByCallNumber(rows);
}

/** Empty value: `GET /holds/pick-list` with no `siteId` (every site). */
export const PICK_LIST_ALL_SITES = '';

const PICK_LIST_SITE_STORAGE_KEY = 'elidune.pickList.siteId';

type SiteStorage = Pick<Storage, 'getItem' | 'setItem'>;

/** Last explicit site, including "" for all sites. Null when the agent has not chosen. */
export function readPickListSiteChoice(storage: SiteStorage | null): string | null {
  if (!storage) return null;
  try {
    return storage.getItem(PICK_LIST_SITE_STORAGE_KEY);
  } catch {
    return null;
  }
}

/** Remember the select value. "" is all sites and must be stored, not removed. */
export function writePickListSiteChoice(storage: SiteStorage | null, siteId: string): void {
  if (!storage) return;
  try {
    storage.setItem(PICK_LIST_SITE_STORAGE_KEY, siteId);
  } catch {
    // Private mode or a full quota must not block the desk.
  }
}

/**
 * Site filter for the pick list.
 * No stored choice means every site. A remembered id is kept only while that
 * site is still in the list. The library default site is not applied: the
 * server drops copies whose source is null when `siteId` is set.
 */
export function resolvePickListSiteId(
  sources: ReadonlyArray<{ id: string }>,
  stored: string | null,
): string {
  if (stored == null || stored.trim() === '') return PICK_LIST_ALL_SITES;
  const id = stored.trim();
  if (sources.some((site) => site.id === id)) return id;
  return PICK_LIST_ALL_SITES;
}

/** `undefined` omits `siteId` on `GET /holds/pick-list`. */
export function pickListRequestSiteId(siteId: string): string | undefined {
  const trimmed = siteId.trim();
  return trimmed || undefined;
}
