import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';

export function useSitesQuery(enabled = true) {
  return useQuery({
    queryKey: ['sites', false],
    queryFn: () => api.getSites(false),
    enabled,
    staleTime: 5 * 60 * 1000,
  });
}
