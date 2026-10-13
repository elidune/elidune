import type { LoanBorrowerMinimized } from '@/types';
import { borrowerInitials } from '@/utils/deskScanAlert';
import {
  shelfIdentifierText,
  type BorrowerRef,
} from '@/utils/readerIdentifier';

type Translate = (key: string, values?: Record<string, unknown>) => string;

/** Card phrase used on hold screens. `{{number}}` is the library card. */
export function holdCardPhrase(number: string, t: Translate): string {
  return t('holds.readerCardNo', { number });
}

/**
 * Patron text for a printed slip or an open-shelf label.
 * Card number, otherwise initials. The name is never included.
 */
export function shelfPatronLabel(
  user: BorrowerRef | null | undefined,
  t: Translate,
): string {
  return shelfIdentifierText(user, (number) => holdCardPhrase(number, t));
}

/**
 * `item_to_pick_for_other` warning. Card phrase, otherwise initials.
 * A name is not accepted: the 422 body does not include one.
 */
export function toPickPatronPhrase(
  input: { cardNumber?: string | null; initials?: string | null } | null | undefined,
  t: Translate,
): string {
  const card = input?.cardNumber?.trim();
  if (card) return holdCardPhrase(card, t);
  const initials = input?.initials?.trim().replace(/\./g, '') ?? '';
  if (initials && !/\s/u.test(initials)) return initials;
  return t('loans.pickPatronUnknown');
}

/** Keep only the shelf fields. Names on a nested object are dropped. */
export function minimizedHoldFor(
  source: Record<string, unknown> | null | undefined,
  fallbackId?: string | null,
): LoanBorrowerMinimized | null {
  const id = text(source?.id) ?? text(fallbackId);
  if (!id) return null;
  const cardNumber = text(source?.cardNumber) ?? text(source?.barcode);
  const explicit = compactInitials(text(source?.initials));
  const derived =
    borrowerInitials({
      firstname: text(source?.firstname),
      lastname: text(source?.lastname),
    }) ?? '';
  return { id, cardNumber, initials: explicit || derived };
}

function text(value: unknown): string | null {
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed || null;
}

function compactInitials(value: string | null): string {
  if (!value || /\s/u.test(value)) return '';
  return [...value].filter((char) => char !== '.').join('');
}
