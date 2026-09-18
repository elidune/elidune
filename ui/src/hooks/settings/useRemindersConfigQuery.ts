import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import { parseRemindersConfig } from '@/utils/reminderPolicy';
import type { RemindersConfig } from '@/types';

export const REMINDERS_CONFIG_QUERY_KEY = ['admin', 'config', 'reminders'] as const;

export type RemindersConfigSection = {
  config: RemindersConfig;
  raw: Record<string, unknown>;
  overridable: boolean;
  overridden: boolean;
};

export function useRemindersConfigQuery() {
  return useQuery({
    queryKey: REMINDERS_CONFIG_QUERY_KEY,
    queryFn: async (): Promise<RemindersConfigSection> => {
      const data = await api.getAdminConfig();
      const section = data.sections.find((s) => s.key === 'reminders');
      const raw = section?.value ?? {};
      return {
        config: parseRemindersConfig(raw),
        raw,
        overridable: section?.overridable ?? true,
        overridden: section?.overridden ?? false,
      };
    },
  });
}

export function useUpdateRemindersConfigMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (value: Record<string, unknown>) => api.putAdminConfigSection('reminders', value),
    onSuccess: (updated) => {
      queryClient.setQueryData(REMINDERS_CONFIG_QUERY_KEY, {
        config: parseRemindersConfig(updated.value),
        raw: updated.value,
        overridable: updated.overridable,
        overridden: updated.overridden,
      } satisfies RemindersConfigSection);
    },
  });
}
