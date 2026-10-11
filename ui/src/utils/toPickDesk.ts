import axios from 'axios';
import type { ItemCheckIn } from '@/types';

export const ITEM_TO_PICK_FOR_OTHER = 'item_to_pick_for_other';

/** Thrown when staff dismiss the force-loan dialog. Callers must not toast it. */
export class CheckoutDeclined extends Error {
  constructor() {
    super('checkout-declined');
    this.name = 'CheckoutDeclined';
  }
}

export function isCheckoutDeclined(error: unknown): boolean {
  return error instanceof CheckoutDeclined;
}

export interface ItemToPickForOther {
  holdId: string | null;
  userId: string | null;
  userName: string | null;
}

function text(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  const s = String(value).trim();
  return s || null;
}

function readName(source: Record<string, unknown>): string | null {
  return text(source.userName) ?? text(source.patronName);
}

/** `item_to_pick_for_other` body, or null for every other error. */
export function readItemToPickForOther(error: unknown): ItemToPickForOther | null {
  if (!axios.isAxiosError(error)) return null;
  const data = error.response?.data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) return null;
  const body = data as Record<string, unknown>;
  if (text(body.code) !== ITEM_TO_PICK_FOR_OTHER) return null;
  const nested =
    body.details && typeof body.details === 'object' && !Array.isArray(body.details)
      ? (body.details as Record<string, unknown>)
      : {};
  return {
    holdId: text(body.holdId) ?? text(nested.holdId),
    userId: text(body.userId) ?? text(nested.userId),
    userName: readName(body) ?? readName(nested),
  };
}

export interface PickCheckInNotice {
  kind: 'ready' | 'transit';
  patronName: string | null;
  siteName: string | null;
  userId: string | null;
  siteId: string | null;
}

function holdRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function embeddedUserName(hold: Record<string, unknown> | null): string | null {
  const user = holdRecord(hold?.user);
  if (!user) return null;
  const joined = [text(user.firstname), text(user.lastname)].filter(Boolean).join(' ').trim();
  return joined || null;
}

/**
 * Pick completion on the existing return path.
 * `ready` puts the copy on the hold shelf. Transit sends it to the pickup site.
 * A closed loan (`returned`) is not a pick.
 */
export function interpretPickCheckIn(body: ItemCheckIn | null | undefined): PickCheckInNotice | null {
  if (!body) return null;
  const status = body.status?.trim() ?? '';
  if (status === 'returned' || body.loan) return null;
  const hold = holdRecord(body.hold);
  const transit =
    body.transitStarted === true || status === 'in_transit' || status === 'inTransit';
  const ready = status === 'ready';
  if (!ready && !transit) return null;
  return {
    kind: transit ? 'transit' : 'ready',
    patronName: text(body.userName) ?? text(body.patronName) ?? embeddedUserName(hold),
    siteName: text(body.destinationSiteName) ?? text(body.pickupSiteName) ?? text(hold?.pickupSiteName),
    userId: text(body.userId) ?? text(hold?.userId),
    siteId:
      text(body.destinationSiteId) ??
      text(body.pickupSiteId) ??
      text(hold?.pickupSiteId),
  };
}
