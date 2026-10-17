export const ACQUISITIONS_TAB_IDS = ['orders', 'vendors', 'funds', 'suggestions'] as const;

export type AcquisitionsTab = (typeof ACQUISITIONS_TAB_IDS)[number];

export function isAcquisitionsTab(value: string | null): value is AcquisitionsTab {
  return value != null && ACQUISITIONS_TAB_IDS.includes(value as AcquisitionsTab);
}

export function acquisitionsTabFromLegacyPath(segment: string): AcquisitionsTab | null {
  return isAcquisitionsTab(segment) ? segment : null;
}
