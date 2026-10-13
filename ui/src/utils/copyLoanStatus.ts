import type { BorrowerRef } from '@/utils/readerIdentifier';

export interface CopyCirculationFields {
  borrowed?: boolean | null;
  availability?: string | null;
  dueDate?: string | null;
  /** Copy is assigned to a hold, or in transit for one. */
  onHold?: boolean | null;
  loanId?: string | null;
  borrower?: BorrowerRef | null;
}

export function isCopyBorrowed(copy: CopyCirculationFields): boolean {
  return copy.borrowed === true || copy.availability === 'onLoan';
}

export function isCopyOnHold(copy: CopyCirculationFields): boolean {
  return copy.availability === 'onHold';
}

/** Card, initials, or name from the item payload. An id alone is not an identifier. */
export function itemBorrowerIdentity(
  borrower: BorrowerRef | null | undefined,
): BorrowerRef | null {
  if (!borrower) return null;
  const card = borrower.cardNumber?.trim() || borrower.barcode?.trim() || null;
  const initials = borrower.initials?.trim() || null;
  const firstname = borrower.firstname?.trim() || null;
  const lastname = borrower.lastname?.trim() || null;
  if (!card && !initials && !firstname && !lastname) return null;
  return {
    id: borrower.id ?? null,
    cardNumber: borrower.cardNumber ?? null,
    initials: borrower.initials ?? null,
    firstname,
    lastname,
    barcode: borrower.barcode ?? null,
    login: borrower.login ?? null,
  };
}

export function formatCopyDueDate(iso: string | null | undefined, locale: string): string | null {
  if (!iso?.trim()) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleDateString(locale, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}
