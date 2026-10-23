import { useMemo, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { useMyLoansQuery } from '@/hooks/loans/useMyLoansQuery';
import { BookOpen, Users, BookMarked, TrendingUp, ArrowRight, AlertTriangle, Lightbulb } from 'lucide-react';
import { useAuth } from '@/contexts/AuthContext';
import { useLibrary } from '@/contexts/LibraryContext';
import { Card, CardHeader, Badge, LibraryInfoSection, ListSkeleton, QueryErrorBanner } from '@/components/common';
import { useLibrarySchedule } from '@/hooks/common/useLibrarySchedule';
import api from '@/services/api';
import { effectiveStaffAccess, isFeatureEnabled } from '@/utils/featureAccess';
import { getApiErrorMessage } from '@/utils/apiError';
import { sortLoansByStartDateAsc } from '@/utils/sortLoans';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';

export default function HomePage() {
  const { t, i18n } = useTranslation();
  const { user } = useAuth();
  const { libraryName, libraryInfo } = useLibrary();
  const { scheduleSlots } = useLibrarySchedule();
  const features = libraryInfo?.enabledFeatures;
  const access = effectiveStaffAccess(user, api.getToken(), features);
  const staff = access.homeStats;
  const showSuggestions = isFeatureEnabled(features, 'purchaseSuggestions');

  const statsQuery = useQuery({
    queryKey: ['home-stats'],
    queryFn: () => api.getStats(),
    enabled: staff,
    staleTime: 60 * 1000,
  });

  const loansQuery = useMyLoansQuery(user?.id, { page: 1, perPage: 20 });

  const stats = statsQuery.data ?? null;
  const myLoans = useMemo(() => loansQuery.data?.items ?? [], [loansQuery.data?.items]);
  const isLoading = loansQuery.isLoading || (staff && statsQuery.isLoading);

  const myLoansSorted = useMemo(() => sortLoansByStartDateAsc(myLoans), [myLoans]);
  const overdueLoans = myLoans.filter((loan) => loan.isOverdue);

  const greetingName =
    [user?.firstname, user?.lastname].filter(Boolean).join(' ').trim() || user?.username?.trim() || '';

  return (
    <div className="flex flex-col gap-6 min-h-full">
      {/* Welcome section — hidden on mobile (library name already in app header) */}
      <div className="relative hidden overflow-hidden rounded-2xl border border-gray-200/80 dark:border-gray-800 bg-gradient-to-br from-white via-amber-50/40 to-white dark:from-gray-900 dark:via-amber-950/25 dark:to-gray-900 p-7 sm:p-10 shadow-sm md:block">
        <img
          src="/elidune_logo.png"
          alt=""
          aria-hidden="true"
          className="absolute -right-6 -top-4 h-56 w-56 sm:h-64 sm:w-64 object-contain opacity-[0.07] dark:opacity-[0.05] select-none pointer-events-none"
        />
        <div className="relative min-w-0 max-w-3xl pr-4 sm:pr-28">
          <h1 className="space-y-1 sm:space-y-2">
            {libraryName ? (
              <span className="block text-2xl sm:text-3xl lg:text-[2rem] font-bold leading-tight tracking-tight text-gray-900 dark:text-white">
                {libraryName}
              </span>
            ) : null}
            <span
              className={`block font-semibold leading-snug text-gray-700 dark:text-gray-300 ${
                libraryName ? 'text-lg sm:text-xl' : 'text-2xl sm:text-3xl font-bold text-gray-900 dark:text-white'
              }`}
            >
              {greetingName ? (
                <>
                  <span className="capitalize">{t('home.welcome')}</span>{' '}
                  <span className="text-amber-800 dark:text-amber-300">{greetingName}</span>
                </>
              ) : (
                t('home.greetingAnonymous')
              )}
            </span>
          </h1>
        </div>
      </div>

      {/* Overdue loans alert */}
      {overdueLoans.length > 0 && (
        <div className="flex items-start gap-3 p-4 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-xl">
          <AlertTriangle className="h-5 w-5 text-amber-600 dark:text-amber-500 flex-shrink-0 mt-0.5" />
          <div>
            <p className="font-medium text-amber-800 dark:text-amber-200">
              {t('loans.overdueCount', { count: overdueLoans.length })}
            </p>
          </div>
        </div>
      )}

      {staff && statsQuery.isError && (
        <QueryErrorBanner
          message={getApiErrorMessage(statsQuery.error, t) || t('home.statsLoadError')}
          onRetry={() => void statsQuery.refetch()}
        />
      )}

      {/* Stats cards (for librarians) */}
      {staff && stats && (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          <StatCard
            icon={BookOpen}
            label={t('stats.documents')}
            value={stats.biblios?.total ?? 0}
            color="gray"
            to={access.internalCatalog ? '/biblios' : undefined}
            ariaLabel={t('home.openSection', { section: t('nav.catalog') })}
          />
          <StatCard
            icon={Users}
            label={t('stats.activeUsers')}
            value={stats.users?.active ?? 0}
            color="emerald"
            to={access.usersRead ? '/users' : undefined}
            ariaLabel={t('home.openSection', { section: t('nav.users') })}
          />
          <StatCard
            icon={BookMarked}
            label={t('stats.activeLoans')}
            value={stats.loans?.active ?? 0}
            color="amber"
            to={access.loansRead ? '/loans' : undefined}
            ariaLabel={t('home.openSection', { section: t('nav.loans') })}
          />
          <StatCard
            icon={TrendingUp}
            label={t('stats.overdue')}
            value={stats.loans?.overdue ?? 0}
            color="red"
            to={access.loansRead ? '/loans?tab=overdue' : undefined}
            ariaLabel={t('home.openSection', { section: t('stats.overdue') })}
          />
        </div>
      )}

      {showSuggestions ? (
        <HomeLinkCard
          to="/my-suggestions"
          ariaLabel={t('home.openSection', { section: t('acquisitions.suggestions.patronTitle') })}
        >
          <CardHeader
            title={t('acquisitions.suggestions.patronTitle')}
            subtitle={t('acquisitions.suggestions.homeHelp')}
            action={
              <span className="text-sm text-amber-600 dark:text-amber-400 flex items-center gap-1" aria-hidden>
                {t('common.view')} <ArrowRight className="h-4 w-4" />
              </span>
            }
          />
          <div className="flex items-start gap-3 text-sm text-gray-600 dark:text-gray-400">
            <Lightbulb className="h-5 w-5 shrink-0 text-amber-600" />
            <p>{t('acquisitions.suggestions.patronSubtitle')}</p>
          </div>
        </HomeLinkCard>
      ) : null}

      {/* My loans */}
      <HomeLinkCard
        to="/my-loans"
        ariaLabel={t('home.openSection', { section: t('loans.myLoans') })}
      >
        <CardHeader
          title={t('loans.myLoans')}
          subtitle={t('loans.count', { count: myLoans.length })}
          action={
            <span className="text-sm text-amber-600 dark:text-amber-400 flex items-center gap-1" aria-hidden>
              {t('common.view')} <ArrowRight className="h-4 w-4" />
            </span>
          }
        />

        {loansQuery.isError && (
          <div className="mb-3">
            <QueryErrorBanner
              message={getApiErrorMessage(loansQuery.error, t) || t('loans.loansLoadError')}
              onRetry={() => void loansQuery.refetch()}
            />
          </div>
        )}

        {isLoading ? (
          <ListSkeleton rows={4} />
        ) : loansQuery.isError ? null : myLoans.length === 0 ? (
          <div className="text-center py-8 text-gray-500 dark:text-gray-400">
            <BookMarked className="h-12 w-12 mx-auto mb-3 opacity-30" />
            <p>{t('loans.noLoans')}</p>
          </div>
        ) : (
          <div className="space-y-3">
            {myLoansSorted.slice(0, 5).map((loan) => (
              <div
                key={loan.id}
                className="flex items-center gap-4 p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50"
              >
                <LoanMediaTypeBadge mediaType={loan.mediaType} size="catalog" />
                <div className="flex-1 min-w-0">
                  <p className="font-medium text-gray-900 dark:text-white truncate">
                    {loan.title || t('items.notSpecified')}
                  </p>
                  <p className="text-sm text-gray-500 dark:text-gray-400">
                    {t('loans.dueDate')}: {new Date(loan.dueDate).toLocaleDateString(i18n.language)}
                  </p>
                </div>
                {loan.isOverdue && (
                  <Badge variant="danger">{t('loans.overdue')}</Badge>
                )}
              </div>
            ))}
          </div>
        )}
      </HomeLinkCard>

      {/* Library info — fills remaining space */}
      <div className="flex-1 flex flex-col">
        <LibraryInfoSection
          info={libraryInfo}
          slots={scheduleSlots}
          canManage={access.fullSettings}
        />
      </div>
    </div>
  );
}

interface StatCardProps {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: number;
  color: 'gray' | 'emerald' | 'amber' | 'red';
  to?: string;
  ariaLabel?: string;
}

function StatCard({ icon: Icon, label, value, color, to, ariaLabel }: StatCardProps) {
  const colors = {
    gray: 'bg-gray-100 text-gray-700 dark:bg-gray-800/50 dark:text-gray-300',
    emerald: 'bg-emerald-50 text-emerald-600 dark:bg-emerald-900/30 dark:text-emerald-400',
    amber: 'bg-amber-50 text-amber-600 dark:bg-amber-900/30 dark:text-amber-400',
    red: 'bg-red-50 text-red-600 dark:bg-red-900/30 dark:text-red-400',
  };

  const body = (
    <div className="flex items-center gap-4">
      <div className={`flex-shrink-0 h-12 w-12 rounded-xl flex items-center justify-center ${colors[color]}`}>
        <Icon className="h-6 w-6" />
      </div>
      <div>
        <p className="text-2xl font-bold text-gray-900 dark:text-white">{value.toLocaleString()}</p>
        <p className="text-sm text-gray-500 dark:text-gray-400">{label}</p>
      </div>
    </div>
  );

  if (!to) {
    return <Card>{body}</Card>;
  }

  return (
    <HomeLinkCard to={to} ariaLabel={ariaLabel ?? label}>
      {body}
    </HomeLinkCard>
  );
}

type HomeLinkCardProps = {
  to: string;
  ariaLabel: string;
  children: ReactNode;
};

function HomeLinkCard({ to, ariaLabel, children }: HomeLinkCardProps) {
  return (
    <Link
      to={to}
      aria-label={ariaLabel}
      className="group block rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-950"
    >
      <Card className="h-full cursor-pointer transition-shadow group-hover:border-amber-200 group-hover:shadow-md dark:group-hover:border-amber-800/60">
        {children}
      </Card>
    </Link>
  );
}

