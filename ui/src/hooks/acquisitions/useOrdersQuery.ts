import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import type { PurchaseOrderStatus } from '@/types';
import { orderAuditQueryKey, orderDetailQueryKey, ordersQueryKey } from './queryKeys';

export function useOrdersQuery(params: {
  q?: string;
  status?: PurchaseOrderStatus;
  vendorId?: string;
  fundId?: string;
  page: number;
  perPage: number;
}) {
  return useQuery({
    queryKey: ordersQueryKey(params),
    queryFn: () =>
      api.getPurchaseOrders({
        q: params.q || undefined,
        status: params.status,
        vendorId: params.vendorId || undefined,
        fundId: params.fundId || undefined,
        page: params.page,
        perPage: params.perPage,
      }),
  });
}

export function useOrderDetailQuery(id: string | undefined) {
  return useQuery({
    queryKey: orderDetailQueryKey(id ?? ''),
    queryFn: () => api.getPurchaseOrder(id!),
    enabled: Boolean(id),
  });
}

export function useOrderAuditQuery(id: string | undefined, enabled = true) {
  return useQuery({
    queryKey: orderAuditQueryKey(id ?? ''),
    queryFn: () => api.getPurchaseOrderAudit(id!),
    enabled: Boolean(id) && enabled,
  });
}
