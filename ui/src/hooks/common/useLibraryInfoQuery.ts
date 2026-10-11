import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { UpdateLibraryInfoRequest } from '@/types';

export const LIBRARY_INFO_QUERY_KEY = ['library-info'] as const;

export function useLibraryInfoQuery() {
  return useQuery({
    queryKey: LIBRARY_INFO_QUERY_KEY,
    queryFn: () => api.getLibraryInfo(),
  });
}

export function useUpdateLibraryInfoMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (body: UpdateLibraryInfoRequest) => api.updateLibraryInfo(body),
    onSuccess: (data) => {
      queryClient.setQueryData(LIBRARY_INFO_QUERY_KEY, data);
    },
  });
}
