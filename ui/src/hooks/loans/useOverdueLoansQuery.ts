import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import { overdueLoansQueryKey } from '@/hooks/loans/queryKeys';

export function useOverdueLoansQuery(
  page: number,
  perPage: number,
  enabled: boolean,
) {
  return useQuery({
    queryKey: overdueLoansQueryKey(page, perPage),
    queryFn: async () => {
      const data = await api.getOverdueLoans({ page, perPage });
      return { loans: data.loans, total: Number(data.total) };
    },
    enabled,
    staleTime: 30 * 1000,
  });
}
