import { useState } from 'react';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import type { Source } from '@/types';
import { defaultPickupSiteId, pickupSiteRequired } from '@/utils/transitDisplay';

const EMPTY_SOURCES: Source[] = [];

export function usePickupSiteDraft(active: boolean) {
  const sourcesQuery = useSourcesQuery(true);
  const sources = sourcesQuery.data ?? EMPTY_SOURCES;
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

  const sourceKey = sources.map((s) => s.id).join(',');
  if (active && sources.length > 0 && !pickupSiteId && seededKey !== sourceKey) {
    setPickupSiteId(defaultPickupSiteId(sources));
    setSeededKey(sourceKey);
  }

  const required = pickupSiteRequired(sources);
  const missing = required && !pickupSiteId.trim();

  return { sources, pickupSiteId, setPickupSiteId, required, missing, sourcesQuery };
}
