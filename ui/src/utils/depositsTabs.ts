export type DepositsTab = 'batches' | 'lenders' | 'losses';

export function isDepositsTab(value: string | null): value is DepositsTab {
  return value === 'batches' || value === 'lenders' || value === 'losses';
}
