import { lazy, Suspense, useState, useEffect, useRef, useMemo } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  ArrowLeft,
  Trash2,
  Calendar,
  BookMarked,
  Ban,
  RotateCcw,
  RefreshCw,
  Check,
  Filter,
  X,
  AlertTriangle,
  Loader2,
  AlertCircle,
  LockOpen,
} from 'lucide-react';
import { useInfiniteQuery, useQuery, useQueryClient } from '@tanstack/react-query';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import { useAccountTypesQuery } from '@/hooks/useAccountTypesQuery';
import { Card, Button, Badge, Modal, Input, Table, ConfirmDialog, MessageModal, ScrollableListRegion, BarcodeScanField } from '@/components/common';
import api from '@/services/api';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { circulationMayForce } from '@/utils/circulationForce';
import { closureReasonText } from '@/utils/loanClosure';
import { renewedUntilMessage } from '@/utils/localeDate';
import { isWeedingServerMessage } from '@/utils/weeding';
import { isSubscriptionExpired } from '@/utils/userSubscription';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import { sortLoansByStartDateAsc } from '@/utils/sortLoans';
import { isClaimedReturnedLoan } from '@/utils/circulationStatus';
import { interpretPickCheckIn, readItemToPickForOther } from '@/utils/toPickDesk';
import { newIdempotencyKey } from '@/utils/idempotency';
import HoldDocumentCell from '@/components/holds/HoldDocumentCell';
import HoldStatusBadge from '@/components/holds/HoldStatusBadge';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import ToPickForceLoanDialog from '@/components/loans/ToPickForceLoanDialog';
import HoldPickupCell from '@/components/holds/HoldPickupCell';
import HoldTransitActions from '@/components/holds/HoldTransitActions';
import TransitActionModals from '@/components/holds/TransitActionModals';
import { useActiveTransitsQuery } from '@/hooks/holds/useActiveTransitsQuery';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import { useTransitActions } from '@/hooks/holds/useTransitActions';
import { indexTransitsByHoldId } from '@/utils/transitDisplay';
import LoansAccessNotice from '@/components/loans/LoansAccessNotice';
import LoansMarcExportButton from '@/components/loans/LoansMarcExportButton';
import CirculationExceptionDialog from '@/components/loans/CirculationExceptionDialog';
import LoanExceptionActions from '@/components/loans/LoanExceptionActions';
import { useCirculationExceptionAction, type CirculationExceptionKind } from '@/hooks/loans/useCirculationExceptionAction';
import { RenewSubscriptionModal, UserEditorForm } from '@/components/users';
import { useAuth } from '@/contexts/AuthContext';
import { useToast } from '@/contexts/ToastContext';
import { canManageUsers, canReadLoans, canReadUsers, type User as UserType, type LoanListItem, type LoanStatsResponse, type AdvancedStatsParams, type StatsInterval, type Author, type Hold } from '@/types';
import PatronFinesPanel from '@/components/users/PatronFinesPanel';
import { useUserFinesQuery } from '@/hooks/users/useUserFinesQuery';
import { mayLoadPatronFines } from '@/utils/patronFines';
import { isUserRecordLoan } from '@/utils/catalogView';
import {
  archivedUserLoansQuery,
  currentUserLoansQuery,
  emptyLoanPage,
  isLoansForbidden,
  loanHistoryVisible,
  loansQueryRetry,
  readPatronLoans,
  userRecordQueryRetry,
} from '@/utils/loanListAccess';
import UserRecordLoanIdentity from '@/components/users/UserRecordLoanIdentity';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import { localizedPublicTypeById } from '@/utils/publicTypeDisplay';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { deferFromEffect } from '@/utils/deferFromEffect';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';

const LoansAreaChart = lazy(() => import('@/components/stats/LoansAreaChart'));

const USER_LOANS_PAGE_SIZE = 20;
const USER_HOLDS_PAGE_SIZE = 20;

type UserDetailTab = 'info' | 'activeLoans' | 'pastLoans' | 'fines' | 'holds' | 'stats';

