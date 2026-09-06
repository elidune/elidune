import type { Loan, LoanSettings, MediaType } from '@/types';

/** Default used only when settings have not loaded or have no matching row. */
export const FALLBACK_MAX_RENEWALS = 2;

export function maxRenewalsForMedia(
  settings: LoanSettings[],
  mediaType: MediaType | string | null | undefined,
): number {
  if (mediaType) {
    const specific = settings.find((s) => s.mediaType === mediaType);
    if (specific) return specific.maxRenewals;
  }
  const fallback = settings.find((s) => s.mediaType == null);
  return fallback?.maxRenewals ?? FALLBACK_MAX_RENEWALS;
}

export function renewalsRemaining(loan: Loan, settings: LoanSettings[]): number {
  const max = maxRenewalsForMedia(settings, loan.biblio?.mediaType);
  return Math.max(0, max - loan.nbRenews);
}
