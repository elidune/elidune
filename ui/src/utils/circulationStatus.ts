import type { ItemShort, Loan } from '@/types';

/** Item-level circulation_status SMALLINT (PR #36). */
export const CIRCULATION_STATUS = {
  AVAILABLE: 0,
  LOST: 1,
  DAMAGED: 2,
  CLAIMED_RETURNED: 3,
  /** Missing from the shelf (`CirculationStatus::ToSearch`). */
  TO_SEARCH: 4,
} as const;

export function loanBorrowedItem(loan: Loan): ItemShort | null {
  const items = loan.biblio?.items;
  if (!items?.length) return null;
  return items.find((s) => s.borrowed) ?? items[0] ?? null;
}

/**
 * Shelf-search state. Items persist it as SMALLINT `4`.
 * The circulation enum's JSON name is `toSearch` when the status itself is serialized.
 */
export function isItemToSearch(item?: { circulationStatus?: number | string | null } | null): boolean {
  const value = item?.circulationStatus;
  if (typeof value === 'number') return value === CIRCULATION_STATUS.TO_SEARCH;
  if (typeof value !== 'string') return false;
  const trimmed = value.trim();
  if (trimmed === String(CIRCULATION_STATUS.TO_SEARCH)) return true;
  return trimmed.toLowerCase().replace(/[_\-\s]/g, '') === 'tosearch';
}

export function isClaimedReturnedLoan(loan: { id: string }, claimedLoanIds?: ReadonlySet<string>): boolean {
  // Embedded list copies (`ItemShortStaff`) do not include circulation status.
  // The claims queue is the source for this flag.
  return claimedLoanIds?.has(loan.id) ?? false;
}
