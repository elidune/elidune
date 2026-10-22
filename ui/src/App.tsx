import { lazy, Suspense } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AuthProvider, useAuth } from '@/contexts/AuthContext';
import { ToastProvider } from '@/contexts/ToastContext';
import { BackgroundTasksProvider } from '@/contexts/BackgroundTasksContext';
import ToastContainer from '@/components/common/ToastContainer';
import { ThemeProvider } from '@/contexts/ThemeContext';
import { LanguageProvider } from '@/contexts/LanguageContext';
import { LibraryProvider } from '@/contexts/LibraryContext';
import { Layout, PublicPageChrome } from '@/components/common';
import PublicLanguageSelector from '@/components/common/PublicLanguageSelector';
import {
  FirstSetupPage,
  MaintenancePage,
  LoginPage,
  ForgotPasswordPage,
  ResetPasswordPage,
  MustChangePasswordPage,
  HomePage,
  BibliosPage,
  ReaderCatalogPage,
  BiblioDetailPage,
  BiblioEditPage,
  BiblioCreatePage,
  InventoryPage,
  WeedingPage,
  WithdrawalDetailPage,
  UsersPage,
  UserCreatePage,
  MyLoansPage,
  MyHoldsPage,
  MyPurchaseSuggestionsPage,
  LoansPage,
  HoldsPage,
  PickListPage,
  TransitsPage,
  Z3950SearchPage,
  ProfilePage,
  DepositsPage,
  DepositsLendersRedirect,
  DepositBatchDetailPage,
  EventsPage,
  UnsubscribeEventsPage,
  PublicEventsPage,
  AcquisitionsPage,
  AcquisitionsOrderDetailPage,
} from '@/pages';
import api from '@/services/api';
import { useLibrary } from '@/contexts/LibraryContext';
import { effectiveStaffAccess, isFeatureEnabled, type FeatureKey } from '@/utils/featureAccess';
import { readerRecordFallback, type StaffAccess } from '@/utils/staffAccess';
import { FirstSetupGate } from '@/components/first-setup/FirstSetupGate';
import LegalInfoModalHost, {
  LegalModalAboutRedirect,
  LegalModalPrivacyRedirect,
} from '@/components/legal/LegalInfoModalHost';

const UserDetailPage = lazy(() => import('@/pages/UserDetailPage'));
const StatsPage = lazy(() => import('@/pages/StatsPage'));
const SettingsPage = lazy(() => import('@/pages/SettingsPage'));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5 * 60 * 1000, // 5 minutes
      retry: 1,
    },
  },
});

function FullPageSpinner() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 dark:bg-gray-950">
      <div className="h-10 w-10 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
    </div>
  );
}

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <FullPageSpinner />;
  }

  if (!isAuthenticated) {
    return <Navigate to="/" replace />;
  }

  return <Layout>{children}</Layout>;
}

/** `/` = OPAC login when logged out; `/home` = app dashboard when logged in. */
function RootPage() {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <FullPageSpinner />;
  }

  if (isAuthenticated) {
    return <Navigate to="/home" replace />;
  }

  return <LoginPage />;
}

function useAppAccess() {
  const { user } = useAuth();
  const { libraryInfo } = useLibrary();
  return effectiveStaffAccess(user, api.getToken(), libraryInfo?.enabledFeatures);
}

function RightsRoute({
  allow,
  fallback = '/home',
  children,
}: {
  allow: (access: StaffAccess) => boolean;
  fallback?: string;
  children: React.ReactNode;
}) {
  const access = useAppAccess();
  if (!allow(access)) {
    return <Navigate to={fallback} replace />;
  }
  return <>{children}</>;
}

/** Redirect when an instance module is disabled (same UI for everyone). */
function FeatureRoute({
  feature,
  fallback = '/home',
  children,
}: {
  feature: FeatureKey;
  fallback?: string;
  children: React.ReactNode;
}) {
  const { libraryInfo } = useLibrary();
  if (!isFeatureEnabled(libraryInfo?.enabledFeatures, feature)) {
    return <Navigate to={fallback} replace />;
  }
  return <>{children}</>;
}

/** Reader records need users read. A desk agent is sent back to circulation. */
function UsersRecordRoute({ children }: { children: React.ReactNode }) {
  const access = useAppAccess();
  if (!access.usersRead) {
    return <Navigate to={readerRecordFallback(access)} replace />;
  }
  return <>{children}</>;
}

/** Shareable `/events`: guests get public chrome; staff EventsPage stays at `/events/manage`. */
function EventsEntryPage() {
  const { isAuthenticated, isLoading } = useAuth();
  const { libraryInfo } = useLibrary();

  if (isLoading) {
    return <FullPageSpinner />;
  }

  if (!isFeatureEnabled(libraryInfo?.enabledFeatures, 'events')) {
    return <Navigate to={isAuthenticated ? '/home' : '/'} replace />;
  }

  if (isAuthenticated) {
    return (
      <Layout>
        <PublicEventsPage />
      </Layout>
    );
  }

  return (
    <PublicPageChrome>
      <PublicEventsPage />
    </PublicPageChrome>
  );
}

