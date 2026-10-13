import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';

/** Staff reader record. `staleTime: 0` so a lost-found settlement shows on the next visit. */
export const USER_FINES_QUERY_KEY = 'user-fines';

export function useUserFinesQuery(userId: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: [USER_FINES_QUERY_KEY, userId],
    queryFn: () => api.getUserFines(userId!),
    enabled: Boolean(userId) && enabled,
    staleTime: 0,
  });
}
