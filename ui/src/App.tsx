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
  MyLoansPage,
  MyHoldsPage,
  MyPurchaseSuggestionsPage,
  LoansPage,
  HoldsPage,
  PickListPage,
  TransitsPage,
  Z3950SearchPage,
  ProfilePage,
  ImportIsoPage,
  EventsPage,
  UnsubscribeEventsPage,
  PublicEventsPage,
  AboutPage,
  PrivacyPage,
  AcquisitionsPage,
  AcquisitionsOrderDetailPage,
} from '@/pages';
import type { User } from '@/types';
import api from '@/services/api';
import { readerRecordFallback, staffAccess } from '@/utils/staffAccess';
import { FirstSetupGate } from '@/components/first-setup/FirstSetupGate';

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

function RightsRoute({
  allow,
  fallback = '/home',
  children,
}: {
  allow: (user: User | null, token: string | null) => boolean;
  fallback?: string;
  children: React.ReactNode;
}) {
  const { user } = useAuth();
  if (!allow(user, api.getToken())) {
    return <Navigate to={fallback} replace />;
  }
  return <>{children}</>;
}

function accessOf(user: User | null, token: string | null) {
  return staffAccess(user, token);
}

/** Reader records need users read. A desk agent is sent back to circulation. */
function UsersRecordRoute({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();
  const access = accessOf(user, api.getToken());
  if (!access.usersRead) {
    return <Navigate to={readerRecordFallback(access)} replace />;
  }
  return <>{children}</>;
}

/** Shareable `/events`: guests get public chrome; staff EventsPage stays at `/events/manage`. */
function EventsEntryPage() {
  const { isAuthenticated, isLoading } = useAuth();

  if (isLoading) {
    return <FullPageSpinner />;
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
            <RightsRoute allow={(user, token) => accessOf(user, token).internalCatalog} fallback="/catalog">
              <BibliosPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios/new"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).itemsWrite}>
              <BiblioCreatePage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/biblios/:id/edit"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).itemsWrite}>
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
            <RightsRoute allow={(user, token) => accessOf(user, token).itemsWrite}>
              <InventoryPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/weeding"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).weeding}>
              <WeedingPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />
      <Route
        path="/weeding/:id"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).weeding}>
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
            <MyPurchaseSuggestionsPage />
          </ProtectedRoute>
        }
      />

      <Route
        path="/loans"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).loansRead}>
              <LoansPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/holds"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).holdsStaff}>
              <HoldsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/pick-list"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).holdsStaff}>
              <PickListPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/transits"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).holdsStaff}>
              <TransitsPage />
            </RightsRoute>
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
            <RightsRoute allow={(user, token) => accessOf(user, token).stats}>
              <StatsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/settings"
        element={
          <ProtectedRoute>
            <RightsRoute
              allow={(user, token) => {
                const access = accessOf(user, token);
                return access.fullSettings || access.librarySettings;
              }}
            >
              <SettingsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/z3950"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).itemsWrite}>
              <Z3950SearchPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/import-iso"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).itemsWrite}>
              <ImportIsoPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/orders/:id"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <AcquisitionsOrderDetailPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/orders"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <Navigate to="/acquisitions?tab=orders" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/vendors"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <Navigate to="/acquisitions?tab=vendors" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/funds"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <Navigate to="/acquisitions?tab=funds" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions/suggestions"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <Navigate to="/acquisitions?tab=suggestions" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/acquisitions"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).acquisitions}>
              <AcquisitionsPage />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route
        path="/events/manage"
        element={
          <ProtectedRoute>
            <RightsRoute allow={(user, token) => accessOf(user, token).eventsWrite}>
              <EventsPage />
            </RightsRoute>
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
            <RightsRoute allow={(user, token) => accessOf(user, token).librarySettings}>
              <Navigate to="/settings?tab=library" replace />
            </RightsRoute>
          </ProtectedRoute>
        }
      />

      <Route path="/about" element={<AboutPage />} />
      <Route path="/privacy" element={<PrivacyPage />} />

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
