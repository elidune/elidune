import { useState, useEffect, useRef, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Plus, Loader2, AlertCircle, Users } from 'lucide-react';
import { useInfiniteQuery, useQueryClient } from '@tanstack/react-query';
import { invalidateUsersListQueries, USERS_LIST_QUERY_ROOT } from '@/hooks/users/queryKeys';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import { useAccountTypesQuery } from '@/hooks/useAccountTypesQuery';
import {
  Card,
  Button,
  SearchInput,
  ConfirmDialog,
  ScrollableListRegion,
  ResponsiveRecordList,
  ListSkeleton,
  MultiSelect,
  PageHeading,
  ListPageLayout,
  ListResultCountBar,
} from '@/components/common';
import { getApiErrorCode, getApiErrorMessage } from '@/utils/apiError';
import { getListScrollRoot } from '@/utils/listScrollRoot';
import { patronId } from '@/utils/patronLink';
import { RenewSubscriptionModal, UserListCard, UsersDesktopList } from '@/components/users';
import api from '@/services/api';
import { canManageUsers, type UserShort } from '@/types';
import { useAuth } from '@/contexts/AuthContext';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import { localizedPublicTypeLabel } from '@/utils/publicTypeDisplay';
import { formControlClass, formLabelClass } from '@/utils/formControl';
const USERS_PER_PAGE = 20;

const CIRCULATION_FILTERS = ['subscriptionExpired', 'overdueLoans', 'unpaidFines'] as const;
type CirculationFilter = (typeof CIRCULATION_FILTERS)[number];

type UsersListFilters = {
  accountType: string;
  publicType: string;
  circulation: CirculationFilter[];
};

const emptyUsersListFilters = (): UsersListFilters => ({
  accountType: '',
  publicType: '',
  circulation: [],
});

