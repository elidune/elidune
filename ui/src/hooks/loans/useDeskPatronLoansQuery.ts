import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import { deskPatronLoansQueryKey } from '@/hooks/loans/queryKeys';
import { currentUserLoansQuery, readPatronLoans } from '@/utils/loanListAccess';
import type { LoanListItem } from '@/types';

export type DeskPatronLoansResult =
  | { status: 'ok'; items: LoanListItem[]; total: number }
  | { status: 'denied'; items: []; total: 0 };

export function useDeskPatronLoansQuery(
  userId: string | undefined,
  page: number,
  perPage: number,
  enabled: boolean,
) {
  return useQuery({
    queryKey: userId ? deskPatronLoansQueryKey(userId, page, perPage) : ['desk-patron-loans', 'none'],
    queryFn: async (): Promise<DeskPatronLoansResult> => {
      const result = await readPatronLoans(() =>
        api.getUserLoans(userId!, currentUserLoansQuery(page, perPage)),
      );
      if (result.ok) {
        return { status: 'ok', items: result.page.items as LoanListItem[], total: result.page.total };
      }
      if (result.denied) {
        return { status: 'denied', items: [], total: 0 };
      }
      throw result.error;
    },
    enabled: Boolean(userId) && enabled,
    staleTime: 15 * 1000,
  });
}
