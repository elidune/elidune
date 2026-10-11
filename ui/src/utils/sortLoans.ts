/** Stable sort: oldest borrow date first. */
export function sortLoansByStartDateAsc<T extends { id: string; startDate: string }>(loans: T[]): T[] {
  return [...loans].sort((a, b) => {
    const c = a.startDate.localeCompare(b.startDate);
    return c !== 0 ? c : a.id.localeCompare(b.id);
  });
}
