import { useQuery } from '@tanstack/react-query';
import api from '@/services/api';
import type { PurchaseSuggestionStatus } from '@/types';
import { suggestionsQueryKey } from './queryKeys';

export function useSuggestionsQuery(params: {
  status?: PurchaseSuggestionStatus;
  page: number;
  perPage: number;
}) {
  return useQuery({
    queryKey: suggestionsQueryKey(params),
    queryFn: () =>
      api.getPurchaseSuggestions({
        status: params.status,
        page: params.page,
        perPage: params.perPage,
      }),
  });
}
