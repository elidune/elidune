import type { QueryClient } from '@tanstack/react-query';

/** Staff catalog list (`BibliosPage` infinite query). */
export const BIBLIOS_LIST_QUERY_ROOT = ['biblios'] as const;

/** Staff biblio detail (`BiblioDetailPage`). */
export const BIBLIO_DETAIL_QUERY_ROOT = ['biblio'] as const;

/** Public OPAC search and detail. */
export const OPAC_BIBLIOS_QUERY_ROOT = ['opac-biblios'] as const;
export const OPAC_BIBLIO_QUERY_ROOT = ['opac-biblio'] as const;

/** Authenticated reader catalog. */
export const READER_CATALOG_QUERY_ROOT = ['reader-catalog'] as const;

/** Incoming deposit batches list. */
export const DEPOSIT_BATCHES_QUERY_ROOT = ['deposit-batches'] as const;

/** One deposit batch detail + its item lists. */
export const DEPOSIT_BATCH_QUERY_ROOT = ['deposit-batch'] as const;

/** Deposit copies declared lost or damaged. */
export const DEPOSIT_LOSSES_QUERY_ROOT = ['deposit-losses'] as const;

/** Home dashboard counters that include availability. */
export const HOME_STATS_QUERY_ROOT = ['home-stats'] as const;

/** Weeding candidate / withdrawal queues. */
export const WEEDING_CANDIDATES_QUERY_ROOT = ['weeding-candidates'] as const;
export const ITEM_WITHDRAWALS_QUERY_ROOT = ['item-withdrawals'] as const;

/**
 * Refresh catalog, OPAC, deposits, and related item views after a copy
 * changes state (lost, missing, found, weeding, loan, deposit scan, …).
 */
export async function invalidateCatalogCaches(
  queryClient: QueryClient,
  options?: { biblioId?: string | null },
): Promise<void> {
  const tasks: Promise<unknown>[] = [
    queryClient.invalidateQueries({ queryKey: BIBLIOS_LIST_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: BIBLIO_DETAIL_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: OPAC_BIBLIOS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: OPAC_BIBLIO_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: READER_CATALOG_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: DEPOSIT_BATCHES_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: DEPOSIT_BATCH_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: DEPOSIT_LOSSES_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: HOME_STATS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: WEEDING_CANDIDATES_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ITEM_WITHDRAWALS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ['item-withdrawal'] }),
    queryClient.invalidateQueries({ queryKey: ['inventory-expand'] }),
  ];
  const biblioId = options?.biblioId?.trim();
  if (biblioId) {
    tasks.push(queryClient.invalidateQueries({ queryKey: [...BIBLIO_DETAIL_QUERY_ROOT, biblioId] }));
  }
  await Promise.all(tasks);
}
