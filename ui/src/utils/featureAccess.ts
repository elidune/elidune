import type { EnabledFeatures } from '@/types';
import { staffAccess, type AccessUser, type StaffAccess } from '@/utils/staffAccess';

/** Feature keys exposed on `library-info.enabledFeatures` (camelCase). */
export type FeatureKey = keyof EnabledFeatures;

const ALL_ENABLED: EnabledFeatures = {
  acquisitions: true,
  purchaseSuggestions: true,
  weeding: true,
  transits: true,
  deposits: true,
  events: true,
  inventory: true,
  z3950: true,
  importIso: true,
  stats: true,
  visitorCounts: true,
  serials: true,
};

/** Normalize partial/absent `enabledFeatures` — missing keys default to enabled (compat). */
export function resolveEnabledFeatures(
  features: Partial<EnabledFeatures> | null | undefined,
): EnabledFeatures {
  if (!features) return { ...ALL_ENABLED };
  return {
    acquisitions: features.acquisitions ?? true,
    purchaseSuggestions: features.purchaseSuggestions ?? true,
    weeding: features.weeding ?? true,
    transits: features.transits ?? true,
    deposits: features.deposits ?? true,
    events: features.events ?? true,
    inventory: features.inventory ?? true,
    z3950: features.z3950 ?? true,
    importIso: features.importIso ?? true,
    stats: features.stats ?? true,
    visitorCounts: features.visitorCounts ?? true,
    serials: features.serials ?? true,
  };
}

export function isFeatureEnabled(
  features: Partial<EnabledFeatures> | null | undefined,
  key: FeatureKey,
): boolean {
  return resolveEnabledFeatures(features)[key];
}

/**
 * Staff menu/route flags AND instance feature flags.
 * A module must be enabled for the whole library and allowed by RBAC.
 */
export function effectiveStaffAccess(
  user: AccessUser,
  authToken: string | null | undefined,
  features: Partial<EnabledFeatures> | null | undefined,
): StaffAccess {
  const access = staffAccess(user, authToken);
  const f = resolveEnabledFeatures(features);
  return {
    ...access,
    acquisitions: access.acquisitions && f.acquisitions,
    serials: access.serials && f.serials,
    weeding: access.weeding && f.weeding,
    holdsStaff: access.holdsStaff, // pick-list stays; transits nav gated separately
    stats: access.stats && f.stats,
    homeStats: access.homeStats && f.stats,
    backgroundTasks: access.backgroundTasks,
  };
}

/** Re-export for callers that need the raw staffAccess user type. */
export type { AccessUser };
