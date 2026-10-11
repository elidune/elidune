import { useState, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { BookOpen, Calendar, RotateCcw, AlertTriangle } from 'lucide-react';
import { Card, CardHeader, Button, Badge, Pagination, ListSkeleton, QueryErrorBanner } from '@/components/common';
import { useToast } from '@/contexts/ToastContext';
import LoansMarcExportButton from '@/components/loans/LoansMarcExportButton';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import type { MyLoan, MyLoanRenewalRefusal } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { renewedUntilMessage } from '@/utils/localeDate';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';
import { sortLoansByStartDateAsc } from '@/utils/sortLoans';
import { newIdempotencyKey } from '@/utils/idempotency';

const MY_LOANS_PAGE_SIZE = 20;

export default function MyLoansPage() {
  const { t, i18n } = useTranslation();
  const { showToast } = useToast();
  const { user } = useAuth();
  const [loans, setLoans] = useState<MyLoan[]>([]);
  const [loansPage, setLoansPage] = useState(1);
  const [loansTotal, setLoansTotal] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [renewError, setRenewError] = useState('');
  const [reloadToken, setReloadToken] = useState(0);

  const [prevLoansUserId, setPrevLoansUserId] = useState(user?.id);
  if (user?.id !== prevLoansUserId) {
    setPrevLoansUserId(user?.id);
    setLoansPage(1);
  }

  useEffect(() => {
    const fetchLoans = async () => {
      if (!user?.id) return;
      setIsLoading(true);
      setLoadError(null);
      try {
        const res = await api.getMyLoans({
          page: loansPage,
          perPage: MY_LOANS_PAGE_SIZE,
        });
        setLoans(res.items);
        setLoansTotal(res.total);
      } catch (error) {
        setLoadError(getApiErrorMessage(error, t) || t('loans.loansLoadError'));
      } finally {
        setIsLoading(false);
      }
    };

    fetchLoans();
  }, [user?.id, loansPage, reloadToken, t]);

  const renewInFlightRef = useRef(false);

  const handleRenewLoan = async (loanId: string) => {
    if (renewInFlightRef.current) return;
    renewInFlightRef.current = true;
    setRenewError('');
    try {
      const renewed = await api.renewLoan(loanId, newIdempotencyKey());
      showToast({
        variant: 'success',
        message: renewedUntilMessage(renewed.expiryAt, t, i18n.language),
      });
      if (user?.id) {
        const res = await api.getMyLoans({
          page: loansPage,
          perPage: MY_LOANS_PAGE_SIZE,
        });
        setLoans(res.items);
        setLoansTotal(res.total);
      }
    } catch (error) {
      console.error('Error renewing loan:', error);
      setRenewError(getApiErrorMessage(error, t) || t('loans.errorRenewingLoan'));
    } finally {
      renewInFlightRef.current = false;
    }
  };

  const loansSorted = useMemo(() => sortLoansByStartDateAsc(loans), [loans]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('loans.myLoans')}</h1>
          <p className="text-gray-500 dark:text-gray-400">
            {t('loans.count', { count: loansTotal })}
          </p>
        </div>
        {user?.id ? (
          <div className="shrink-0">
            <LoansMarcExportButton userId={user.id} />
          </div>
        ) : null}
      </div>

      {loadError && (
        <QueryErrorBanner
          message={loadError}
          onRetry={() => setReloadToken((n) => n + 1)}
        />
      )}

      {/* Renew error */}
      {renewError && (
        <div className="flex items-start gap-3 p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-xl">
          <AlertTriangle className="h-5 w-5 text-red-600 dark:text-red-500 flex-shrink-0 mt-0.5" />
          <p className="font-medium text-red-800 dark:text-red-200">{renewError}</p>
        </div>
      )}

      <Card>
        <CardHeader
          title={t('loans.activeLoans')}
          subtitle={t('items.count', { count: loansTotal })}
        />
        {isLoading ? (
          <ListSkeleton rows={6} />
        ) : loadError ? null : loans.length === 0 ? (
          <div className="text-center py-8 text-gray-500 dark:text-gray-400">
            <BookOpen className="h-12 w-12 mx-auto mb-3 opacity-30" />
            <p>{t('loans.noLoans')}</p>
          </div>
        ) : (
          <div className="space-y-3">
            {loansSorted.map((loan) => (
              <LoanCard key={loan.id} loan={loan} onRenew={handleRenewLoan} isOverdue={loan.isOverdue} />
            ))}
          </div>
        )}
        {!loadError && loansTotal > 0 && (
          <div className="pt-4 border-t border-gray-200 dark:border-gray-800">
            <Pagination
              currentPage={loansPage}
              totalPages={Math.max(1, Math.ceil(loansTotal / MY_LOANS_PAGE_SIZE))}
              onPageChange={setLoansPage}
            />
          </div>
        )}
      </Card>
    </div>
  );
}

