/**
 * Borrower shown on circulation screens.
 *
 * Full contact (`GET /loans/overdue` with users read): `firstname`, `lastname`,
 * `email`, `phone` — email and phone may be null.
 * Staff profile (`UserShortStaff`): name and `barcode`, no email or phone.
 * Minimized (`LoanBorrowerMinimized`): `id`, `cardNumber`, `initials` only.
 */
export interface BorrowerRef {
  id?: string | null;
  firstname?: string | null;
  lastname?: string | null;
  email?: string | null;
  phone?: string | null;
  login?: string | null;
  barcode?: string | null;
  cardNumber?: string | null;
  initials?: string | null;
  accountType?: string | null;
}

const NOMINATIVE_KEYS = ['firstname', 'lastname', 'email', 'phone', 'login', 'barcode', 'accountType'] as const;

function hasOwn(user: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(user, key);
}

function clean(value: string | null | undefined): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

/** True when the payload is `{ id, cardNumber, initials }` and nothing nominative. */
export function isMinimizedBorrower(user: object | null | undefined): boolean {
  if (!user) return false;
  if (NOMINATIVE_KEYS.some((key) => hasOwn(user, key))) return false;
  return hasOwn(user, 'cardNumber') || hasOwn(user, 'initials');
}

export function borrowerDisplayName(user: BorrowerRef | null | undefined): string | null {
  if (!user || isMinimizedBorrower(user)) return null;
  const name = [clean(user.firstname), clean(user.lastname)].filter(Boolean).join(' ');
  return name || null;
}

/** Library card used for the minimized body, else the staff `barcode`. */
export function borrowerCardNumber(user: BorrowerRef | null | undefined): string | null {
  if (!user) return null;
  if (isMinimizedBorrower(user)) return clean(user.cardNumber);
  return clean(user.cardNumber) ?? clean(user.barcode);
}

/**
 * Visible identifier.
 * Minimized: card phrase when a card exists, otherwise server initials (no dots).
 * Full: display name. A card phrase is used only when the name is blank.
 */
export function readerIdentifierText(
  user: BorrowerRef | null | undefined,
  cardLabel: (number: string) => string,
): string {
  if (!user) return '—';
  if (isMinimizedBorrower(user)) {
    const card = clean(user.cardNumber);
    if (card) return cardLabel(card);
    const initials = clean(user.initials)?.replace(/\./g, '') ?? '';
    return initials || '—';
  }
  const name = borrowerDisplayName(user);
  if (name) return name;
  const card = clean(user.barcode) ?? clean(user.cardNumber);
  if (card) return cardLabel(card);
  return '—';
}

/** Avatar letters. Minimized rows use server initials and never a full name. */
export function borrowerAvatarInitials(user: BorrowerRef | null | undefined): string {
  if (!user) return '?';
  if (isMinimizedBorrower(user)) {
    const initials = clean(user.initials)?.replace(/\./g, '') ?? '';
    return initials || '?';
  }
  const letters = `${clean(user.firstname)?.[0] ?? ''}${clean(user.lastname)?.[0] ?? ''}`.toUpperCase();
  return letters || '?';
}

/**
 * Open-shelf label and printed hold slip.
 * Card number when one exists, otherwise initials. A name is never returned.
 */
export function shelfIdentifierText(
  user: BorrowerRef | null | undefined,
  cardLabel: (number: string) => string,
): string {
  if (!user) return '—';
  const card = clean(user.cardNumber) ?? clean(user.barcode);
  if (card) return cardLabel(card);
  const explicit = compactInitials(user.initials);
  if (explicit) return explicit;
  const derived = `${clean(user.firstname)?.[0] ?? ''}${clean(user.lastname)?.[0] ?? ''}`.toUpperCase();
  return derived || '—';
}

function compactInitials(value: string | null | undefined): string | null {
  const text = clean(value);
  if (!text || /\s/u.test(text)) return null;
  const compact = [...text].filter((char) => char !== '.').join('');
  return compact || null;
}

/**
 * What an agent may render for another reader.
 * With users read, the payload is unchanged.
 * Without it, only `id`, `cardNumber` (or the staff card stored in `barcode`),
 * and server `initials` remain. Names are not turned into initials.
 */
export function borrowerShownToAgent(
  user: BorrowerRef | null | undefined,
  revealIdentity: boolean,
): BorrowerRef | null | undefined {
  if (!user || revealIdentity) return user;
  const card = clean(user.cardNumber) ?? clean(user.barcode);
  const initials = clean(user.initials)?.replace(/\./g, '') || null;
  return { id: user.id ?? null, cardNumber: card, initials };
}

/** Email and phone only exist on the full overdue borrower. Blank values are omitted. */
export function borrowerContact(user: BorrowerRef | null | undefined): {
  email: string | null;
  phone: string | null;
} {
  if (!user || isMinimizedBorrower(user)) return { email: null, phone: null };
  return { email: clean(user.email), phone: clean(user.phone) };
}
