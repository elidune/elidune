import { useQuery, type QueryClient } from '@tanstack/react-query';
import api from '@/services/api';

export const SITES_QUERY_ROOT = ['sites'] as const;

export function sitesQueryKey(includeArchived: boolean) {
  return [...SITES_QUERY_ROOT, includeArchived] as const;
}

export function useSitesQuery(includeArchived = false, enabled = true) {
  return useQuery({
    queryKey: sitesQueryKey(includeArchived),
    queryFn: () => api.getSites(includeArchived),
    enabled,
    staleTime: 5 * 60 * 1000,
  });
}

export function invalidateSitesQueries(queryClient: QueryClient) {
  return queryClient.invalidateQueries({ queryKey: SITES_QUERY_ROOT });
}
