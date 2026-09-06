import { useTranslation } from 'react-i18next';
import type { Hold, ItemTransit, Source } from '@/types';
import { sourceLabel } from '@/utils/transitDisplay';
import TransitStatusBadge from '@/components/holds/TransitStatusBadge';

export default function HoldPickupCell({
  hold,
  sources,
  transit,
  showTransit = true,
}: {
  hold: Hold;
  sources?: Source[];
  transit?: ItemTransit | null;
  showTransit?: boolean;
}) {
  const { t } = useTranslation();
  const site = sourceLabel(sources, hold.pickupSiteId);

  return (
    <div className="space-y-1 min-w-0">
      <div className="text-sm text-gray-800 dark:text-gray-200 truncate">
        {site ?? t('holds.transitNone')}
      </div>
      {showTransit && transit && <TransitStatusBadge status={transit.status} />}
    </div>
  );
}
