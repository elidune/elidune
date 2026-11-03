import type { QueryClient } from '@tanstack/react-query';
import { invalidateCatalogCaches } from '@/hooks/catalog/queryKeys';
import {
  DESK_PATRON_LOANS_QUERY_ROOT,
  MY_LOANS_QUERY_ROOT,
  OVERDUE_LOANS_QUERY_ROOT,
} from '@/hooks/loans/queryKeys';
import { USER_FINES_QUERY_KEY } from '@/hooks/users/useUserFinesQuery';

/** Refresh loan lists and catalog views after desk or patron circulation changes. */
export async function invalidateCirculationCaches(
  queryClient: QueryClient,
  userId?: string | null,
  options?: { biblioId?: string | null },
) {
  const tasks: Promise<unknown>[] = [
    queryClient.invalidateQueries({ queryKey: MY_LOANS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: OVERDUE_LOANS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ['loans-claims-returned'] }),
    queryClient.invalidateQueries({ queryKey: ['user-active-loans'] }),
    queryClient.invalidateQueries({ queryKey: ['user-past-loans'] }),
    queryClient.invalidateQueries({ queryKey: DESK_PATRON_LOANS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: [USER_FINES_QUERY_KEY] }),
    queryClient.invalidateQueries({ queryKey: ['activeHolds'] }),
    queryClient.invalidateQueries({ queryKey: ['biblioHolds'] }),
    queryClient.invalidateQueries({ queryKey: ['my-holds'] }),
    queryClient.invalidateQueries({ queryKey: ['user-holds'] }),
    queryClient.invalidateQueries({ queryKey: ['pickList'] }),
    invalidateCatalogCaches(queryClient, { biblioId: options?.biblioId }),
  ];
  if (userId) {
    tasks.push(queryClient.invalidateQueries({ queryKey: ['user-active-loans', userId] }));
    tasks.push(queryClient.invalidateQueries({ queryKey: ['user-past-loans', userId] }));
    tasks.push(queryClient.invalidateQueries({ queryKey: [USER_FINES_QUERY_KEY, userId] }));
    tasks.push(queryClient.invalidateQueries({ queryKey: ['user-holds', userId] }));
  }
  await Promise.all(tasks);
}
