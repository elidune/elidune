import type { MoneyAmount, PurchaseOrderLine, PurchaseOrderStatus } from '@/types';

export function parseMoney(amount: MoneyAmount | null | undefined): number | null {
  if (amount == null || amount === '') return null;
  const n = typeof amount === 'number' ? amount : Number(String(amount).replace(',', '.'));
  return Number.isFinite(n) ? n : null;
}

/** Normalize a form amount for the API (`12,50` → `12.50`). Empty → null. */
export function moneyInputToApi(raw: string | null | undefined): string | null {
  const s = (raw ?? '').trim().replace(/\s/g, '').replace(',', '.');
  if (!s) return null;
  return Number.isFinite(Number(s)) ? s : null;
}

/** True when a draft line PUT cannot clear fields the API treats as “leave unchanged”. */
export function lineUpdateNeedsRecreate(
  initial: Pick<PurchaseOrderLine, 'biblioId' | 'isbn' | 'title' | 'fundId'>,
  next: { biblioId?: string | null; isbn?: string | null; title?: string | null; fundId?: string | null },
): boolean {
  if (initial.biblioId && !next.biblioId) return true;
  if (initial.isbn?.trim() && !next.isbn?.trim()) return true;
  if (initial.title?.trim() && !next.title?.trim()) return true;
  if (initial.fundId && !next.fundId) return true;
  return false;
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
