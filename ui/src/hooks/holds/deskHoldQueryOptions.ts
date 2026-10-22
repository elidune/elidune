/** Staff hold desk lists: always refetch when opening the page (no stale 5m default). */
export const deskHoldListQueryOptions = {
  staleTime: 0,
  refetchOnMount: 'always' as const,
};

export const PICK_LIST_QUERY_ROOT = ['pickList'] as const;
export const ACTIVE_HOLDS_QUERY_ROOT = ['activeHolds'] as const;
