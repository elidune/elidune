import type { MoneyAmount, PurchaseOrderLine, PurchaseOrderStatus } from '@/types';

export function parseMoney(amount: MoneyAmount | null | undefined): number | null {
  if (amount == null || amount === '') return null;
  const n = typeof amount === 'number' ? amount : Number(String(amount).replace(',', '.'));
  return Number.isFinite(n) ? n : null;
}

export function formatMoney(
  amount: MoneyAmount | null | undefined,
  currency: string | null | undefined,
  locale: string,
): string {
  const n = parseMoney(amount);
  if (n == null) return '—';
  const code = (currency ?? '').trim().toUpperCase() || 'EUR';
  try {
    return new Intl.NumberFormat(locale, { style: 'currency', currency: code }).format(n);
  } catch {
    return `${n} ${code}`;
  }
}

export function quantityRemaining(line: Pick<PurchaseOrderLine, 'quantityOrdered' | 'quantityReceived'>): number {
  return Math.max(0, (line.quantityOrdered ?? 0) - (line.quantityReceived ?? 0));
}

export function orderStatusBadgeVariant(
  status: PurchaseOrderStatus,
): 'default' | 'success' | 'warning' | 'danger' | 'info' {
  if (status === 'received') return 'success';
  if (status === 'partial' || status === 'ordered') return 'info';
  if (status === 'cancelled') return 'danger';
  if (status === 'draft') return 'warning';
  return 'default';
}

export function canEditOrder(status: PurchaseOrderStatus): boolean {
  return status === 'draft';
}

export function canSubmitOrder(status: PurchaseOrderStatus): boolean {
  return status === 'draft';
}

export function canReceiveOrder(status: PurchaseOrderStatus): boolean {
  return status === 'ordered' || status === 'partial';
}

export function canCancelOrder(status: PurchaseOrderStatus): boolean {
  return status === 'draft' || status === 'ordered';
}

export function lineIntentLabel(
  line: Pick<PurchaseOrderLine, 'title' | 'isbn' | 'biblioId'>,
): string {
  const title = line.title?.trim();
  if (title) return title;
  const isbn = line.isbn?.trim();
  if (isbn) return isbn;
  if (line.biblioId) return `#${line.biblioId}`;
  return '—';
}

export const ORDER_STATUSES: PurchaseOrderStatus[] = [
  'draft',
  'ordered',
  'partial',
  'received',
  'cancelled',
];
