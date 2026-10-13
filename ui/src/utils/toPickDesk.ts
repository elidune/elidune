import axios from 'axios';
import type { ItemCheckIn } from '@/types';
import { deskScanAlert, type DeskCopySnapshot } from '@/utils/deskScanAlert';
import {
  borrowerCardNumber,
  borrowerDisplayName,
  isMinimizedBorrower,
  type BorrowerRef,
} from '@/utils/readerIdentifier';

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

/** Identity for the force-loan dialog. A name is set only for a full hold profile. */
export interface ToPickForceIdentity {
  patronName: string | null;
  cardNumber: string | null;
  initials: string | null;
}

/**
 * Name when `holdFor` is a users-read profile. Otherwise the card, then initials.
 * A name on the 422 body is never accepted.
 */
export function toPickForceIdentity(input: {
  holdFor?: BorrowerRef | null;
  cardNumber?: string | null;
  initials?: string | null;
}): ToPickForceIdentity {
  const holdFor = input.holdFor ?? null;
  if (holdFor && !isMinimizedBorrower(holdFor)) {
    const name = borrowerDisplayName(holdFor);
    if (name) return { patronName: name, cardNumber: null, initials: null };
  }
  return {
    patronName: null,
    cardNumber: (holdFor ? borrowerCardNumber(holdFor) : null) ?? text(input.cardNumber),
    initials: text(holdFor?.initials) ?? text(input.initials),
  };
}

/**
 * Checkout pre-check: the copy is on hold for someone else.
 * Same reader and an on-loan copy are not this dialog.
 */
export function toPickForceFromDeskCopy(
  copy: DeskCopySnapshot | null | undefined,
  patronId?: string | null,
): ToPickForceIdentity | null {
  if (!copy) return null;
  const alert = deskScanAlert(copy, { patronId });
  if (alert?.kind !== 'onHold') return null;
  return toPickForceIdentity({ holdFor: copy.holdFor });
}

/** 422 `item_to_pick_for_other`, with the desk copy's holdFor when the lookup has it. */
export function toPickForceFromError(
  error: unknown,
  holdFor?: BorrowerRef | null,
): ToPickForceIdentity | null {
  const parsed = readItemToPickForOther(error);
  if (!parsed) return null;
  return toPickForceIdentity({
    holdFor,
    cardNumber: parsed.cardNumber,
    initials: parsed.initials,
  });
}

export interface ItemToPickForOther {
  holdId: string | null;
  userId: string | null;
  cardNumber: string | null;
  initials: string | null;
}

function text(value: unknown): string | null {
  if (typeof value !== 'string' && typeof value !== 'number') return null;
  const s = String(value).trim();
  return s || null;
}

/** `item_to_pick_for_other` body, or null for every other error. Names are ignored. */
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
    holdId: text(nested.holdId) ?? text(body.holdId),
    userId: text(nested.userId) ?? text(body.userId),
    cardNumber: text(nested.cardNumber) ?? text(body.cardNumber),
    initials: text(nested.initials) ?? text(body.initials),
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
