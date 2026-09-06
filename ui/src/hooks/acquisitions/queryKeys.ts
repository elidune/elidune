import type { PurchaseOrderStatus } from '@/types';

export const ACQUISITIONS_VENDORS_KEY = ['acquisitions', 'vendors'] as const;
export const ACQUISITIONS_FUNDS_KEY = ['acquisitions', 'funds'] as const;
export const ACQUISITIONS_ORDERS_KEY = ['acquisitions', 'orders'] as const;

export function vendorsQueryKey(params: {
  q?: string;
  includeArchived?: boolean;
  page: number;
  perPage: number;
}) {
  return [...ACQUISITIONS_VENDORS_KEY, params] as const;
}

export function fundsQueryKey(params: {
  q?: string;
  fiscalYear?: number;
  page: number;
  perPage: number;
}) {
  return [...ACQUISITIONS_FUNDS_KEY, params] as const;
}

export function ordersQueryKey(params: {
  q?: string;
  status?: PurchaseOrderStatus;
  vendorId?: string;
  fundId?: string;
  page: number;
  perPage: number;
}) {
  return [...ACQUISITIONS_ORDERS_KEY, params] as const;
}

export function orderDetailQueryKey(id: string) {
  return [...ACQUISITIONS_ORDERS_KEY, id] as const;
}

export function orderAuditQueryKey(id: string) {
  return [...ACQUISITIONS_ORDERS_KEY, id, 'audit'] as const;
}