function AppRoutes() {
  return (
    <Routes>
      <Route path="/first-setup" element={<FirstSetupPage />} />
      <Route path="/maintenance" element={<MaintenancePage />} />
      <Route path="/login" element={<Navigate to="/" replace />} />
      <Route path="/forgot-password" element={<ForgotPasswordPage />} />
      <Route path="/reset-password" element={<ResetPasswordPage />} />
      <Route path="/change-password" element={<MustChangePasswordPage />} />

      <Route path="/" element={<RootPage />} />

      <Route
        path="/home"
        element={
          <ProtectedRoute>
            <HomePage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/catalog"
        element={
          <ProtectedRoute>
            <ReaderCatalogPage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.internalCatalog} fallback="/catalog">
              <BibliosPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios/new"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.itemsWrite}>
              <BiblioCreatePage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios/:id/edit"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.itemsWrite}>
              <BiblioEditPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios/:id"
        element={
          <ProtectedRoute>
            <BiblioDetailPage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/inventory"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="inventory">
              <RightsRoute allow={(access) => access.itemsWrite}>
                <InventoryPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/weeding"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.weeding}>
              <WeedingPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/weeding/:id"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.weeding}>
              <WithdrawalDetailPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/my-loans"
        element={
          <ProtectedRoute>
            <MyLoansPage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/my-holds"
        element={
          <ProtectedRoute>
            <MyHoldsPage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/my-suggestions"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="purchaseSuggestions">
              <MyPurchaseSuggestionsPage />
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/loans"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.loansRead}>
              <LoansPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/holds"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.holdsStaff}>
              <HoldsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/pick-list"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.holdsStaff}>
              <PickListPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/transits"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="transits">
              <RightsRoute allow={(access) => access.holdsStaff}>
                <TransitsPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/profile"
        element={
          <ProtectedRoute>
            <ProfilePage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/users"
        element={
          <ProtectedRoute>
            <UsersRecordRoute>
              <UsersPage />
            </UsersRecordRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/users/new"
        element={
          <ProtectedRoute>
            <UsersRecordRoute>
              <UserCreatePage />
            </UsersRecordRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/users/:id"
        element={
          <ProtectedRoute>
            <UsersRecordRoute>
              <UserDetailPage />
            </UsersRecordRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/stats"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.stats}>
              <StatsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/settings"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.fullSettings || access.librarySettings}>
              <SettingsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/z3950"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="z3950">
              <RightsRoute allow={(access) => access.itemsWrite}>
                <Z3950SearchPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/import-iso"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="importIso">
              <RightsRoute allow={(access) => access.itemsWrite}>
                <Navigate to="/biblios?tab=import" replace />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/deposits/lenders"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="deposits">
              <RightsRoute allow={(access) => access.internalCatalog && access.itemsRead}>
                <DepositsLendersRedirect />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/deposits/:id"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="deposits">
              <RightsRoute allow={(access) => access.internalCatalog && access.itemsRead}>
                <DepositBatchDetailPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/deposits"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="deposits">
              <RightsRoute allow={(access) => access.internalCatalog && access.itemsRead}>
                <DepositsPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/orders/:id"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <AcquisitionsOrderDetailPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/orders"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <Navigate to="/acquisitions?tab=orders" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/vendors"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <Navigate to="/acquisitions?tab=vendors" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/funds"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <Navigate to="/acquisitions?tab=funds" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/suggestions"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <Navigate to="/acquisitions?tab=suggestions" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.acquisitions}>
              <AcquisitionsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/events/manage"
        element={
          <ProtectedRoute>
            <FeatureRoute feature="events">
              <RightsRoute allow={(access) => access.eventsWrite}>
                <EventsPage />
              </RightsRoute>
            </FeatureRoute>
          </ProtectedRoute>
        }
      />

      <Route path="/events/unsubscribe" element={<UnsubscribeEventsPage />} />
      <Route path="/events" element={<EventsEntryPage />} />
      <Route path="/public/events" element={<Navigate to="/events" replace />} />

      <Route
        path="/library"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(access) => access.librarySettings}>
              <Navigate to="/settings?tab=library" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route path="/about" element={<LegalModalAboutRedirect />} />
      <Route path="/privacy" element={<LegalModalPrivacyRedirect />} />

      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <AuthProvider>
          <ToastProvider>
            <BackgroundTasksProvider>
              <LibraryProvider>
                <LanguageProvider>
                  <BrowserRouter>
                    <PublicLanguageSelector />
                    <FirstSetupGate>
                      <Suspense fallback={<FullPageSpinner />}>
                        <AppRoutes />
                      </Suspense>
                      <LegalInfoModalHost />
                      <ToastContainer />
                    </FirstSetupGate>
                  </BrowserRouter>
                </LanguageProvider>
              </LibraryProvider>
            </BackgroundTasksProvider>
          </ToastProvider>
        </AuthProvider>
      </ThemeProvider>
    </QueryClientProvider>
  );
}
