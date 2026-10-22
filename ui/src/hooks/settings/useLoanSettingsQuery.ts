import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { UpdateLoanSettingsRequest } from '@/types';

export const LOAN_SETTINGS_QUERY_KEY = ['loan-settings'] as const;

export function useLoanSettingsQuery(enabled = true) {
  return useQuery({
    queryKey: LOAN_SETTINGS_QUERY_KEY,
    queryFn: () => api.getLoanSettings(),
    enabled,
    select: (data) => data.loanSettings ?? [],
  });
}

export function useUpdateLoanSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateLoanSettingsRequest) => api.updateLoanSettings(body),
    onSuccess: (data) => {
      queryClient.setQueryData(LOAN_SETTINGS_QUERY_KEY, data);
    },
  });
}
