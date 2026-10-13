import type { TFunction } from 'i18next';
import type { LoanClosureReason } from '@/types';
import { formatLocaleDate } from '@/utils/localeDate';

export interface LoanClosureFields {
  closureReason?: LoanClosureReason | null;
  foundAt?: string | null;
}

/** Query for `GET /me/loans`. `archived` is sent only for the caller's closed loans. */
export function myLoansRequestParams(options?: {
  page?: number;
  perPage?: number;
  archived?: boolean;
}): { page?: number; perPage?: number; archived?: true } {
  return {
    page: options?.page,
    perPage: options?.perPage,
    ...(options?.archived ? { archived: true as const } : {}),
  };
}

/**
 * Closure label for a closed loan.
 * `returned` → Rendu. `lost` → Perdu. `lost` with `foundAt` → Perdu, retrouvé le {date}.
 * Open loans omit the reason and produce no label.
 */
export function closureReasonText(
  loan: LoanClosureFields,
  t: TFunction,
  language: string,
): string | null {
  if (loan.closureReason === 'lost') {
    const foundAt = loan.foundAt?.trim();
    if (foundAt) {
      return t('loans.closure.lostFound', { date: formatLocaleDate(foundAt, language) });
    }
    return t('loans.closure.lost');
  }
  if (loan.closureReason === 'returned') return t('loans.closure.returned');
  return null;
}
