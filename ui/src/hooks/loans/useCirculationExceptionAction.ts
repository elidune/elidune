import { useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import api from '@/services/api';
import { useToast } from '@/contexts/ToastContext';
import { getApiErrorMessage } from '@/utils/apiError';
import type { CirculationExceptionOutcome, DamageDisposition } from '@/types';

export type CirculationExceptionKind =
  | 'lost'
  | 'damaged'
  | 'claimedReturned'
  | 'resolveFound'
  | 'resolveNotFound';

export interface CirculationExceptionTarget {
  loanId: string;
  kind: CirculationExceptionKind;
  label?: string;
}

export interface CirculationExceptionSubmit {
  bill: boolean;
  amount: string;
  disposition: DamageDisposition;
  notes: string;
  inventoryChecked: boolean;
}

function emptyToUndef(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

export function successKeyForOutcome(outcome: CirculationExceptionOutcome): string {
  switch (outcome) {
    case 'lost':
      return 'loans.exceptions.lostSuccess';
    case 'damaged':
      return 'loans.exceptions.damagedSuccess';
    case 'claimedReturned':
      return 'loans.exceptions.claimedSuccess';
    case 'claimsResolvedFound':
      return 'loans.exceptions.resolveFoundSuccess';
    case 'claimsResolvedNotFound':
      return 'loans.exceptions.resolveNotFoundSuccess';
  }
}

async function applyException(loanId: string, kind: CirculationExceptionKind, payload: CirculationExceptionSubmit) {
  const notes = emptyToUndef(payload.notes);
  switch (kind) {
    case 'lost': {
      const amount = emptyToUndef(payload.amount);
      const bill = payload.bill || Boolean(amount);
      return api.markLoanLost(loanId, { bill, amount: bill ? amount : undefined, notes });
    }
    case 'damaged': {
      const amount = emptyToUndef(payload.amount);
      const bill = payload.bill || Boolean(amount);
      return api.markLoanDamaged(loanId, {
        disposition: payload.disposition,
        bill,
        amount: bill ? amount : undefined,
        notes,
      });
    }
    case 'claimedReturned':
      return api.markLoanClaimedReturned(loanId, { notes });
    case 'resolveFound':
      return api.resolveClaimsReturned(loanId, {
        outcome: 'found',
        inventoryChecked: true,
        notes,
      });
    case 'resolveNotFound':
      return api.resolveClaimsReturned(loanId, {
        outcome: 'notFound',
        inventoryChecked: true,
        notes,
      });
  }
}

interface UseCirculationExceptionActionOptions {
  setBusy?: (busy: { loanId: string; op: CirculationExceptionKind } | null) => void;
  onSuccess?: () => void | Promise<void>;
}

export function useCirculationExceptionAction(options: UseCirculationExceptionActionOptions = {}) {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const queryClient = useQueryClient();
  const inFlightRef = useRef(false);

  const [target, setTarget] = useState<CirculationExceptionTarget | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setBusy = options.setBusy;
  const onSuccess = options.onSuccess;

  const open = useCallback((next: CirculationExceptionTarget) => {
    if (inFlightRef.current) return;
    setError(null);
    setTarget(next);
  }, [inFlightRef]);

  const close = useCallback(() => {
    if (inFlightRef.current) return;
    setTarget(null);
    setError(null);
  }, [inFlightRef]);

  const submit = useCallback(
    async (payload: CirculationExceptionSubmit) => {
      if (!target || inFlightRef.current) return;
      inFlightRef.current = true;
      setBusy?.({ loanId: target.loanId, op: target.kind });
      setIsLoading(true);
      setError(null);
      try {
        const result = await applyException(target.loanId, target.kind, payload);
        showToast({ variant: 'success', message: t(successKeyForOutcome(result.outcome)) });
        setTarget(null);
        await queryClient.invalidateQueries({ queryKey: ['loans-claims-returned'] });
        await queryClient.invalidateQueries({ queryKey: ['user-active-loans'] });
        await queryClient.invalidateQueries({ queryKey: ['user-past-loans'] });
        await onSuccess?.();
      } catch (err: unknown) {
        const msg = getApiErrorMessage(err, t) || t('loans.exceptions.error');
        setError(msg);
        showToast({ variant: 'error', message: msg });
      } finally {
        inFlightRef.current = false;
        setBusy?.(null);
        setIsLoading(false);
      }
    },
    [inFlightRef, onSuccess, queryClient, setBusy, showToast, t, target],
  );

  return { target, isLoading, error, open, close, submit };
}