export default function UsersPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { user: currentUser } = useAuth();
  const canCreateUser = canManageUsers(currentUser, api.getToken());
  const { data: publicTypes = [] } = usePublicTypesQuery();
  const accountTypesQuery = useAccountTypesQuery();
  const accountTypes = accountTypesQuery.data ?? [];

  const [searchQuery, setSearchQuery] = useState('');
  const [searchDraft, setSearchDraft] = useState('');
  const [listFilters, setListFilters] = useState<UsersListFilters>(emptyUsersListFilters);
  const [renewModalUser, setRenewModalUser] = useState<UserShort | null>(null);
  const [deleteModalUser, setDeleteModalUser] = useState<UserShort | null>(null);
  const [deleteUserForce, setDeleteUserForce] = useState(false);
  const [deleteUserLoading, setDeleteUserLoading] = useState(false);
  const [deleteUserError, setDeleteUserError] = useState<string | null>(null);

  const {
    data,
    isLoading,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
    isError: isUsersQueryError,
    error: usersQueryError,
    refetch: refetchUsers,
  } = useInfiniteQuery({
    queryKey: [...USERS_LIST_QUERY_ROOT, searchQuery, listFilters],
    queryFn: async ({ pageParam }) =>
      api.getUsers({
        name: searchQuery || undefined,
        accountType: listFilters.accountType || undefined,
        publicType: listFilters.publicType ? Number(listFilters.publicType) : undefined,
        subscriptionExpired: listFilters.circulation.includes('subscriptionExpired') || undefined,
        overdueLoans: listFilters.circulation.includes('overdueLoans') || undefined,
        unpaidFines: listFilters.circulation.includes('unpaidFines') || undefined,
        page: pageParam,
        perPage: USERS_PER_PAGE,
      }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      if (!lastPage?.items?.length) return undefined;
      const loaded = lastPage.page * lastPage.perPage;
      return loaded < lastPage.total ? lastPage.page + 1 : undefined;
    },
  });

  const users = data?.pages.flatMap((p) => p.items) ?? [];
  const totalUsers = data?.pages[0]?.total ?? 0;

  const loadMoreRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = loadMoreRef.current;
    const scrollRoot = getListScrollRoot(el);
    if (!el || !scrollRoot || !hasNextPage || isFetchingNextPage || !data?.pages?.length) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) fetchNextPage();
      },
      { root: scrollRoot, rootMargin: '200px', threshold: 0 }
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, data?.pages?.length]);

  const handleSearch = (value: string) => {
    setSearchQuery(value);
    setSearchDraft(value);
  };

  const handleRowClick = (user: UserShort) => {
    const id = patronId(user.id);
    if (id) navigate(`/users/${id}`);
  };

  const openRenewModal = (user: UserShort) => setRenewModalUser(user);

  const openDeleteModal = (user: UserShort) => {
    setDeleteUserForce(false);
    setDeleteUserError(null);
    setDeleteModalUser(user);
  };

  const confirmDeleteUser = async () => {
    if (!deleteModalUser) return;
    setDeleteUserLoading(true);
    setDeleteUserError(null);
    try {
      await api.deleteUser(deleteModalUser.id, deleteUserForce);
      setDeleteModalUser(null);
      setDeleteUserForce(false);
      await invalidateUsersListQueries(queryClient);
    } catch (error: unknown) {
      const code = getApiErrorCode(error);
      const msg =
        (error as { response?: { data?: { message?: string } } })?.response?.data?.message ?? '';
      if (
        !deleteUserForce &&
        (code === 'business_rule_violation' ||
          code === 'conflict' ||
          (typeof msg === 'string' &&
            (msg.includes('active loans') || msg.includes('force=true'))))
      ) {
        setDeleteUserForce(true);
      } else {
        setDeleteUserError(getApiErrorMessage(error, t));
      }
    } finally {
      setDeleteUserLoading(false);
    }
  };

  const circulationFilterOptions = useMemo(
    () =>
      CIRCULATION_FILTERS.map((value) => ({
        value,
        label: t(`users.listFilters.${value}`),
      })),
    [t],
  );

  return (
    <ListPageLayout>
      <PageHeading
        className="shrink-0"
        icon={Users}
        title={t('users.title')}
        subtitle={t('users.pageSubtitle')}
        actions={
          canCreateUser ? (
            <Button onClick={() => navigate('/users/new')} leftIcon={<Plus className="h-4 w-4" />}>
              {t('users.newUser')}
            </Button>
          ) : undefined
        }
      />

      <Card className="shrink-0">
        <div className="space-y-4">
          <SearchInput
            value={searchDraft}
            onChange={setSearchDraft}
            placeholder={t('users.searchPlaceholder')}
            submitMode
            onSubmit={handleSearch}
            submitLabel={t('common.search')}
          />
          <div className="mt-4 rounded-lg border border-gray-200 bg-gray-50/80 p-4 dark:border-gray-700 dark:bg-gray-800/40">
            <div className="flex flex-col gap-4 xl:flex-row xl:flex-wrap xl:items-end">
              <div className="flex flex-col gap-1.5 w-full sm:w-[13rem] shrink-0">
                <label
                  htmlFor="users-filter-account-type"
                  className={formLabelClass({ marginBottom: false })}
                >
                  {t('users.listFilters.accountType')}
                </label>
                <select
                  id="users-filter-account-type"
                  value={listFilters.accountType}
                  onChange={(e) =>
                    setListFilters((prev) => ({ ...prev, accountType: e.target.value }))
                  }
                  className={formControlClass({ className: 'w-full' })}
                >
                  <option value="">{t('users.listFilters.allAccountTypes')}</option>
                  {accountTypes.map((row) => (
                    <option key={row.code} value={row.code}>
                      {localizedAccountTypeLabel(t, row.code, accountTypes)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col gap-1.5 w-full sm:w-[13rem] shrink-0">
                <label
                  htmlFor="users-filter-public-type"
                  className={formLabelClass({ marginBottom: false })}
                >
                  {t('users.listFilters.publicType')}
                </label>
                <select
                  id="users-filter-public-type"
                  value={listFilters.publicType}
                  onChange={(e) =>
                    setListFilters((prev) => ({ ...prev, publicType: e.target.value }))
                  }
                  className={formControlClass({ className: 'w-full' })}
                >
                  <option value="">{t('users.listFilters.allPublicTypes')}</option>
                  {publicTypes.map((pt) => (
                    <option key={pt.id} value={String(pt.id)}>
                      {localizedPublicTypeLabel(t, pt)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="min-w-0 flex-1 w-full xl:min-w-[16rem]">
                <MultiSelect
                  id="users-filter-circulation"
                  label={t('users.listFilters.statusHeading')}
                  options={circulationFilterOptions}
                  value={[...listFilters.circulation]}
                  onChange={(next) =>
                    setListFilters((prev) => ({
                      ...prev,
                      circulation: next.filter((v): v is CirculationFilter =>
                        (CIRCULATION_FILTERS as readonly string[]).includes(v),
                      ),
                    }))
                  }
                  placeholder={t('users.listFilters.circulationPlaceholder')}
                />
              </div>
            </div>
          </div>
        </div>
      </Card>

      <Card padding="none" className="flex min-h-0 flex-1 flex-col">
        {isUsersQueryError && (
          <div
            role="alert"
            className="mx-4 mt-4 rounded-lg border border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/30 px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3"
          >
            <div className="flex items-start gap-2 flex-1 min-w-0 text-sm text-red-800 dark:text-red-200">
              <AlertCircle className="h-5 w-5 shrink-0 mt-0.5" aria-hidden />
              <span>{getApiErrorMessage(usersQueryError, t)}</span>
            </div>
            <Button type="button" size="sm" variant="secondary" onClick={() => void refetchUsers()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        <ListResultCountBar>{t('users.count', { count: totalUsers })}</ListResultCountBar>
        <ScrollableListRegion fill aria-label={t('users.title')}>
          {isLoading && !users.length ? (
            <ListSkeleton rows={10} />
          ) : (
            <>
              <ResponsiveRecordList
                desktop={
                  <UsersDesktopList
                    users={users}
                    accountTypes={accountTypes}
                    publicTypes={publicTypes}
                    emptyMessage={t('users.noUsers')}
                    t={t}
                    onRowClick={handleRowClick}
                    onDelete={openDeleteModal}
                    onRenew={openRenewModal}
                  />
                }
                mobile={
                  users.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                      {t('users.noUsers')}
                    </div>
                  ) : (
                    <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                      {users.map((user) => (
                        <UserListCard
                          key={user.id}
                          user={user}
                          publicTypes={publicTypes}
                          accountTypes={accountTypes}
                          onOpen={() => handleRowClick(user)}
                          onDelete={() => openDeleteModal(user)}
                          onRenew={() => openRenewModal(user)}
                          deleteLabel={t('common.delete')}
                          renewLabel={t('users.renewSubscriptionListAction')}
                          actionsAriaLabel={t('common.actions')}
                        />
                      ))}
                    </div>
                  )
                }
              />
              <div ref={loadMoreRef} className="h-4 flex-shrink-0" aria-hidden />
              {isFetchingNextPage && (
                <div className="flex items-center justify-center gap-2 py-4 text-sm text-gray-500 dark:text-gray-400">
                  <Loader2 className="h-5 w-5 animate-spin" />
                  <span>{t('common.loading')}</span>
                </div>
              )}
            </>
          )}
        </ScrollableListRegion>
      </Card>

      <ConfirmDialog
        isOpen={deleteModalUser !== null}
        onClose={() => {
          if (deleteUserLoading) return;
          setDeleteModalUser(null);
          setDeleteUserForce(false);
          setDeleteUserError(null);
        }}
        onConfirm={() => {
          void confirmDeleteUser();
        }}
        title={t('common.confirm')}
        message={
          deleteUserForce
            ? t('users.activeLoansForceDelete')
            : t('users.deleteConfirm', {
                name: `${deleteModalUser?.firstname ?? ''} ${deleteModalUser?.lastname ?? ''}`.trim() || '—',
              })
        }
        confirmLabel={deleteUserForce ? t('users.forceDelete') : t('common.delete')}
        confirmVariant="danger"
        isLoading={deleteUserLoading}
        error={deleteUserError}
      />

      <RenewSubscriptionModal
        user={renewModalUser}
        isOpen={renewModalUser !== null}
        onClose={() => setRenewModalUser(null)}
        onSuccess={async () => {
          await invalidateUsersListQueries(queryClient);
        }}
      />

    </ListPageLayout>
  );
}