export default function UserDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user: currentUser } = useAuth();
  const canReadBorrowers = canReadUsers(currentUser, api.getToken());
  const canListReaderLoans = canReadBorrowers && canReadLoans(currentUser, api.getToken());
  const canSeeLoanHistory = loanHistoryVisible(
    canReadBorrowers,
    canReadLoans(currentUser, api.getToken()),
  );
  const { t, i18n } = useTranslation();
  const { showToast } = useToast();
  const holdsLoadMoreRef = useRef<HTMLDivElement>(null);
  const activeLoansLoadMoreRef = useRef<HTMLDivElement>(null);
  const pastLoansLoadMoreRef = useRef<HTMLDivElement>(null);

  const [detailTab, setDetailTab] = useState<UserDetailTab>('info');
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleteUserLoading, setDeleteUserLoading] = useState(false);
  const [deleteUserActiveLoansError, setDeleteUserActiveLoansError] = useState(false);
  const [showBorrowModal, setShowBorrowModal] = useState(false);
  const [forceBorrowDialog, setForceBorrowDialog] = useState<{ message: string; specimenCode: string } | null>(
    null
  );
  const [toPickForce, setToPickForce] = useState<{
    cardNumber: string | null;
    initials: string | null;
    specimenCode: string;
  } | null>(null);
  const [toPickForceLoading, setToPickForceLoading] = useState(false);
  const [borrowForceError, setBorrowForceError] = useState<string | null>(null);
  const [isEditLoading, setIsEditLoading] = useState(false);
  const [isBorrowLoading, setIsBorrowLoading] = useState(false);
  const [loanDetails, setLoanDetails] = useState<LoanListItem | null>(null);
  const [renewError, setRenewError] = useState('');
  const [renewForceCode, setRenewForceCode] = useState<string | null>(null);
  const [renewForceLoanId, setRenewForceLoanId] = useState<string | null>(null);
  const [loanAction, setLoanAction] = useState<{ loanId: string; op: 'return' | 'renew' | CirculationExceptionKind } | null>(null);
  const loanActionInFlightRef = useRef(false);

  const { data: claimsIdPage } = useQuery({
    queryKey: ['loans-claims-returned', 'ids'],
    queryFn: () => api.getClaimsReturned({ page: 1, perPage: 200 }),
  });
  const claimedLoanIds = useMemo(
    () => new Set((claimsIdPage?.items ?? []).map((row) => row.loanId)),
    [claimsIdPage],
  );

  const exceptionAction = useCirculationExceptionAction({
    setBusy: setLoanAction,
    onSuccess: () => {
      if (id) {
        void queryClient.invalidateQueries({ queryKey: ['user-active-loans', id] });
        void queryClient.invalidateQueries({ queryKey: ['user-past-loans', id] });
      }
    },
  });
  const deskBusy = loanAction != null || exceptionAction.target != null;
  const [showRenewSubscriptionModal, setShowRenewSubscriptionModal] = useState(false);
  const [showUnlockSignIn, setShowUnlockSignIn] = useState(false);
  const [unlockSignInLoading, setUnlockSignInLoading] = useState(false);
  const [unlockSignInError, setUnlockSignInError] = useState<string | null>(null);

  const [cancellingHoldId, setCancellingHoldId] = useState<string | null>(null);
  const sourcesQuery = useSourcesQuery();
  const sources = sourcesQuery.data ?? [];
  const activeTransitsQuery = useActiveTransitsQuery();
  const transitByHoldId = useMemo(
    () => indexTransitsByHoldId(activeTransitsQuery.data ?? []),
    [activeTransitsQuery.data],
  );
  const transitActions = useTransitActions();

  const { data: publicTypes = [] } = usePublicTypesQuery();
  const { data: accountTypes = [] } = useAccountTypesQuery();

  // Stats state
  const [loanStats, setLoanStats] = useState<LoanStatsResponse | null>(null);
  const [isLoadingStats, setIsLoadingStats] = useState(false);
  const [totalLoansAllTime, setTotalLoansAllTime] = useState<number | null>(null);
  const [userStatsFilters, setUserStatsFilters] = useState<{
    startDate: string;
    endDate: string;
    interval: StatsInterval;
  } | null>(null);

  const getDefaultStatsFilters = () => {
    const end = new Date();
    const start = new Date();
    start.setFullYear(end.getFullYear() - 1);
    return {
      startDate: start.toISOString().split('T')[0],
      endDate: end.toISOString().split('T')[0],
      interval: 'month' as StatsInterval,
    };
  };

  const {
    data: user,
    isPending: isUserLoading,
    isError: isUserError,
    error: userQueryError,
    refetch: refetchUser,
  } = useQuery({
    queryKey: ['user', id],
    queryFn: () => api.getUser(id!),
    enabled: !!id,
    retry: userRecordQueryRetry,
  });

  const {
    data: activeLoansPages,
    isLoading: isLoadingActiveLoans,
    isFetchingNextPage: isFetchingNextActiveLoans,
    hasNextPage: hasNextActivePage,
    fetchNextPage: fetchNextActivePage,
    isError: isActiveLoansError,
    error: activeLoansError,
    refetch: refetchActiveLoans,
  } = useInfiniteQuery({
    queryKey: ['user-active-loans', id],
    queryFn: async ({ pageParam }) => {
      const result = await readPatronLoans(() =>
        api.getUserLoans(id!, currentUserLoansQuery(pageParam, USER_LOANS_PAGE_SIZE)),
      );
      if (result.ok) return result.page;
      throw result.error;
    },
    initialPageParam: 1,
    getNextPageParam: (lastPage, allPages) => {
      const totalLoaded = allPages.reduce((sum, p) => sum + p.items.length, 0);
      return totalLoaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    enabled: !!id && canListReaderLoans && detailTab === 'activeLoans',
    retry: loansQueryRetry,
  });
  const activeLoans = useMemo(
    () =>
      sortLoansByStartDateAsc(
        (activeLoansPages?.pages.flatMap((p) => p.items) ?? []).filter(isUserRecordLoan),
      ),
    [activeLoansPages],
  );
  const activeLoansTotal = activeLoansPages?.pages[0]?.total ?? 0;

  const {
    data: pastLoansPages,
    isLoading: isLoadingPastLoans,
    isFetchingNextPage: isFetchingNextPastLoans,
    hasNextPage: hasNextPastPage,
    fetchNextPage: fetchNextPastPage,
    isError: isPastLoansError,
    error: pastLoansError,
    refetch: refetchPastLoans,
  } = useInfiniteQuery({
    queryKey: ['user-past-loans', id],
    queryFn: async ({ pageParam }) => {
      const query = archivedUserLoansQuery(pageParam, USER_LOANS_PAGE_SIZE, canReadBorrowers);
      if (!query) return emptyLoanPage<LoanListItem>();
      const result = await readPatronLoans(() => api.getUserLoans(id!, query));
      if (result.ok) return result.page;
      throw result.error;
    },
    initialPageParam: 1,
    getNextPageParam: (lastPage, allPages) => {
      const totalLoaded = allPages.reduce((sum, p) => sum + p.items.length, 0);
      return totalLoaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    enabled: !!id && canSeeLoanHistory && detailTab === 'pastLoans',
    retry: loansQueryRetry,
  });
  const pastLoans = (pastLoansPages?.pages.flatMap((p) => p.items) ?? []).filter(isUserRecordLoan);
  const finesVisible = mayLoadPatronFines(canReadBorrowers);
  const {
    data: finesData,
    isLoading: isLoadingFines,
    isError: isFinesError,
    error: finesError,
    refetch: refetchFines,
  } = useUserFinesQuery(id, detailTab === 'fines' && finesVisible);

  useEffect(() => {
    const el = activeLoansLoadMoreRef.current;
    const scrollRoot = el?.closest('.app-list-scroll') ?? null;
    if (!el || !scrollRoot || !hasNextActivePage || isFetchingNextActiveLoans || detailTab !== 'activeLoans')
      return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) fetchNextActivePage();
      },
      { root: scrollRoot, rootMargin: '200px', threshold: 0 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasNextActivePage, isFetchingNextActiveLoans, fetchNextActivePage, detailTab]);

  useEffect(() => {
    const el = pastLoansLoadMoreRef.current;
    const scrollRoot = el?.closest('.app-list-scroll') ?? null;
    if (!el || !scrollRoot || !hasNextPastPage || isFetchingNextPastLoans || detailTab !== 'pastLoans') return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) fetchNextPastPage();
      },
      { root: scrollRoot, rootMargin: '200px', threshold: 0 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasNextPastPage, isFetchingNextPastLoans, fetchNextPastPage, detailTab]);

  const refreshLoans = () => {
    if (!user?.id) return;
    void queryClient.invalidateQueries({ queryKey: ['user-active-loans', user.id] });
    void queryClient.invalidateQueries({ queryKey: ['user-past-loans', user.id] });
    void queryClient.invalidateQueries({ queryKey: ['user-holds', user.id] });
  };

  if (detailTab === 'stats' && user && !userStatsFilters) {
    setUserStatsFilters(getDefaultStatsFilters());
  }

  // Fetch stats when filters or user change
  useEffect(() => {
    if (detailTab !== 'stats' || !user || !userStatsFilters) return;

    const fetchStats = async () => {
      setIsLoadingStats(true);
      try {
        const startDateTime = new Date(userStatsFilters.startDate);
        startDateTime.setHours(0, 0, 0, 0);
        const endDateTime = new Date(userStatsFilters.endDate);
        endDateTime.setHours(23, 59, 59, 999);

        const params: AdvancedStatsParams = {
          startDate: startDateTime.toISOString(),
          endDate: endDateTime.toISOString(),
          interval: userStatsFilters.interval,
          userId: user.id,
        };

        const fetches: Promise<LoanStatsResponse>[] = [api.getLoanStats(params)];
        if (totalLoansAllTime === null) {
          const allTimeStart = new Date('2000-01-01T00:00:00.000Z');
          fetches.push(
            api.getLoanStats({
              startDate: allTimeStart.toISOString(),
              endDate: new Date().toISOString(),
              interval: 'year',
              userId: user.id,
            })
          );
        }

        const results = await Promise.all(fetches);
        setLoanStats(results[0]);
        if (results[1] !== undefined) {
          setTotalLoansAllTime(results[1].totalLoans);
        }
      } catch (error) {
        console.error('Error fetching loan stats:', error);
      } finally {
        setIsLoadingStats(false);
      }
    };

    return deferFromEffect(() => { void fetchStats(); });
  }, [detailTab, user, userStatsFilters, totalLoansAllTime]);

  const formatStatsDate = (dateStr: string) => {
    const interval = userStatsFilters?.interval || 'month';
    if (interval === 'week' && /^\d{4}-W\d{2}$/.test(dateStr)) {
      const [year, week] = dateStr.split('-W');
      return t('stats.weekFormat', { year, week });
    }
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return dateStr;
    switch (interval) {
      case 'year':
        return date.toLocaleDateString(i18n.language, { year: 'numeric' });
      case 'month':
        return date.toLocaleDateString(i18n.language, { month: 'short', year: 'numeric' });
      case 'week':
        return date.toLocaleDateString(i18n.language, { month: 'short', day: 'numeric' });
      case 'day':
      default:
        return date.toLocaleDateString(i18n.language, { day: '2-digit', month: 'short' });
    }
  };

  const handleUserStatsResetFilters = () => {
    setUserStatsFilters(getDefaultStatsFilters());
  };

  const {
    data: holdsPages,
    isLoading: isLoadingHolds,
    isFetchingNextPage: isFetchingNextHolds,
    hasNextPage: hasNextHoldsPage,
    fetchNextPage: fetchNextHoldsPage,
  } = useInfiniteQuery({
    queryKey: ['user-holds', id],
    queryFn: ({ pageParam }) =>
      api.getUserHolds(id!, { page: pageParam, perPage: USER_HOLDS_PAGE_SIZE }),
    initialPageParam: 1,
    getNextPageParam: (lastPage, allPages) => {
      const totalLoaded = allPages.reduce((sum, p) => sum + p.items.length, 0);
      return totalLoaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
    enabled: !!id && detailTab === 'holds',
  });
  const holds = holdsPages?.pages.flatMap((p) => p.items) ?? [];

  useEffect(() => {
    const el = holdsLoadMoreRef.current;
    const scrollRoot = el?.closest('.app-list-scroll') ?? null;
    if (!el || !scrollRoot || !hasNextHoldsPage || isFetchingNextHolds || detailTab !== 'holds') return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) fetchNextHoldsPage();
      },
      { root: scrollRoot, rootMargin: '200px', threshold: 0 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasNextHoldsPage, isFetchingNextHolds, fetchNextHoldsPage, detailTab]);

  const handleCancelHold = async (holdId: string) => {
    setCancellingHoldId(holdId);
    try {
      await api.cancelHold(holdId);
      await queryClient.invalidateQueries({ queryKey: ['user-holds', id] });
    } catch {
      // ignore
    } finally {
      setCancellingHoldId(null);
    }
  };

  const STATS_INTERVALS: { value: StatsInterval; label: string }[] = [
    { value: 'day', label: t('stats.interval.day') },
    { value: 'week', label: t('stats.interval.week') },
    { value: 'month', label: t('stats.interval.month') },
    { value: 'year', label: t('stats.interval.year') },
  ];

  const handleUnlockSignIn = async () => {
    const login = user?.login?.trim() ?? '';
    if (!login || unlockSignInLoading) return;
    setUnlockSignInLoading(true);
    setUnlockSignInError(null);
    try {
      await api.clearAuthRateLimit(login);
      setShowUnlockSignIn(false);
      showToast({
        variant: 'success',
        message: t('users.unlockSignInSuccess', { login }),
      });
    } catch (error: unknown) {
      setUnlockSignInError(getApiErrorMessage(error, t));
    } finally {
      setUnlockSignInLoading(false);
    }
  };

  const handleDelete = async (force = false) => {
    if (!user) return;
    if (deleteUserLoading) return;
    setDeleteUserLoading(true);
    try {
      await api.deleteUser(user.id, force);
      navigate('/users');
    } catch (error: unknown) {
      const code = getApiErrorCode(error);
      const msg =
        (error as { response?: { data?: { message?: string } } })?.response?.data?.message ?? '';
      if (
        !force &&
        (code === 'business_rule_violation' ||
          (typeof msg === 'string' &&
            (msg.includes('active loans') || msg.includes('force=true'))))
      ) {
        setDeleteUserActiveLoansError(true);
      } else {
        console.error('Error deleting user:', error);
      }
    } finally {
      setDeleteUserLoading(false);
    }
  };

  const handleReturnLoan = async (loanId: string) => {
    if (loanActionInFlightRef.current) return;
    loanActionInFlightRef.current = true;
    setLoanAction({ loanId, op: 'return' });
    setRenewError('');
    try {
      await api.returnLoan(loanId);
      await refreshLoans();
      showToast({ variant: 'success', message: t('loans.returnSuccess') });
    } catch (error) {
      const msg = getApiErrorMessage(error, t) || t('loans.errorReturningLoan');
      setRenewError(msg);
      showToast({ variant: 'error', message: msg });
    } finally {
      loanActionInFlightRef.current = false;
      setLoanAction(null);
    }
  };

  const handleRenewLoan = async (loanId: string, force = false) => {
    if (loanActionInFlightRef.current) return;
    loanActionInFlightRef.current = true;
    setLoanAction({ loanId, op: 'renew' });
    setRenewError('');
    setRenewForceCode(null);
    setRenewForceLoanId(null);
    try {
      const renewed = await api.renewLoan(loanId, newIdempotencyKey(), { force: force || undefined });
      await refreshLoans();
      showToast({
        variant: 'success',
        message: renewedUntilMessage(renewed.expiryAt, t, i18n.language),
      });
    } catch (error) {
      const code = getApiErrorCode(error);
      const msg = getApiErrorMessage(error, t) || t('loans.errorRenewingLoan');
      setRenewError(msg);
      if (!force && circulationMayForce('renewal', code)) {
        setRenewForceCode(code);
        setRenewForceLoanId(loanId);
      }
      showToast({ variant: 'error', message: msg });
    } finally {
      loanActionInFlightRef.current = false;
      setLoanAction(null);
    }
  };

  if (isUserError && id) {
    const recordDenied = isLoansForbidden(userQueryError);
    return (
      <div className="max-w-xl mx-auto px-4 py-12">
        <Card>
          <div className="flex items-start gap-3 p-4">
            {recordDenied ? null : (
              <AlertCircle className="h-5 w-5 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
            )}
            <div className="flex-1 space-y-3">
              {recordDenied ? (
                <LoansAccessNotice
                  denied
                  message={t('users.recordNotAllowed')}
                  onRetry={() => undefined}
                  fallbackMessage={t('loans.userLoadError')}
                />
              ) : (
                <p className="text-gray-900 dark:text-gray-100 text-sm">{getApiErrorMessage(userQueryError, t)}</p>
              )}
              <div className="flex flex-wrap gap-2">
                {recordDenied ? null : (
                  <Button type="button" leftIcon={<RefreshCw className="h-4 w-4" />} onClick={() => void refetchUser()}>
                    {t('common.retry')}
                  </Button>
                )}
                <Button type="button" variant="secondary" onClick={() => navigate('/users')}>
                  {t('common.back')}
                </Button>
              </div>
            </div>
          </div>
        </Card>
      </div>
    );
  }

  if (isUserLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="h-8 w-8 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!user) {
    return (
      <div className="text-center py-12">
        <p className="text-gray-500 dark:text-gray-400">{t('users.detailNotFound')}</p>
      </div>
    );
  }

  const overdueLoans = activeLoans.filter((l) => l.isOverdue);
  const signInLogin = user.login?.trim() ?? '';
  const canUnlockSignIn = canManageUsers(currentUser, api.getToken()) && signInLogin !== '';

  const renderLoanTitle = (loan: LoanListItem) => (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        setLoanDetails(loan);
      }}
      className="text-left font-medium text-indigo-600 dark:text-indigo-400 hover:underline"
    >
      {loan.biblio.title || t('items.noTitle')}
    </button>
  );

  const formatAuthor = (author?: Pick<Author, 'firstname' | 'lastname'> | null) => {
    if (!author) return '-';
    return `${author.firstname || ''} ${author.lastname || ''}`.trim() || '-';
  };

  const loanColumnsActive = [
    {
      key: 'title',
      header: t('loans.document'),
      render: (loan: LoanListItem) => (
        <div className="flex items-start gap-3 min-w-0">
          <LoanMediaTypeBadge mediaType={loan.biblio.mediaType} size="table" />
          <div className="min-w-0">
            {renderLoanTitle(loan)}
            <UserRecordLoanIdentity loan={loan} linked={canReadBorrowers} placement="row" />
          </div>
        </div>
      ),
    },
    {
      key: 'date',
      header: t('loans.borrowDate'),
      render: (loan: LoanListItem) =>
        new Date(loan.startDate).toLocaleDateString(i18n.language),
    },
    {
      key: 'expiryAt',
      header: t('loans.dueDate'),
      render: (loan: LoanListItem) => (
        <div className="flex items-center gap-2">
          <span>{new Date(loan.expiryAt).toLocaleDateString(i18n.language)}</span>
        </div>
      ),
    },
    {
      key: 'renews',
      header: t('loans.renewals'),
      render: (loan: LoanListItem) => loan.nbRenews,
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: (loan: LoanListItem) => {
        const claimed = isClaimedReturnedLoan(loan, claimedLoanIds);
        const busy = loanAction?.loanId === loan.id ? loanAction.op : null;
        return (
        <div className="flex flex-col items-end gap-2">
          <div className="flex items-center justify-end gap-2 flex-wrap">
            {!claimed && (
              <Button
                size="sm"
                variant="ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  void handleRenewLoan(loan.id);
                }}
                leftIcon={<RotateCcw className="h-4 w-4" />}
                isLoading={busy === 'renew'}
                disabled={deskBusy}
              >
                {t('loans.renew')}
              </Button>
            )}
            <Button
              size="sm"
              variant="primary"
              onClick={(e) => {
                e.stopPropagation();
                void handleReturnLoan(loan.id);
              }}
              leftIcon={<Check className="h-4 w-4" />}
              isLoading={busy === 'return'}
              disabled={deskBusy}
            >
              {t('loans.return')}
            </Button>
          </div>
          <LoanExceptionActions
            loanId={loan.id}
            loanLabel={loan.biblio.title || undefined}
            claimedReturned={claimed}
            disabled={deskBusy}
            busyKind={busy && busy !== 'return' && busy !== 'renew' ? busy : null}
            onOpen={(kind, label) => exceptionAction.open({ loanId: loan.id, kind, label })}
          />
        </div>
        );
      },
    },
  ];

  const loanColumnsPast = [
    {
      key: 'title',
      header: t('loans.document'),
      render: (loan: LoanListItem) => (
        <div className="flex items-start gap-3 min-w-0">
          <LoanMediaTypeBadge mediaType={loan.biblio.mediaType} size="table" />
          <div className="min-w-0">
            {renderLoanTitle(loan)}
            <UserRecordLoanIdentity loan={loan} linked={canReadBorrowers} placement="row" />
          </div>
        </div>
      ),
    },
    {
      key: 'date',
      header: t('loans.borrowDate'),
      render: (loan: LoanListItem) => new Date(loan.startDate).toLocaleDateString(i18n.language),
    },
    {
      key: 'returnedAt',
      header: t('loans.returnDate'),
      render: (loan: LoanListItem) =>
        loan.returnedAt ? new Date(loan.returnedAt).toLocaleDateString(i18n.language) : '-',
    },
    {
      key: 'closure',
      header: t('loans.closure.column'),
      render: (loan: LoanListItem) => closureReasonText(loan, t, i18n.language) ?? '-',
    },
    {
      key: 'renews',
      header: t('loans.renewals'),
      render: (loan: LoanListItem) => loan.nbRenews,
    },
  ];

  const userHoldColumns = [
    {
      key: 'document',
      header: t('holds.columnDocument'),
      render: (h: Hold) => <HoldDocumentCell hold={h} />,
    },
    {
      key: 'status',
      header: t('holds.status'),
      render: (h: Hold) => (
        <HoldStatusBadge status={h.status} />
      ),
    },
    {
      key: 'pickup',
      header: t('holds.columnPickup'),
      render: (h: Hold) => (
        <HoldPickupCell hold={h} sources={sources} transit={transitByHoldId.get(h.id)} />
      ),
    },
    {
      key: 'position',
      header: t('holds.position'),
      render: (h: Hold) => h.position,
    },
    {
      key: 'created',
      header: t('holds.createdAt'),
      render: (h: Hold) => new Date(h.createdAt).toLocaleString(i18n.language),
    },
    {
      key: 'expires',
      header: t('holds.expiresAt'),
      render: (h: Hold) => (h.expiresAt ? new Date(h.expiresAt).toLocaleString(i18n.language) : '—'),
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: (h: Hold) => (
        <div className="flex flex-wrap justify-end gap-2">
          <HoldTransitActions hold={h} transit={transitByHoldId.get(h.id)} actions={transitActions} />
          {h.status === 'pending' || h.status === 'ready' ? (
            <Button
              size="sm"
              variant="secondary"
              leftIcon={<Ban className="h-4 w-4" />}
              isLoading={cancellingHoldId === h.id}
              onClick={() => void handleCancelHold(h.id)}
            >
              {t('holds.cancelHold')}
            </Button>
          ) : null}
        </div>
      ),
    },
  ];

  const statsTabContent = (
    <div className="space-y-6">
      {userStatsFilters && (
        <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50 space-y-4 mb-6">
          <div className="flex items-center gap-2">
            <Filter className="h-5 w-5 text-gray-400" />
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              {t('stats.advancedFilters')}
            </span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div>
              <label className={formLabelClass()}>
                {t('stats.startDate')}
              </label>
              <Input
                type="date"
                value={userStatsFilters.startDate}
                onChange={(e) => setUserStatsFilters({ ...userStatsFilters, startDate: e.target.value })}
              />
            </div>
            <div>
              <label className={formLabelClass()}>
                {t('stats.endDate')}
              </label>
              <Input
                type="date"
                value={userStatsFilters.endDate}
                onChange={(e) => setUserStatsFilters({ ...userStatsFilters, endDate: e.target.value })}
              />
            </div>
            <div>
              <label className={formLabelClass()}>
                {t('stats.intervalLabel')}
              </label>
              <select
                value={userStatsFilters.interval}
                onChange={(e) =>
                  setUserStatsFilters({
                    ...userStatsFilters,
                    interval: e.target.value as StatsInterval,
                  })
                }
                className={formControlClass({ className: 'w-full' })}
              >
                {STATS_INTERVALS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="flex items-end">
              <Button
                variant="ghost"
                size="sm"
                onClick={handleUserStatsResetFilters}
                leftIcon={<X className="h-4 w-4" />}
              >
                {t('common.reset')}
              </Button>
            </div>
          </div>
        </div>
      )}

      {isLoadingStats ? (
        <div className="flex items-center justify-center h-64">
          <div className="h-8 w-8 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
        </div>
      ) : loanStats ? (
        <div className="space-y-4">
          <div className="flex items-center gap-4 flex-wrap">
            {totalLoansAllTime !== null && (
              <div className="p-3 rounded-lg bg-indigo-50 dark:bg-indigo-900/20">
                <p className="text-2xl font-bold text-indigo-600 dark:text-indigo-400">{totalLoansAllTime}</p>
                <p className="text-xs text-indigo-700 dark:text-indigo-300">{t('users.totalLoansLabel')}</p>
              </div>
            )}
            <div className="p-3 rounded-lg bg-green-50 dark:bg-green-900/20">
              <p className="text-2xl font-bold text-green-600 dark:text-green-400">{loanStats.totalReturns}</p>
              <p className="text-xs text-green-700 dark:text-green-300">{t('users.returnsInPeriod')}</p>
            </div>
            <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-900/20">
              <p className="text-2xl font-bold text-blue-600 dark:text-blue-400">{loanStats.totalLoans}</p>
              <p className="text-xs text-blue-700 dark:text-blue-300">{t('users.loansInPeriod')}</p>
            </div>
          </div>

          <Suspense
            fallback={
              <div className="flex items-center justify-center h-72">
                <div className="h-8 w-8 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
              </div>
            }
          >
            <LoansAreaChart
              data={loanStats.timeSeries.map((item) => ({
                date: item.period,
                loans: item.loans,
                returns: item.returns,
              }))}
              formatTick={formatStatsDate}
              formatTooltipLabel={(labelStr) => {
                const interval = userStatsFilters?.interval || 'month';
                if (interval === 'week' && /^\d{4}-W\d{2}$/.test(labelStr)) {
                  const [year, week] = labelStr.split('-W');
                  return t('stats.weekFormat', { year, week });
                }
                const date = new Date(labelStr);
                if (isNaN(date.getTime())) return labelStr;
                return date.toLocaleDateString(i18n.language, {
                  weekday: interval === 'day' ? 'long' : undefined,
                  day: 'numeric',
                  month: 'long',
                  year: interval === 'year' ? 'numeric' : undefined,
                });
              }}
              loansName={t('stats.chart.loans')}
              returnsName={t('stats.chart.returns')}
              heightClass="h-72"
              gradientPrefix="userLoans"
            />
          </Suspense>
        </div>
      ) : (
        <p className="text-center text-gray-500 dark:text-gray-400 py-8">{t('users.noStatsAvailable')}</p>
      )}
    </div>
  );

  return (
    <div className="flex flex-col min-h-0 gap-4 h-[calc(100dvh-9rem)] lg:h-[calc(100dvh-5.5rem)]">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-start sm:justify-between gap-4">
        <div className="flex items-start gap-4">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={() => navigate('/users')}
            aria-label={t('common.back')}
            title={t('common.back')}
          >
            <ArrowLeft className="h-5 w-5" />
          </Button>
          <div className="flex-shrink-0 h-16 w-16 rounded-xl bg-indigo-100 dark:bg-indigo-900/50 flex items-center justify-center">
            <span className="text-xl font-bold text-indigo-600 dark:text-indigo-400">
              {user.firstname?.[0] || '?'}{user.lastname?.[0] || ''}
            </span>
          </div>
          <div>
            <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white">
              {user.firstname} {user.lastname}
            </h1>
            <div className="flex items-center gap-2 mt-1">
              <Badge>{localizedAccountTypeLabel(t, user.accountType, accountTypes)}</Badge>
              {(() => {
                const publicLabel = localizedPublicTypeById(t, publicTypes, user.publicType);
                return publicLabel ? <Badge variant="info">{publicLabel}</Badge> : null;
              })()}
              {overdueLoans.length > 0 && (
                <Badge variant="danger">{t('loans.overdueCount', { count: overdueLoans.length })}</Badge>
              )}
            </div>
          </div>
        </div>

        <div className="flex flex-wrap gap-2 w-full sm:w-auto sm:justify-end">
          <Button variant="secondary" onClick={() => setShowBorrowModal(true)} leftIcon={<BookMarked className="h-4 w-4" />}>
            {t('loans.borrow')}
          </Button>
          {canReadLoans(currentUser, api.getToken()) && <LoansMarcExportButton userId={user.id} />}
          {user.expiryAt != null && isSubscriptionExpired(user.expiryAt) && (
            <Button
              type="button"
              variant="secondary"
              onClick={() => setShowRenewSubscriptionModal(true)}
              leftIcon={<RefreshCw className="h-4 w-4" />}
            >
              {t('users.renewSubscription')}
            </Button>
          )}
          {canUnlockSignIn && (
            <Button
              type="button"
              variant="secondary"
              onClick={() => {
                setUnlockSignInError(null);
                setShowUnlockSignIn(true);
              }}
              leftIcon={<LockOpen className="h-4 w-4" />}
            >
              {t('users.unlockSignIn')}
            </Button>
          )}
          <Button variant="danger" onClick={() => setShowDeleteModal(true)} leftIcon={<Trash2 className="h-4 w-4" />}>
            {t('users.anonymize')}
          </Button>
        </div>
      </div>

      <div className="flex flex-col flex-1 min-h-0 min-w-0">
        <Card padding="none" className="flex flex-col flex-1 min-h-0 min-w-0">
          <div className="p-3 sm:p-4 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
            <div className="flex flex-nowrap items-center justify-between gap-3 min-w-0">
              <div className="overflow-x-auto min-w-0 flex-1 min-h-0 -mx-0.5 px-0.5">
                <div
                  className="inline-flex flex-wrap gap-1 rounded-lg border border-gray-200 dark:border-gray-700 p-0.5 bg-gray-50 dark:bg-gray-900"
                  role="tablist"
                  aria-label={t('users.title')}
                >
                  {(
                    [
                      { id: 'info' as UserDetailTab, label: t('users.information') },
                      { id: 'activeLoans' as UserDetailTab, label: t('loans.activeLoans') },
                      { id: 'pastLoans' as UserDetailTab, label: t('users.pastLoansTab') },
                      { id: 'fines' as UserDetailTab, label: t('users.finesTab') },
                      { id: 'holds' as UserDetailTab, label: t('users.holdsTab') },
                      { id: 'stats' as UserDetailTab, label: t('users.loanStatistics') },
                    ].filter((tab) => {
                      if (tab.id === 'fines') return finesVisible;
                      if (tab.id === 'pastLoans') return canSeeLoanHistory;
                      if (tab.id === 'activeLoans') return canListReaderLoans;
                      return true;
                    })
                  ).map((tab) => (
                    <button
                      key={tab.id}
                      type="button"
                      role="tab"
                      aria-selected={detailTab === tab.id}
                      onClick={() => setDetailTab(tab.id)}
                      className={`px-3 py-1.5 text-sm font-medium rounded-md whitespace-nowrap ${
                        detailTab === tab.id
                          ? 'bg-white dark:bg-gray-800 text-amber-700 dark:text-amber-400 shadow-sm'
                          : 'text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white'
                      }`}
                    >
                      {tab.label}
                    </button>
                  ))}
                </div>
              </div>
              {user.createdAt && (
                <p className="text-sm text-gray-500 dark:text-gray-400 flex items-center gap-2 shrink-0 whitespace-nowrap sm:ml-2">
                  <Calendar className="h-4 w-4 shrink-0" aria-hidden />
                  <span>
                    {t('users.createdAt')}:{' '}
                    {new Date(user.createdAt).toLocaleDateString(i18n.language, {
                      day: 'numeric',
                      month: 'long',
                      year: 'numeric',
                    })}
                  </span>
                </p>
              )}
            </div>
          </div>

          {renewError && detailTab === 'activeLoans' && (
            <div className="px-4 sm:px-6 pt-3 flex-shrink-0">
              <div
                role="alert"
                className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 flex items-start gap-2"
              >
                <AlertTriangle className="h-4 w-4 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" />
                <div className="min-w-0 flex-1 space-y-2">
                  <p className="text-sm font-medium text-red-800 dark:text-red-200">{renewError}</p>
                  <CirculationForceButton
                    surface="renewal"
                    code={renewForceCode}
                    disabled={loanAction?.op === 'renew'}
                    onForce={() => {
                      if (renewForceLoanId) void handleRenewLoan(renewForceLoanId, true);
                    }}
                  />
                </div>
              </div>
            </div>
          )}

          {detailTab === 'info' && (
            <>
              <ScrollableListRegion
                aria-label={t('users.information')}
                className="flex-1 min-h-0 !max-h-none overflow-auto px-3 sm:px-4 py-3"
              >
                <UserEditorForm
                  key={`${user.id}-${user.updateAt ?? ''}`}
                  mode="edit"
                  formId="user-detail-edit-form"
                  user={user}
                  publicTypes={publicTypes}
                  accountTypes={accountTypes}
                  onLoadingChange={setIsEditLoading}
                  onSuccess={(updatedUser) => {
                    if (updatedUser) queryClient.setQueryData<UserType>(['user', id], updatedUser);
                  }}
                />
              </ScrollableListRegion>
              <div className="flex-shrink-0 px-3 sm:px-4 py-2 border-t border-gray-200 dark:border-gray-800 flex justify-end">
                <Button type="submit" form="user-detail-edit-form" isLoading={isEditLoading}>
                  {t('common.save')}
                </Button>
              </div>
            </>
          )}

          {detailTab === 'activeLoans' && (
            <ScrollableListRegion
              aria-label={t('loans.activeLoans')}
              className="flex-1 min-h-0 !max-h-none overflow-auto"
            >
              {isActiveLoansError ? (
                <div className="p-4">
                  <LoansAccessNotice
                    error={activeLoansError}
                    onRetry={() => {
                      void refetchActiveLoans();
                    }}
                    fallbackMessage={t('loans.loansLoadError')}
                  />
                </div>
              ) : (
                <>
                  <Table
                    columns={loanColumnsActive}
                    data={activeLoans}
                    keyExtractor={(loan) => loan.id}
                    isLoading={isLoadingActiveLoans && activeLoans.length === 0}
                    emptyMessage={t('loans.noLoans')}
                  />
                  <div ref={activeLoansLoadMoreRef} className="h-4 flex-shrink-0" aria-hidden />
                  {isFetchingNextActiveLoans && (
                    <div className="flex items-center justify-center gap-2 py-4 text-sm text-gray-500 dark:text-gray-400">
                      <Loader2 className="h-5 w-5 animate-spin" />
                      <span>{t('common.loading')}</span>
                    </div>
                  )}
                </>
              )}
            </ScrollableListRegion>
          )}

          {detailTab === 'pastLoans' && (
            <ScrollableListRegion
              aria-label={t('users.pastLoansTab')}
              className="flex-1 min-h-0 !max-h-none overflow-auto"
            >
              {isPastLoansError ? (
                <div className="p-4">
                  <LoansAccessNotice
                    error={pastLoansError}
                    onRetry={() => {
                      void refetchPastLoans();
                    }}
                    fallbackMessage={t('loans.loansLoadError')}
                  />
                </div>
              ) : (
                <>
                  <Table
                    columns={loanColumnsPast}
                    data={pastLoans}
                    keyExtractor={(loan) => loan.id}
                    isLoading={isLoadingPastLoans && pastLoans.length === 0}
                    emptyMessage={t('loans.noLoans')}
                  />
                  <div ref={pastLoansLoadMoreRef} className="h-4 flex-shrink-0" aria-hidden />
                  {isFetchingNextPastLoans && (
                    <div className="flex items-center justify-center gap-2 py-4 text-sm text-gray-500 dark:text-gray-400">
                      <Loader2 className="h-5 w-5 animate-spin" />
                      <span>{t('common.loading')}</span>
                    </div>
                  )}
                </>
              )}
            </ScrollableListRegion>
          )}

          {detailTab === 'fines' && finesVisible && (
            <ScrollableListRegion
              aria-label={t('users.finesTab')}
              className="flex-1 min-h-0 !max-h-none overflow-auto"
            >
              <PatronFinesPanel
                fines={finesData?.fines ?? []}
                totalUnpaid={finesData?.totalUnpaid}
                isLoading={isLoadingFines}
                errorMessage={
                  isFinesError
                    ? getApiErrorMessage(finesError, t) || t('users.fines.loadError')
                    : null
                }
                onRetry={() => {
                  void refetchFines();
                }}
              />
            </ScrollableListRegion>
          )}

          {detailTab === 'holds' && (
            <>
              <ScrollableListRegion aria-label={t('users.holdsTab')} className="flex-1 min-h-0 !max-h-none overflow-auto">
                <>
                  <Table
                    columns={userHoldColumns}
                    data={holds}
                    keyExtractor={(h) => h.id}
                    isLoading={isLoadingHolds && holds.length === 0}
                    emptyMessage={t('holds.noHolds')}
                  />
                  <div ref={holdsLoadMoreRef} className="h-4 flex-shrink-0" aria-hidden />
                  {isFetchingNextHolds && (
                    <div className="flex items-center justify-center gap-2 py-4 text-sm text-gray-500 dark:text-gray-400">
                      <Loader2 className="h-5 w-5 animate-spin" />
                      <span>{t('common.loading')}</span>
                    </div>
                  )}
                </>
              </ScrollableListRegion>
            </>
          )}

          {detailTab === 'stats' && (
            <ScrollableListRegion
              aria-label={t('users.loanStatistics')}
              className="flex-1 min-h-0 !max-h-none overflow-auto px-4 sm:px-6 py-4"
            >
              {statsTabContent}
            </ScrollableListRegion>
          )}
        </Card>
      </div>

      {/* Loan details modal */}
      <Modal
        isOpen={loanDetails !== null}
        onClose={() => setLoanDetails(null)}
        title={t('loans.loanDetails')}
        size="md"
      >
        {loanDetails && (
          <div className="space-y-4">
            <div>
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('loans.document')}</p>
              <div className="flex items-start gap-3 mt-1">
                <LoanMediaTypeBadge mediaType={loanDetails.biblio.mediaType} size="table" />
                <div className="min-w-0">
                  <p className="text-gray-900 dark:text-white font-medium">
                    {loanDetails.biblio.title || t('items.noTitle')}
                  </p>
                  <p className="text-sm text-gray-600 dark:text-gray-300">
                    {formatAuthor(loanDetails.biblio.author)}
                  </p>
                  {loanDetails.biblio.isbn && (
                    <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
                      {t('items.isbn')}: <span className="font-mono">{formatIsbnDisplay(loanDetails.biblio.isbn)}</span>
                    </p>
                  )}
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <p className="text-xs text-gray-500 dark:text-gray-400">{t('loans.loanDate')}</p>
                <p className="text-gray-900 dark:text-white">
                  {new Date(loanDetails.startDate).toLocaleDateString(i18n.language)}
                </p>
              </div>
              <div>
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  {loanDetails.returnedAt ? t('loans.returnDate') : t('loans.dueDate')}
                </p>
                <p className="text-gray-900 dark:text-white">
                  {loanDetails.returnedAt
                    ? new Date(loanDetails.returnedAt).toLocaleDateString(i18n.language)
                    : new Date(loanDetails.expiryAt).toLocaleDateString(i18n.language)}
                </p>
              </div>
              {closureReasonText(loanDetails, t, i18n.language) ? (
                <div>
                  <p className="text-xs text-gray-500 dark:text-gray-400">{t('loans.closure.column')}</p>
                  <p className="text-gray-900 dark:text-white">
                    {closureReasonText(loanDetails, t, i18n.language)}
                  </p>
                </div>
              ) : null}
            </div>

            <div className="pt-2 border-t border-gray-200 dark:border-gray-800">
              <UserRecordLoanIdentity loan={loanDetails} linked={canReadBorrowers} placement="detail" />
            </div>

            <div className="pt-2 border-t border-gray-200 dark:border-gray-800">
              <p className="text-xs text-gray-500 dark:text-gray-400">{t('loans.borrowedSpecimen')}</p>
              {(() => {
                const ident = loanDetails.itemIdentification;
                const spec = loanDetails.biblio?.items?.find(
                  (s) => (s.barcode != null && ident != null && s.barcode === ident) || s.id === ident
                );
                const barcode = spec?.barcode ?? ident ?? '-';
                const sourceName = spec?.sourceName ?? '-';
                return (
                  <div className="mt-1 space-y-1">
                    <p className="text-sm text-gray-700 dark:text-gray-300">
                      {t('items.barcode')}: <span className="font-mono">{barcode}</span>
                    </p>
                    <p className="text-sm text-gray-700 dark:text-gray-300">
                      {t('items.source')}: <span className="font-medium">{sourceName}</span>
                    </p>
                    {spec?.callNumber && (
                      <p className="text-sm text-gray-700 dark:text-gray-300">
                        {t('items.callNumber')}: <span className="font-mono">{spec.callNumber}</span>
                      </p>
                    )}
                  </div>
                );
              })()}
            </div>
          </div>
        )}
      </Modal>

      <ConfirmDialog
        isOpen={showDeleteModal}
        onClose={() => {
          if (deleteUserLoading) return;
          setShowDeleteModal(false);
          setDeleteUserActiveLoansError(false);
        }}
        onConfirm={() => {
          void handleDelete(deleteUserActiveLoansError);
        }}
        title={t('common.confirm')}
        message={
          deleteUserActiveLoansError
            ? t('users.activeLoansForceDelete')
            : [
                t('users.deleteConfirm', { name: `${user.firstname} ${user.lastname}`.trim() }),
                activeLoansTotal > 0 ? t('users.hasLoansWarning', { count: activeLoansTotal }) : null,
              ]
                .filter(Boolean)
                .join('\n\n')
        }
        confirmLabel={deleteUserActiveLoansError ? t('users.forceDelete') : t('users.anonymize')}
        confirmVariant="danger"
        isLoading={deleteUserLoading}
      />

      <ConfirmDialog
        isOpen={showUnlockSignIn}
        onClose={() => {
          if (unlockSignInLoading) return;
          setShowUnlockSignIn(false);
          setUnlockSignInError(null);
        }}
        onConfirm={() => {
          void handleUnlockSignIn();
        }}
        title={t('users.unlockSignIn')}
        message={t('users.unlockSignInConfirm', { login: signInLogin })}
        confirmLabel={t('users.unlockSignIn')}
        isLoading={unlockSignInLoading}
        error={unlockSignInError}
      />

      <RenewSubscriptionModal
        user={showRenewSubscriptionModal && user ? user : null}
        isOpen={showRenewSubscriptionModal}
        onClose={() => setShowRenewSubscriptionModal(false)}
        onSuccess={(updated) => {
          queryClient.setQueryData<UserType>(['user', id], updated);
        }}
      />

      {/* Borrow modal */}
      <Modal
        isOpen={showBorrowModal}
        onClose={() => setShowBorrowModal(false)}
        title={t('loans.newLoan')}
        footer={
          <div className="flex justify-end gap-2">
            <Button type="submit" form="borrow-user-form" isLoading={isBorrowLoading}>
              {t('loans.borrow')}
            </Button>
          </div>
        }
      >
        <BorrowForm
          formId="borrow-user-form"
          userId={user.id}
          onLoadingChange={setIsBorrowLoading}
          onSuccess={() => {
            setShowBorrowModal(false);
            refreshLoans();
            showToast({ variant: 'success', message: t('loans.checkoutSuccessShort') });
          }}
          onBusinessRuleViolation={(msg, specimenCode) => {
            setShowBorrowModal(false);
            setForceBorrowDialog({
              message: `${msg}\n\n${t('loans.forceBorrowConfirm')}`,
              specimenCode,
            });
          }}
          onToPickForOther={(identity, specimenCode) => {
            setShowBorrowModal(false);
            setToPickForce({ ...identity, specimenCode });
          }}
        />
      </Modal>

      <ToPickForceLoanDialog
        isOpen={toPickForce !== null}
        cardNumber={toPickForce?.cardNumber ?? null}
        initials={toPickForce?.initials ?? null}
        isLoading={toPickForceLoading}
        onCancel={() => {
          if (!toPickForceLoading) setToPickForce(null);
        }}
        onSetAside={() => {
          const ctx = toPickForce;
          if (!ctx || toPickForceLoading) return;
          setToPickForceLoading(true);
          void (async () => {
            try {
              const result = await api.returnLoanByBarcode(ctx.specimenCode);
              const pick = interpretPickCheckIn(result);
              setToPickForce(null);
              const message = pick
                ? pick.kind === 'ready'
                  ? t('loans.pickReady', { patron: pick.patronName?.trim() || t('loans.pickPatronUnknown') })
                  : t('loans.pickTransit', { site: pick.siteName?.trim() || t('loans.pickSiteUnknown') })
                : t('loans.returnSuccess');
              showToast({ variant: 'success', message });
            } catch (e) {
              setBorrowForceError(getApiErrorMessage(e, t) || t('loans.errorReturningLoan'));
              setToPickForce(null);
            } finally {
              setToPickForceLoading(false);
            }
          })();
        }}
      />

      <ConfirmDialog
        isOpen={forceBorrowDialog !== null}
        onClose={() => setForceBorrowDialog(null)}
        onConfirm={() => {
          const ctx = forceBorrowDialog;
          setForceBorrowDialog(null);
          if (!ctx) return;
          void (async () => {
            try {
              await api.createLoan({
                userId: user.id,
                itemIdentification: ctx.specimenCode,
                force: true,
              }, newIdempotencyKey());
              await refreshLoans();
              showToast({ variant: 'success', message: t('loans.checkoutSuccessShort') });
            } catch (e) {
              setBorrowForceError(getApiErrorMessage(e, t) || t('loans.errorCreatingLoan'));
            }
          })();
        }}
        message={forceBorrowDialog?.message ?? ''}
        confirmLabel={t('loans.forceLoan')}
        stackOnTop
      />

      <MessageModal
        isOpen={borrowForceError !== null}
        onClose={() => setBorrowForceError(null)}
        message={borrowForceError ?? ''}
        stackOnTop
      />

      <CirculationExceptionDialog
        target={exceptionAction.target}
        isLoading={exceptionAction.isLoading}
        error={exceptionAction.error}
        onClose={exceptionAction.close}
        onConfirm={(payload) => void exceptionAction.submit(payload)}
      />
      <TransitActionModals actions={transitActions} />
    </div>
  );
}

