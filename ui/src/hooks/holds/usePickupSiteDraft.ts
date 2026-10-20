import { useState } from 'react';
import { useSitesQuery } from '@/hooks/holds/useSitesQuery';
import type { Site } from '@/types';
import { defaultPickupSiteId, pickupSiteRequired } from '@/utils/transitDisplay';

const EMPTY_SOURCES: Site[] = [];

export function usePickupSiteDraft(active: boolean) {
  const sitesQuery = useSitesQuery(true);
  const sites = sitesQuery.data ?? EMPTY_SOURCES;
  const [pickupSiteId, setPickupSiteId] = useState('');
  const [wasActive, setWasActive] = useState(active);
  const [seededKey, setSeededKey] = useState('');

  if (active !== wasActive) {
    setWasActive(active);
    if (!active) {
      setPickupSiteId('');
      setSeededKey('');
    }
  }

  const siteKey = sites.map((s) => s.id).join(',');
  if (active && sites.length > 0 && !pickupSiteId && seededKey !== siteKey) {
    setPickupSiteId(defaultPickupSiteId(sites));
    setSeededKey(siteKey);
  }

  const required = pickupSiteRequired(sites);
  const missing = required && !pickupSiteId.trim();

  return { sites, pickupSiteId, setPickupSiteId, required, missing, sitesQuery };
}
