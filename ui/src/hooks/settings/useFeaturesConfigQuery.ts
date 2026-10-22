import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import type { FeaturesConfig } from '@/types';

export const FEATURES_CONFIG_QUERY_KEY = ['admin', 'config', 'features'] as const;

const FEATURE_KEYS = [
  'acquisitions',
  'purchase_suggestions',
  'weeding',
  'transits',
  'deposits',
  'events',
  'inventory',
  'z3950',
  'import_iso',
  'stats',
  'visitor_counts',
] as const;

export type FeatureConfigKey = (typeof FEATURE_KEYS)[number];

export { FEATURE_KEYS };

export type FeaturesConfigSection = {
  config: FeaturesConfig;
  raw: Record<string, unknown>;
  overridable: boolean;
  overridden: boolean;
};

function parseBool(value: unknown, fallback = true): boolean {
  if (typeof value === 'boolean') return value;
  return fallback;
}

export function parseFeaturesConfig(raw: Record<string, unknown>): FeaturesConfig {
  return {
    overridable: parseBool(raw.overridable, true),
    profile: typeof raw.profile === 'string' ? raw.profile : null,
    acquisitions: parseBool(raw.acquisitions),
    purchase_suggestions: parseBool(raw.purchase_suggestions),
    weeding: parseBool(raw.weeding),
    transits: parseBool(raw.transits),
    deposits: parseBool(raw.deposits),
    events: parseBool(raw.events),
    inventory: parseBool(raw.inventory),
    z3950: parseBool(raw.z3950),
    import_iso: parseBool(raw.import_iso),
    stats: parseBool(raw.stats),
    visitor_counts: parseBool(raw.visitor_counts),
  };
}

export function useFeaturesConfigQuery() {
  return useQuery({
    queryKey: FEATURES_CONFIG_QUERY_KEY,
    queryFn: async (): Promise<FeaturesConfigSection> => {
      const data = await api.getAdminConfig();
      const section = data.sections.find((s) => s.key === 'features');
      const raw = section?.value ?? {};
      return {
        config: parseFeaturesConfig(raw),
        raw,
        overridable: section?.overridable ?? true,
        overridden: section?.overridden ?? false,
      };
    },
  });
}

export function useUpdateFeaturesConfigMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (value: Record<string, unknown>) => api.putAdminConfigSection('features', value),
    onSuccess: (updated) => {
      queryClient.setQueryData(FEATURES_CONFIG_QUERY_KEY, {
        config: parseFeaturesConfig(updated.value),
        raw: updated.value,
        overridable: updated.overridable,
        overridden: updated.overridden,
      } satisfies FeaturesConfigSection);
      void queryClient.invalidateQueries({ queryKey: ['library-info'] });
    },
  });
}

export function useResetFeaturesConfigMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.deleteAdminConfigSection('features'),
    onSuccess: (updated) => {
      queryClient.setQueryData(FEATURES_CONFIG_QUERY_KEY, {
        config: parseFeaturesConfig(updated.value),
        raw: updated.value,
        overridable: updated.overridable,
        overridden: updated.overridden,
      } satisfies FeaturesConfigSection);
      void queryClient.invalidateQueries({ queryKey: ['library-info'] });
    },
  });
}