interface BorrowFormProps {
  formId: string;
  userId: string;
  onLoadingChange: (loading: boolean) => void;
  onSuccess: () => void;
  onBusinessRuleViolation?: (message: string, specimenCode: string) => void;
  onToPickForOther?: (
    identity: { cardNumber: string | null; initials: string | null },
    specimenCode: string,
  ) => void;
}

function BorrowForm({
  formId,
  userId,
  onLoadingChange,
  onSuccess,
  onBusinessRuleViolation,
  onToPickForOther,
}: BorrowFormProps) {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const [specimenCode, setSpecimenCode] = useState('');
  const [error, setError] = useState('');

  const doCreateLoan = async (force = false) => {
    try {
      await api.createLoan({
        userId: userId,
        itemIdentification: specimenCode,
        force: force || undefined,
      }, newIdempotencyKey());
      setError('');
      onSuccess();
    } catch (err: unknown) {
      const axiosErr = err as { response?: { data?: { code?: string; message?: string } } };
      const axiosData = axiosErr?.response?.data;
      const errorCode = axiosData?.code ?? '';
      const rawMessage = typeof axiosData?.message === 'string' ? axiosData.message : '';
      const displayMsg = getApiErrorMessage(err, t, { allowRawMessage: false }) || t('loans.errorCreatingLoan');
      if (isWeedingServerMessage(rawMessage)) {
        setError(displayMsg);
        showToast({ variant: 'error', message: displayMsg });
        return;
      }
      const toPick = readItemToPickForOther(err);
      if (toPick && !force) {
        onToPickForOther?.(
          { cardNumber: toPick.cardNumber, initials: toPick.initials },
          specimenCode.trim(),
        );
        return;
      }
      if (errorCode === 'card_lost') {
        setError(displayMsg);
        showToast({ variant: 'error', message: displayMsg });
        return;
      }
      if (circulationMayForce('loan', errorCode) && !force) {
        onBusinessRuleViolation?.(displayMsg, specimenCode.trim());
        return;
      }
      setError(displayMsg);
      showToast({ variant: 'error', message: displayMsg });
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!specimenCode.trim()) return;
    setError('');
    onLoadingChange(true);
    try {
      await doCreateLoan();
    } finally {
      onLoadingChange(false);
    }
  };

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4">
      <BarcodeScanField
        label={t('loans.specimenBarcode')}
        value={specimenCode}
        onChange={(e) => {
          setSpecimenCode(e.target.value);
          if (error) setError('');
        }}
        placeholder={t('loans.scanOrEnterBarcode')}
        autoFocus
        required
        scannerTitle={t('loans.scanSpecimenBarcode')}
        onCameraScan={(code) => {
          setSpecimenCode(code);
          if (error) setError('');
        }}
      />
      {error && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">{error}</p>
      )}
    </form>
  );
}


