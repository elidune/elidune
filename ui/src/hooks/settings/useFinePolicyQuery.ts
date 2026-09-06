import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { CirculationFinePolicy } from '@/types';

export const FINE_POLICY_QUERY_KEY = ['fines', 'policy'] as const;

export function useFinePolicyQuery() {
  return useQuery({
    queryKey: FINE_POLICY_QUERY_KEY,
    queryFn: () => api.getFinePolicy(),
  });
}

export function useUpdateFinePolicyMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (policy: CirculationFinePolicy) => api.updateFinePolicy(policy),
    onSuccess: (data) => {
      queryClient.setQueryData(FINE_POLICY_QUERY_KEY, data);
    },
  });
}
