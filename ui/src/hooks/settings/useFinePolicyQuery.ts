import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { FineRule } from '@/types';
import { isDefaultFineRule, type GlobalUnpaidThresholdBody, type UpsertFineRuleBody } from '@/utils/finePolicy';

export const FINE_POLICY_QUERY_KEY = ['fines', 'policy'] as const;
export const FINE_RULES_QUERY_KEY = ['fines', 'rules'] as const;

export function useFinePolicyQuery() {
  return useQuery({
    queryKey: FINE_POLICY_QUERY_KEY,
    queryFn: () => api.getFinePolicy(),
  });
}

export function useUpdateFinePolicyMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (policy: GlobalUnpaidThresholdBody) => api.updateFinePolicy(policy),
    onSuccess: (data) => {
      queryClient.setQueryData(FINE_POLICY_QUERY_KEY, data);
    },
  });
}

export function useFineRulesQuery() {
  return useQuery({
    queryKey: FINE_RULES_QUERY_KEY,
    queryFn: () => api.getFineRules(),
  });
}

export function useUpsertFineRuleMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: UpsertFineRuleBody) => api.upsertFineRule(body),
    onSuccess: (rule) => {
      queryClient.setQueryData<FineRule[]>(FINE_RULES_QUERY_KEY, (prev) => {
        const list = prev ?? [];
        const index = list.findIndex((row) => isDefaultFineRule(row));
        if (index < 0) return [...list, rule];
        const next = list.slice();
        next[index] = rule;
        return next;
      });
    },
  });
}
