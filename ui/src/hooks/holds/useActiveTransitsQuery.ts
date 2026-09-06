import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import type { ItemTransit } from '@/types';

export const ACTIVE_TRANSITS_QUERY_KEY = ['activeTransits'] as const;

async function fetchActiveTransits(): Promise<ItemTransit[]> {
  const [requested, inTransit] = await Promise.all([
    api.listTransits({ status: 'requested', page: 1, perPage: 200 }),
    api.listTransits({ status: 'inTransit', page: 1, perPage: 200 }),
  ]);
  return [...requested.items, ...inTransit.items];
}

export function useActiveTransitsQuery(enabled = true) {
  return useQuery({
    queryKey: ACTIVE_TRANSITS_QUERY_KEY,
    queryFn: fetchActiveTransits,
    enabled,
    staleTime: 30 * 1000,
  });
}
