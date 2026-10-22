import { useMutation, useQuery, useQueryClient, type QueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import { invalidateCirculationCaches } from '@/hooks/loans/invalidateCirculationCaches';
import { myLoansQueryKey, MY_LOANS_QUERY_ROOT } from '@/hooks/loans/queryKeys';
import { newIdempotencyKey } from '@/utils/idempotency';

const DEFAULT_STALE_MS = 60 * 1000;

export function useMyLoansQuery(
  userId: string | undefined,
  params: { page: number; perPage: number; archived?: boolean },
  options?: { enabled?: boolean; staleTime?: number },
) {
  return useQuery({
    queryKey: myLoansQueryKey(userId, params),
    queryFn: () =>
      api.getMyLoans({
        page: params.page,
        perPage: params.perPage,
        archived: params.archived,
      }),
    enabled: Boolean(userId) && (options?.enabled ?? true),
    staleTime: options?.staleTime ?? DEFAULT_STALE_MS,
  });
}

export function useRenewMyLoanMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (loanId: string) => api.renewLoan(loanId, newIdempotencyKey()),
    onSuccess: async () => {
      await invalidateCirculationCaches(queryClient);
    },
  });
}

export function invalidateMyLoansQueries(queryClient: QueryClient) {
  return queryClient.invalidateQueries({ queryKey: MY_LOANS_QUERY_ROOT });
}
