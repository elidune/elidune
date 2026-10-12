import type { ItemCopyBorrower } from '@/types';
import type { BorrowerRef } from '@/utils/readerIdentifier';

export interface CopyCirculationFields {
  borrowed?: boolean | null;
  availability?: string | null;
  dueDate?: string | null;
  loanId?: string | null;
  borrower?: ItemCopyBorrower | null;
}

export function isCopyBorrowed(copy: CopyCirculationFields): boolean {
  return copy.borrowed === true || copy.availability === 'onLoan';
}

export function isCopyOnHold(copy: CopyCirculationFields): boolean {
  return copy.availability === 'onHold';
}

/** Card or initials from the item payload. An id alone is not an identifier. */
export function itemBorrowerIdentity(
  borrower: ItemCopyBorrower | null | undefined,
): BorrowerRef | null {
  if (!borrower) return null;
  const card = borrower.cardNumber?.trim() || null;
  const initials = borrower.initials?.trim() || null;
  if (!card && !initials) return null;
  return { id: borrower.id ?? null, cardNumber: card, initials };
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
