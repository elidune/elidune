import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import { useToast } from '@/contexts/ToastContext';
import { getApiErrorMessage } from '@/utils/apiError';
import type { CreateTransit, Hold, ItemTransit } from '@/types';
import { ACTIVE_TRANSITS_QUERY_KEY } from '@/hooks/holds/useActiveTransitsQuery';

export type PendingTransitAction =
  | { kind: 'start'; hold: Hold }
  | { kind: 'ship'; transit: ItemTransit }
  | { kind: 'receive'; transit: ItemTransit }
  | { kind: 'cancel'; transit: ItemTransit };

function busyKeyFor(pending: PendingTransitAction): string {
  if (pending.kind === 'start') return `start:${pending.hold.id}`;
  return `${pending.kind}:${pending.transit.id}`;
}

export function useTransitActions() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const inFlightRef = useRef(false);
  const [pending, setPending] = useState<PendingTransitAction | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busyKey, setBusyKey] = useState<string | null>(null);

  const invalidate = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ACTIVE_TRANSITS_QUERY_KEY });
    void queryClient.invalidateQueries({ queryKey: ['transits'] });
    void queryClient.invalidateQueries({ queryKey: ['activeHolds'] });
    void queryClient.invalidateQueries({ queryKey: ['biblioHolds'] });
    void queryClient.invalidateQueries({ queryKey: ['my-holds'] });
    void queryClient.invalidateQueries({ queryKey: ['user-holds'] });
  }, [queryClient]);

  const open = useCallback((next: PendingTransitAction) => {
    if (inFlightRef.current) return;
    setError(null);
    setPending(next);
  }, []);

  const close = useCallback(() => {
    if (inFlightRef.current) return;
    setPending(null);
    setError(null);
  }, []);

  const run = useCallback(
    async (fn: () => Promise<void>, successKey: string) => {
      if (!pending || inFlightRef.current) return;
      inFlightRef.current = true;
      setBusyKey(busyKeyFor(pending));
      setIsLoading(true);
      setError(null);
      try {
        await fn();
        showToast({ variant: 'success', message: t(successKey) });
        setPending(null);
        invalidate();
      } catch (err: unknown) {
        const msg = getApiErrorMessage(err, t) || t('holds.transitActionError');
        setError(msg);
        showToast({ variant: 'error', message: msg });
      } finally {
        inFlightRef.current = false;
        setBusyKey(null);
        setIsLoading(false);
      }
    },
    [invalidate, pending, showToast, t],
  );

  const confirmStart = useCallback(
    (data: CreateTransit) => {
      if (pending?.kind !== 'start') return;
      const holdId = pending.hold.id;
      return run(() => api.createHoldTransit(holdId, data).then(() => undefined), 'holds.startTransitSuccess');
    },
    [pending, run],
  );

  const confirmShip = useCallback(() => {
    if (pending?.kind !== 'ship') return;
    const id = pending.transit.id;
    return run(() => api.shipTransit(id).then(() => undefined), 'holds.shipSuccess');
  }, [pending, run]);

  const confirmReceive = useCallback(() => {
    if (pending?.kind !== 'receive') return;
    const id = pending.transit.id;
    return run(() => api.receiveTransit(id).then(() => undefined), 'holds.receiveSuccess');
  }, [pending, run]);

  const confirmCancel = useCallback(() => {
    if (pending?.kind !== 'cancel') return;
    const transit = pending.transit;
    return run(
      () =>
        api
          .cancelTransit(transit.id, { reverse: transit.status === 'inTransit' })
          .then(() => undefined),
      'holds.cancelTransitSuccess',
    );
  }, [pending, run]);

  return {
    pending,
    isLoading,
    error,
    busyKey,
    open,
    close,
    confirmStart,
    confirmShip,
    confirmReceive,
    confirmCancel,
  };
}
