import { useTranslation } from 'react-i18next';
import { Package, PackageCheck, Truck } from 'lucide-react';
import { Button } from '@/components/common';
import type { Hold, ItemTransit } from '@/types';
import type { useTransitActions } from '@/hooks/holds/useTransitActions';
import {
  canReceiveTransit,
  canShipTransit,
  canStartHoldTransit,
} from '@/utils/transitDisplay';

export default function HoldTransitActions({
  hold,
  transit,
  actions,
}: {
  hold: Hold;
  transit?: ItemTransit | null;
  actions: ReturnType<typeof useTransitActions>;
}) {
  const { t } = useTranslation();
  const { busyKey, isLoading, open } = actions;
  const startBusy = busyKey === `start:${hold.id}`;
  const shipBusy = transit ? busyKey === `ship:${transit.id}` : false;
  const receiveBusy = transit ? busyKey === `receive:${transit.id}` : false;

  if (transit && canShipTransit(transit)) {
    return (
      <Button
        size="sm"
        variant="secondary"
        leftIcon={<Truck className="h-4 w-4" />}
        isLoading={shipBusy}
        disabled={isLoading}
        onClick={() => open({ kind: 'ship', transit })}
      >
        {t('holds.shipTransit')}
      </Button>
    );
  }

  if (transit && canReceiveTransit(transit)) {
    return (
      <Button
        size="sm"
        variant="primary"
        leftIcon={<PackageCheck className="h-4 w-4" />}
        isLoading={receiveBusy}
        disabled={isLoading}
        onClick={() => open({ kind: 'receive', transit })}
      >
        {t('holds.receiveTransit')}
      </Button>
    );
  }

  if (canStartHoldTransit(hold, transit)) {
    return (
      <Button
        size="sm"
        variant="secondary"
        leftIcon={<Package className="h-4 w-4" />}
        isLoading={startBusy}
        disabled={isLoading}
        onClick={() => open({ kind: 'start', hold })}
      >
        {t('holds.startTransit')}
      </Button>
    );
  }

  return null;
}
