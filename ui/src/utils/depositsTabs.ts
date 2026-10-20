export type DepositsTab = 'batches' | 'lenders';

export function isDepositsTab(value: string | null): value is DepositsTab {
  return value === 'batches' || value === 'lenders';
}
