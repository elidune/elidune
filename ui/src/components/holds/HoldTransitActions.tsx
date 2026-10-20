import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Package, PackageCheck, Truck } from 'lucide-react';
import { Button } from '@/components/common';
import api from '@/services/api';
import type { HoldListItem, ItemTransit } from '@/types';
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
  hold: HoldListItem;
  transit?: ItemTransit | null;
  actions: ReturnType<typeof useTransitActions>;
}) {
  const { t } = useTranslation();
  const { busyKey, isLoading, open } = actions;
  const startBusy = busyKey === `start:${hold.id}`;
  const shipBusy = transit ? busyKey === `ship:${transit.id}` : false;
  const receiveBusy = transit ? busyKey === `receive:${transit.id}` : false;
  const awaitingTransfer = Boolean(transit && (canShipTransit(transit) || canReceiveTransit(transit)));
  const resolveCopySite =
    hold.status === 'pending' && !!hold.pickupSiteId && !!hold.itemId && !awaitingTransfer;
  const siteQuery = useQuery({
    queryKey: ['biblio', hold.biblioId],
    queryFn: () => api.getBiblio(hold.biblioId),
    enabled: resolveCopySite && !!hold.biblioId && !transit?.siteId,
    staleTime: 60 * 1000,
  });
  const copySourceId =
    transit?.siteId ??
    siteQuery.data?.items.find((item) => item.id === hold.itemId)?.siteId;

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

  if (resolveCopySite && siteQuery.isLoading) return null;

  if (canStartHoldTransit(hold, transit, copySourceId)) {
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
