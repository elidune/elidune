import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import { fundsQueryKey } from './queryKeys';

export function useFundsQuery(params: {
  q?: string;
  fiscalYear?: number;
  page: number;
  perPage: number;
}) {
  return useQuery({
    queryKey: fundsQueryKey(params),
    queryFn: () =>
      api.getFunds({
        q: params.q || undefined,
        fiscalYear: params.fiscalYear,
        page: params.page,
        perPage: params.perPage,
      }),
  });
}

export function useFundsPickerQuery(enabled = true) {
  return useQuery({
    queryKey: [...fundsQueryKey({ page: 1, perPage: 200 }), 'picker'],
    queryFn: () => api.getFunds({ page: 1, perPage: 200 }),
    enabled,
    staleTime: 60_000,
  });
}
