import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';

export function useSourcesQuery(enabled = true) {
  return useQuery({
    queryKey: ['sources', false],
    queryFn: () => api.getSources(false),
    enabled,
    staleTime: 5 * 60 * 1000,
  });
}
