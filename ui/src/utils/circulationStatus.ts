import type { ItemShort, Loan } from '@/types';

/** Item-level circulation_status SMALLINT (PR #36). */
export const CIRCULATION_STATUS = {
  AVAILABLE: 0,
  LOST: 1,
  DAMAGED: 2,
  CLAIMED_RETURNED: 3,
} as const;

export function loanBorrowedItem(loan: Loan): ItemShort | null {
  const items = loan.biblio?.items;
  if (!items?.length) return null;
  return items.find((s) => s.borrowed) ?? items[0] ?? null;
}

export function isClaimedReturnedLoan(loan: Loan, claimedLoanIds?: ReadonlySet<string>): boolean {
  if (claimedLoanIds?.has(loan.id)) return true;
  const spec = loanBorrowedItem(loan);
  return spec?.circulationStatus === CIRCULATION_STATUS.CLAIMED_RETURNED;
}
