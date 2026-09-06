import { useEffect, useState } from 'react';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import { defaultPickupSiteId, pickupSiteRequired } from '@/utils/transitDisplay';

export function usePickupSiteDraft(active: boolean) {
  const sourcesQuery = useSourcesQuery(true);
  const sources = sourcesQuery.data ?? [];
  const [pickupSiteId, setPickupSiteId] = useState('');

  useEffect(() => {
    if (!active) {
      setPickupSiteId('');
      return;
    }
    if (sources.length === 0) return;
    setPickupSiteId((current) => current || defaultPickupSiteId(sources));
  }, [active, sources]);

  const required = pickupSiteRequired(sources);
  const missing = required && !pickupSiteId.trim();

  return { sources, pickupSiteId, setPickupSiteId, required, missing, sourcesQuery };
}
