import type { QueryClient } from '@tanstack/react-query';
import {
  DESK_PATRON_LOANS_QUERY_ROOT,
  MY_LOANS_QUERY_ROOT,
  OVERDUE_LOANS_QUERY_ROOT,
} from '@/hooks/loans/queryKeys';

/** Refresh shared loan lists after desk or patron circulation changes. */
export async function invalidateCirculationCaches(queryClient: QueryClient, userId?: string | null) {
  const tasks: Promise<void>[] = [
    queryClient.invalidateQueries({ queryKey: MY_LOANS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: OVERDUE_LOANS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ['loans-claims-returned'] }),
    queryClient.invalidateQueries({ queryKey: ['user-active-loans'] }),
    queryClient.invalidateQueries({ queryKey: ['user-past-loans'] }),
    queryClient.invalidateQueries({ queryKey: DESK_PATRON_LOANS_QUERY_ROOT }),
  ];
  if (userId) {
    tasks.push(queryClient.invalidateQueries({ queryKey: ['user-active-loans', userId] }));
    tasks.push(queryClient.invalidateQueries({ queryKey: ['user-past-loans', userId] }));
  }
  await Promise.all(tasks);
}
