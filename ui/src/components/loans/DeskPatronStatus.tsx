import { useTranslation } from 'react-i18next';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import type { PatronLoanStatus } from '@/types';
import { formatIsoCalendarDate } from '@/utils/apiError';
import { checkoutBlockingReasons } from '@/utils/deskPatron';
import { formatCentsAsEuros } from '@/utils/lostFound';
import {
  borrowerAvatarInitials,
  borrowerShownToAgent,
  isMinimizedBorrower,
  type BorrowerRef,
} from '@/utils/readerIdentifier';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';

interface DeskPatronStatusProps {
  identity: BorrowerRef;
  /** Link to the reader record. Callers pass this only with users read rights. */
  linked: boolean;
  loanStatus: PatronLoanStatus | null;
}

function subscriptionLabel(
  status: PatronLoanStatus,
  t: (key: string, values?: Record<string, unknown>) => string,
  language: string,
): string {
  const expiry = status.subscriptionExpiresAt
    ? formatIsoCalendarDate(status.subscriptionExpiresAt, language)
    : null;
  if (status.subscriptionValid) {
    return expiry ? t('loans.subscriptionValidUntil', { date: expiry }) : t('loans.subscriptionValid');
  }
  return expiry ? t('loans.subscriptionExpiredOn', { date: expiry }) : t('loans.subscriptionExpired');
}

/**
 * Patron chosen at the desk after a card scan (or a name search when users read is allowed).
 * Loan facts come from `loanStatus` on `GET /users/by-card/:cardNumber`.
 */
export default function DeskPatronStatus({ identity, linked, loanStatus }: DeskPatronStatusProps) {
  const { t, i18n } = useTranslation();
  const shown = borrowerShownToAgent(identity, linked) ?? identity;
  const minimized = !linked || isMinimizedBorrower(shown);
  const blocking = loanStatus ? checkoutBlockingReasons(loanStatus.blockReasons) : [];
  const overdueWarning = loanStatus?.blockReasons.includes('overdue_loans') ?? false;
  const cardLost = loanStatus?.blockReasons.includes('card_lost') ?? false;
  const quota =
    loanStatus == null
      ? null
      : loanStatus.maxLoans == null
        ? t('loans.loanQuotaOpen', { active: loanStatus.activeLoans })
        : t('loans.loanQuota', { active: loanStatus.activeLoans, max: loanStatus.maxLoans });

  return (
    <div className="min-w-0 flex-1">
      <div className="flex min-w-0 items-center gap-2.5">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-100 text-sm font-bold text-amber-800 dark:bg-amber-900/55 dark:text-amber-200">
          {borrowerAvatarInitials(shown)}
        </div>
        <div className="min-w-0">
          <ReaderIdentifier
            user={shown}
            linked={linked}
            className="font-semibold text-gray-900 hover:text-indigo-600 dark:text-white dark:hover:text-indigo-400"
          />
          {!minimized && (shown.accountType || shown.barcode) ? (
            <p className="truncate text-xs text-gray-500 dark:text-gray-400">
              {shown.accountType ? localizedAccountTypeLabel(t, shown.accountType) : null}
              {shown.barcode ? `${shown.accountType ? ' · ' : ''}${shown.barcode}` : ''}
            </p>
          ) : null}
        </div>
      </div>
      {loanStatus ? (
        <ul className="mt-2 space-y-0.5 text-xs text-gray-600 dark:text-gray-300">
          <li data-loan-fact="subscription">
            {subscriptionLabel(loanStatus, (key, values) => String(t(key, values)), i18n.language)}
          </li>
          <li data-loan-fact="quota">{quota}</li>
          <li data-loan-fact="overdue">{t('loans.overdueCount', { count: loanStatus.overdueLoans })}</li>
          <li data-loan-fact="fines">
            {t('loans.unpaidFines', {
              amount: formatCentsAsEuros(loanStatus.unpaidFinesCents, i18n.language),
            })}
          </li>
          <li data-loan-fact="readyHolds">{t('loans.readyHolds', { count: loanStatus.readyHolds })}</li>
        </ul>
      ) : null}
      {loanStatus?.blocked && blocking.length > 0 ? (
        <p className="mt-1 text-xs font-medium text-red-700 dark:text-red-300" data-patron-block="true">
          {t('loans.cardBlocked', {
            reasons: blocking.map((code) => t(`loans.blockReason.${code}`)).join(', '),
          })}
        </p>
      ) : null}
      {overdueWarning ? (
        <p
          className="mt-1 text-xs font-medium text-amber-700 dark:text-amber-300"
          data-patron-warning="overdue_loans"
        >
          {t('loans.blockReason.overdue_loans')}
        </p>
      ) : null}
      {cardLost ? (
        <div
          role="status"
          data-card-lost-banner
          className="mt-2 rounded-lg border border-sky-200 bg-sky-50 p-3 text-sm text-sky-950 dark:border-sky-900/60 dark:bg-sky-950/30 dark:text-sky-100"
        >
          <p>{t('loans.cardLostNotice')}</p>
          <p className="mt-1">{t('loans.cardLostStillAllowed')}</p>
        </div>
      ) : null}
    </div>
  );
}
