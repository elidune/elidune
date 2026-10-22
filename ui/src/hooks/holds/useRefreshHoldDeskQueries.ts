import type { QueryClient } from '@tanstack/react-query';
import { ACTIVE_HOLDS_QUERY_ROOT, PICK_LIST_QUERY_ROOT } from '@/hooks/holds/deskHoldQueryOptions';
import { ACTIVE_TRANSITS_QUERY_KEY } from '@/hooks/holds/useActiveTransitsQuery';
import { invalidateCirculationCaches } from '@/hooks/loans/invalidateCirculationCaches';

/** Pick check-in, transits, and hold lists after a shelf scan. */
export async function refreshHoldDeskQueries(queryClient: QueryClient, userId?: string | null) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: PICK_LIST_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ACTIVE_HOLDS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ACTIVE_TRANSITS_QUERY_KEY }),
    invalidateCirculationCaches(queryClient, userId),
  ]);
}
