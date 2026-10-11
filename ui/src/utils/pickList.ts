import type { HoldPickListRow } from '@/types';

function text(value: unknown): string | null {
  if (value == null) return null;
  const s = String(value).trim();
  return s || null;
}

function personName(row: Record<string, unknown>): string | null {
  const direct = text(row.userName) ?? text(row.patronName);
  if (direct) return direct;
  const joined = [text(row.patronFirstname), text(row.patronLastname)].filter(Boolean).join(' ').trim();
  return joined || null;
}

/** Map one pick-list JSON object onto the desk row. */
export function normalizePickListRow(raw: unknown): HoldPickListRow | null {
  if (!raw || typeof raw !== 'object') return null;
  const row = raw as Record<string, unknown>;
  const holdId = text(row.holdId) ?? text(row.id);
  const itemId = text(row.itemId);
  const userId = text(row.userId);
  if (!holdId || !itemId || !userId) return null;

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
    userId,
    userName: personName(row) ?? userId,
    pickupSiteId: text(row.pickupSiteId),
    pickupSiteName: text(row.pickupSiteName),
    pickStartedAt: text(row.pickStartedAt),
    pickDueAt,
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
