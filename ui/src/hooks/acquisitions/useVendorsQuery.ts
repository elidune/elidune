import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import { vendorsQueryKey } from './queryKeys';

export function useVendorsQuery(params: {
  q?: string;
  includeArchived?: boolean;
  page: number;
  perPage: number;
}) {
  return useQuery({
    queryKey: vendorsQueryKey(params),
    queryFn: () =>
      api.getVendors({
        q: params.q || undefined,
        includeArchived: params.includeArchived,
        page: params.page,
        perPage: params.perPage,
      }),
  });
}

export function useActiveVendorsQuery(enabled = true) {
  return useQuery({
    queryKey: [...vendorsQueryKey({ page: 1, perPage: 200, includeArchived: false }), 'picker'],
    queryFn: () => api.getVendors({ includeArchived: false, page: 1, perPage: 200 }),
    enabled,
    staleTime: 60_000,
  });
}