interface LoanCardProps {
  loan: MyLoan;
  onRenew: (id: string) => void;
  isOverdue?: boolean;
}

function renewalRefusalText(
  t: (key: string, options?: Record<string, unknown>) => string,
  code: MyLoanRenewalRefusal | null | undefined,
  opensAt: string | null | undefined,
  language: string,
): string | null {
  if (!code) return null;
  if (code === 'renewal_too_early') {
    const date = opensAt
      ? new Date(`${opensAt}T00:00:00`).toLocaleDateString(language)
      : '';
    return t('loans.renewalRefusal.renewal_too_early', { date });
  }
  return t(`loans.renewalRefusal.${code}`);
}

function LoanCard({ loan, onRenew, isOverdue = false }: LoanCardProps) {
  const { t, i18n } = useTranslation();
  const [referenceMs] = useState(() => Date.now());
  const daysUntilDue = Math.ceil(
    (new Date(loan.dueDate).getTime() - referenceMs) / (1000 * 60 * 60 * 24)
  );
  const refusal = renewalRefusalText(t, loan.renewalRefusalCode, loan.renewalOpensAt, i18n.language);

  return (
    <div
      className={`flex flex-col sm:flex-row sm:items-center gap-4 p-4 rounded-xl border ${
        isOverdue
          ? 'border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/10'
          : 'border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50'
      }`}
    >
      <LoanMediaTypeBadge mediaType={loan.mediaType} size="comfortable" />

      <div className="flex-1 min-w-0">
        <p className="font-medium text-gray-900 dark:text-white truncate">
          {loan.title || t('items.notSpecified')}
        </p>
        <p className="text-sm text-gray-500 dark:text-gray-400 font-mono">
          {loan.barcode || '-'}
        </p>
        <div className="flex items-center gap-4 mt-2 text-sm">
          <div className="flex items-center gap-1 text-gray-500 dark:text-gray-400">
            <Calendar className="h-4 w-4" />
            <span>{t('loans.loanDate')}: {new Date(loan.startDate).toLocaleDateString(i18n.language)}</span>
          </div>
        </div>
      </div>

      <div className="flex flex-col sm:items-end gap-2">
        <div className="flex items-center gap-2">
          {isOverdue ? (
            <Badge variant="danger">{t('loans.overdueDays', { count: Math.abs(daysUntilDue) })}</Badge>
          ) : daysUntilDue <= 3 ? (
            <Badge variant="warning">
              {daysUntilDue === 0 ? t('loans.dueToday') : t('loans.dueInDays', { count: daysUntilDue })}
            </Badge>
          ) : (
            <Badge variant="success">
              {t('loans.dueDate')}: {new Date(loan.dueDate).toLocaleDateString(i18n.language)}
            </Badge>
          )}
        </div>

        {loan.canRenew ? (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onRenew(loan.id)}
            leftIcon={<RotateCcw className="h-4 w-4" />}
          >
            {t('loans.renew')}
          </Button>
        ) : refusal ? (
          <p className="text-sm text-gray-600 dark:text-gray-300 sm:text-right max-w-xs">{refusal}</p>
        ) : null}
      </div>
    </div>
  );
}

