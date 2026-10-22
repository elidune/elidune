import type { QueryClient } from '@tanstack/react-query';

export const USERS_LIST_QUERY_ROOT = ['users'] as const;
export const userDetailQueryKey = (userId: string) => ['user', userId] as const;

export function invalidateUsersListQueries(queryClient: QueryClient) {
  return queryClient.invalidateQueries({ queryKey: USERS_LIST_QUERY_ROOT });
}
