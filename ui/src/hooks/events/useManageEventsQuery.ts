import { useQuery, type QueryClient } from '@tanstack/react-query';
import api from '@/services/api';

export const MANAGE_EVENTS_QUERY_ROOT = ['events', 'manage'] as const;

export function manageEventsQueryKey(showPast: boolean, page: number, perPage: number) {
  return [...MANAGE_EVENTS_QUERY_ROOT, { showPast, page, perPage }] as const;
}

function todayStr() {
  return new Date().toISOString().slice(0, 10);
}

function yesterdayStr() {
  const d = new Date();
  d.setDate(d.getDate() - 1);
  return d.toISOString().slice(0, 10);
}

export function useManageEventsQuery(showPast: boolean, page: number, perPage: number) {
  return useQuery({
    queryKey: manageEventsQueryKey(showPast, page, perPage),
    queryFn: async () => {
      const params = showPast
        ? { endDate: yesterdayStr(), page, perPage }
        : { startDate: todayStr(), page, perPage };
      return api.getEvents(params);
    },
    staleTime: 60 * 1000,
  });
}

export async function invalidateEventListCaches(queryClient: QueryClient) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: MANAGE_EVENTS_QUERY_ROOT }),
    queryClient.invalidateQueries({ queryKey: ['public-events'] }),
    queryClient.invalidateQueries({ queryKey: ['app-public-events'] }),
    queryClient.invalidateQueries({ queryKey: ['public-events-login'] }),
  ]);
}
