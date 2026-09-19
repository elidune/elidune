import type { BiblioWeedingStatus, ItemWeedingStatus } from '@/types';

export const ITEM_WEEDING_STATUSES: readonly ItemWeedingStatus[] = [
  'onShelf',
  'candidate',
  'withdrawn',
];

/** Exact server 422 messages from POST checkout / renew / hold when withdrawn. */
export const WEEDING_SERVER_MESSAGES = {
  checkout: 'Item is withdrawn and cannot be checked out',
  renew: 'Item is withdrawn and cannot be renewed',
  hold: 'Item is withdrawn and cannot be placed on hold',
  titleHold: 'No circulable copies remain for this title',
} as const;

export function itemWeedingStatus(item?: { weedingStatus?: ItemWeedingStatus | null } | null): ItemWeedingStatus {
  return item?.weedingStatus ?? 'onShelf';
}

export function isItemWithdrawn(item?: { weedingStatus?: ItemWeedingStatus | null } | null): boolean {
  return itemWeedingStatus(item) === 'withdrawn';
}

export function isBiblioWithdrawn(biblio?: { weedingStatus?: BiblioWeedingStatus | null } | null): boolean {
  return biblio?.weedingStatus === 'withdrawn';
}

/** Title-level offer check: derived field, or every remaining copy withdrawn. */
export function isTitleWithdrawn(biblio?: {
  weedingStatus?: BiblioWeedingStatus | null;
  items?: Array<{ weedingStatus?: ItemWeedingStatus | null }> | null;
} | null): boolean {
  if (isBiblioWithdrawn(biblio)) return true;
  const items = biblio?.items;
  return !!items?.length && items.every((item) => isItemWithdrawn(item));
}

/** Candidate stays offerable; withdrawn copies are not offered for checkout or hold. */
export function isCopyOfferable(item?: {
  weedingStatus?: ItemWeedingStatus | null;
  borrowable?: boolean | null;
} | null): boolean {
  if (!item || isItemWithdrawn(item)) return false;
  return item.borrowable !== false;
}

export function isCopyAvailable(item?: {
  weedingStatus?: ItemWeedingStatus | null;
  borrowable?: boolean | null;
  borrowed?: boolean;
} | null): boolean {
  if (!item || isItemWithdrawn(item) || item.borrowed) return false;
  return item.borrowable === true;
}

export function weedingServerMessageI18nKey(raw: string | null | undefined): string | null {
  if (!raw) return null;
  switch (raw.trim()) {
    case WEEDING_SERVER_MESSAGES.checkout:
      return 'weeding.blockedCheckout';
    case WEEDING_SERVER_MESSAGES.renew:
      return 'weeding.blockedRenew';
    case WEEDING_SERVER_MESSAGES.hold:
      return 'weeding.blockedHold';
    case WEEDING_SERVER_MESSAGES.titleHold:
      return 'weeding.blockedTitleHold';
    default:
      return null;
  }
}

export function isWeedingServerMessage(raw: string | null | undefined): boolean {
  return weedingServerMessageI18nKey(raw) != null;
}
