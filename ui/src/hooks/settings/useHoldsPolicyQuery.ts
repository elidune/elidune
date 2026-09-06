import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { HoldsPolicy } from '@/types';

export const HOLDS_POLICY_QUERY_KEY = ['holds', 'policy'] as const;

export function useHoldsPolicyQuery() {
  return useQuery({
    queryKey: HOLDS_POLICY_QUERY_KEY,
    queryFn: () => api.getHoldsPolicy(),
  });
}

export function useUpdateHoldsPolicyMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (policy: HoldsPolicy) => api.updateHoldsPolicy(policy),
    onSuccess: (data) => {
      queryClient.setQueryData(HOLDS_POLICY_QUERY_KEY, data);
    },
  });
}
