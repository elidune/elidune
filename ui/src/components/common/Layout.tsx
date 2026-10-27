import { useState, useEffect, useLayoutEffect, useRef, useCallback } from 'react';
import { rateLimitWaitMessage } from '@/utils/apiError';
import { Link, useNavigate } from 'react-router-dom';
import LegalModalLink from '@/components/legal/LegalModalLink';
import { useTranslation } from 'react-i18next';
import {
  Users,
  BarChart3,
  Settings,
  LogOut,
  Menu,
  X,
  BookMarked,
  Globe,
  ArrowLeftRight,
  CalendarDays,
  LibraryBig,
  PackageOpen,
  ClipboardList,
  Leaf,
  Bookmark,
  BookmarkCheck,
  PackageSearch,
  Truck,
  ShoppingCart,
  Lightbulb,
  Newspaper,
} from 'lucide-react';
import { useAuth } from '@/contexts/AuthContext';
import Button from './Button';
import { useLibrary } from '@/contexts/LibraryContext';
import { canPatronSelfServiceHolds, canWriteSettings } from '@/types';
import { effectiveStaffAccess, isFeatureEnabled } from '@/utils/featureAccess';
import { useAccountTypesQuery } from '@/hooks/useAccountTypesQuery';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import api from '@/services/api';
import { version as uiVersion } from '../../../package.json';
import {
  BackgroundTasksDrawer,
  BackgroundTasksNavItem,
} from './BackgroundTasksPanel';
import SidebarNavSections, { type SidebarNavSection } from './SidebarNavSections';
import { APP_LIST_SCROLL_ROOT_CLASS } from './ScrollableListRegion';
import { noModalDialogOpen, useFocusTrap } from '@/hooks/common/useFocusTrap';
import { MobilePageTitleProvider } from '@/contexts/MobilePageTitleContext';

interface LayoutProps {
  children: React.ReactNode;
}

function LayoutShell({ children }: LayoutProps) {
  const { t, i18n } = useTranslation();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [serverVersion, setServerVersion] = useState<string | null>(null);
  const sidebarRef = useRef<HTMLElement>(null);
  const closeSidebar = useCallback(() => setSidebarOpen(false), []);
  const sidebarShouldHandleTrap = useCallback((root: HTMLElement) => {
    if (!window.matchMedia('(max-width: 1023px)').matches) return false;
    return noModalDialogOpen() && root.isConnected;
  }, []);
  const { user, logout, profileWarning, dismissProfileWarning, refreshProfile } = useAuth();
  const settingsWrite = canWriteSettings(user, api.getToken());
  const { data: accountTypes = [] } = useAccountTypesQuery({ enabled: settingsWrite });
  const { libraryName, libraryInfo } = useLibrary();
  const features = libraryInfo?.enabledFeatures;
  const access = effectiveStaffAccess(user, api.getToken(), features);
  const showAcquisitions = access.acquisitions;
  const showSerials = access.serials && isFeatureEnabled(features, 'serials');
  const showWeeding = access.weeding;
  const navigate = useNavigate();

  useEffect(() => {
    api.getHealth()
      .then((data) => setServerVersion(data.version ?? null))
      .catch(() => setServerVersion(null));
  }, []);

  useFocusTrap({
    enabled: sidebarOpen,
    containerRef: sidebarRef,
    onEscape: closeSidebar,
    shouldHandle: sidebarShouldHandleTrap,
  });

  const handleLogout = () => {
    logout();
    navigate('/');
  };

  const staffNavSectionIds = new Set(['circulation', 'cataloging', 'admin']);

  const navSections: (SidebarNavSection & {
    items: (SidebarNavSection['items'][number] & { show: boolean })[];
  })[] = [
    {
      id: 'top',
      collapseLabel: t('nav.sectionGeneral'),
      items: [{ name: t('nav.catalog'), href: '/catalog', icon: LibraryBig, show: !access.internalCatalog }],
    },
    {
      id: 'circulation',
      label: t('nav.sectionCirculation'),
      items: [
        { name: t('nav.loans'), href: '/loans', icon: ArrowLeftRight, show: access.loansRead },
        { name: t('nav.holds'), href: '/holds', icon: Bookmark, show: access.holdsStaff },
        { name: t('nav.pickList'), href: '/pick-list', icon: PackageSearch, show: access.holdsStaff },
        {
          name: t('nav.transits'),
          href: '/transits',
          icon: Truck,
          show: access.holdsStaff && isFeatureEnabled(features, 'transits'),
        },
        { name: t('nav.users'), href: '/users', icon: Users, show: access.usersRead },
      ],
    },
    {
      id: 'cataloging',
      label: t('nav.sectionCataloging'),
      items: [
        { name: t('nav.catalog'), href: '/biblios', icon: LibraryBig, show: access.internalCatalog },
        {
          name: t('nav.inventory'),
          href: '/inventory',
          icon: ClipboardList,
          show: access.itemsWrite && isFeatureEnabled(features, 'inventory'),
        },
        {
          name: t('nav.sectionAcquisitions'),
          href: '/acquisitions',
          icon: ShoppingCart,
          show: showAcquisitions,
        },
        {
          name: t('nav.serials'),
          href: '/serials',
          icon: Newspaper,
          show: showSerials,
        },
        { name: t('nav.weeding'), href: '/weeding', icon: Leaf, show: showWeeding },
        {
          name: t('nav.z3950Search'),
          href: '/z3950',
          icon: Globe,
          show: access.itemsWrite && isFeatureEnabled(features, 'z3950'),
        },
        {
          name: t('nav.deposits'),
          href: '/deposits',
          icon: PackageOpen,
          show: access.internalCatalog && access.itemsRead && isFeatureEnabled(features, 'deposits'),
        },
      ],
    },
    {
      id: 'patron',
      items: [
        { name: t('nav.myLoans'), href: '/my-loans', icon: BookMarked, show: true },
        {
          name: t('nav.myHolds'),
          href: '/my-holds',
          icon: BookmarkCheck,
          show: canPatronSelfServiceHolds(user, api.getToken()),
        },
        {
          name: t('nav.events'),
          href: '/events',
          icon: CalendarDays,
          show: isFeatureEnabled(features, 'events'),
        },
        {
          name: t('nav.suggestions'),
          href: '/my-suggestions',
          icon: Lightbulb,
          show: isFeatureEnabled(features, 'purchaseSuggestions'),
        },
      ],
    },
    {
      id: 'admin',
      collapseLabel: t('nav.sectionAdmin'),
      items: [
        { name: t('nav.stats'), href: '/stats', icon: BarChart3, show: access.stats },
        {
          name: t('nav.library'),
          href: '/settings?tab=library',
          icon: LibraryBig,
          show: access.librarySettings && !access.fullSettings,
        },
        { name: t('nav.settings'), href: '/settings', icon: Settings, show: access.fullSettings },
      ],
    },
  ]
    .map((section) => ({ ...section, items: section.items.filter((item) => item.show) }))
    .filter((section) => section.items.length > 0);

  const hasStaffNav = navSections.some((section) => staffNavSectionIds.has(section.id));
  const navSectionsWithPatronHeader = navSections.map((section) => {
    if (section.id !== 'patron') return section;
    if (!hasStaffNav) {
      return { ...section, showHeader: false };
    }
    return {
      ...section,
      label: t('nav.sectionMySpace'),
      collapseLabel: t('nav.sectionMySpace'),
    };
  });

  const navSectionsSyncKey = navSections
    .map((section) => `${section.id}:${section.items.map((item) => item.href).join(',')}`)
    .join('|');

  const navScrollRef = useRef<HTMLElement>(null);
  const [navScrollable, setNavScrollable] = useState(false);
  const [navScrollFade, setNavScrollFade] = useState({ top: false, bottom: false });

  const remeasureNavScroll = useCallback(() => {
    const nav = navScrollRef.current;
    if (!nav) return;

    const edgeSlack = 4;
    const scrollable = nav.scrollHeight > nav.clientHeight + edgeSlack;
    const top = scrollable && nav.scrollTop > edgeSlack;
    const bottom =
      scrollable && nav.scrollTop + nav.clientHeight < nav.scrollHeight - edgeSlack;

    setNavScrollable((prev) => (prev === scrollable ? prev : scrollable));
    setNavScrollFade((prev) => (prev.top === top && prev.bottom === bottom ? prev : { top, bottom }));
  }, []);

  useLayoutEffect(() => {
    const nav = navScrollRef.current;
    if (!nav) return;

    const measure = () => remeasureNavScroll();

    measure();
    let raf = 0;
    const scheduleMeasure = () => {
      if (raf) cancelAnimationFrame(raf);
      raf = requestAnimationFrame(measure);
    };

    const resizeObserver = new ResizeObserver(scheduleMeasure);
    resizeObserver.observe(nav);
    nav.addEventListener('scroll', measure, { passive: true });
    window.addEventListener('resize', scheduleMeasure);
    return () => {
      if (raf) cancelAnimationFrame(raf);
      resizeObserver.disconnect();
      nav.removeEventListener('scroll', measure);
      window.removeEventListener('resize', scheduleMeasure);
    };
  }, [navSectionsSyncKey, sidebarOpen, remeasureNavScroll]);

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-950">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-[80] focus:rounded-lg focus:bg-amber-600 focus:px-3 focus:py-2 focus:text-sm focus:font-medium focus:text-white focus:shadow-lg"
      >
        {t('nav.skipToContent')}
      </a>
      {/* Mobile sidebar backdrop */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-40 lg:hidden"
          aria-hidden="true"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar */}
      <aside
        id="main-sidebar"
        ref={sidebarRef}
        className={`print:hidden fixed top-0 left-0 z-50 h-full w-64 bg-white dark:bg-gray-900 border-r border-gray-200 dark:border-gray-800 transform transition-transform duration-300 lg:translate-x-0 ${
          sidebarOpen ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        <div className="flex flex-col h-full">
          {/* Logo / title */}
          <div className="relative flex flex-col items-center justify-center py-4 px-4 border-b border-gray-200 dark:border-gray-800">
            <Link
              to="/home"
              className="flex flex-col items-center gap-1.5 text-center w-full"
              onClick={() => setSidebarOpen(false)}
            >
              <img
                src="/elidune_logo.png"
                alt="Elidune"
                className="h-25 w-25 object-contain shrink-0"
              />
              <span className="text-sm font-semibold text-gray-900 dark:text-white leading-tight">
                {libraryName ?? 'Elidune'}
              </span>
            </Link>
            <button
              type="button"
              onClick={() => setSidebarOpen(false)}
              aria-label={t('common.close')}
              className="lg:hidden absolute right-3 top-3 p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800"
            >
              <X className="h-5 w-5" />
            </button>
          </div>

          {/* Navigation */}
          <div className="relative min-h-0 flex-1">
            {navScrollFade.top ? (
              <div
                className="pointer-events-none absolute inset-x-0 top-0 z-10 h-7 bg-gradient-to-b from-white via-white/80 to-transparent dark:from-gray-900 dark:via-gray-900/80"
                aria-hidden
              />
            ) : null}
            {navScrollFade.bottom ? (
              <div
                className="pointer-events-none absolute inset-x-0 bottom-0 z-10 h-9 bg-gradient-to-t from-white via-white/85 to-transparent dark:from-gray-900 dark:via-gray-900/85"
                aria-hidden
              />
            ) : null}
            <nav
              ref={navScrollRef}
              aria-label={t('nav.main')}
              className="h-full min-h-0 px-3 py-4 space-y-4 overflow-y-auto [scrollbar-gutter:stable]"
            >
              <SidebarNavSections
                sections={navSectionsWithPatronHeader}
                onNavigate={() => setSidebarOpen(false)}
                onLayoutChange={remeasureNavScroll}
              />
            </nav>
          </div>

          {access.backgroundTasks && (
            <div
              className={[
                'shrink-0 px-3 pb-2',
                navScrollable ? 'border-t border-gray-200 pt-2 dark:border-gray-800' : '',
              ]
                .filter(Boolean)
                .join(' ')}
            >
              <BackgroundTasksNavItem />
            </div>
          )}

          {/* User info */}
          <div className="p-4 border-t border-gray-200 dark:border-gray-800">
            <div className="flex items-center gap-3">
              <Link
                to="/profile"
                onClick={() => setSidebarOpen(false)}
                className="flex items-center gap-3 flex-1 min-w-0 p-2 -m-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                title={t('nav.profile')}
              >
                <div className="flex-shrink-0 h-10 w-10 rounded-full bg-amber-100 dark:bg-amber-900/50 flex items-center justify-center">
                  <span className="text-sm font-medium text-amber-700 dark:text-amber-400">
                    {user?.firstname?.[0] || user?.username?.[0] || '?'}
                  </span>
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
                    {user?.firstname} {user?.lastname}
                  </p>
                  <p className="text-xs text-gray-500 dark:text-gray-400 truncate">
                    {localizedAccountTypeLabel(t, user?.accountType, accountTypes)}
                  </p>
                </div>
              </Link>
              <button
                type="button"
                onClick={handleLogout}
                aria-label={t('nav.logout')}
                className="p-2 rounded-lg text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-gray-700 dark:hover:text-gray-300"
                title={t('nav.logout')}
              >
                <LogOut className="h-5 w-5" />
              </button>
            </div>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <div className="flex h-svh min-h-0 flex-col lg:pl-64 print:h-auto print:pl-0">
        {/* Mobile header */}
        <header className="print:hidden sticky top-0 z-30 flex items-center h-16 px-4 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 lg:hidden">
          <button
            type="button"
            onClick={() => setSidebarOpen(true)}
            aria-controls="main-sidebar"
            aria-expanded={sidebarOpen}
            aria-label={t('nav.openMenu')}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800"
          >
            <Menu className="h-6 w-6" />
          </button>
          <Link
            to="/home"
            className="ml-2 shrink-0 rounded-lg p-1 hover:bg-gray-100 dark:hover:bg-gray-800"
            aria-label={libraryName ?? 'Elidune'}
          >
            <img src="/elidune_logo.png" alt="" className="h-8 w-8 object-contain" aria-hidden />
          </Link>
          <span className="min-w-0 flex-1" aria-hidden />
        </header>

        <div className="print:hidden">
          <BackgroundTasksDrawer />
        </div>

        {/*
          Mobile: one scrollport below the sticky header (main + footer).
          Desktop (`lg+`): main scrolls; footer stays in the shell.
        */}
        <div
          id="app-page-scroll"
          className={`flex min-h-0 flex-1 flex-col overflow-y-auto lg:overflow-hidden ${APP_LIST_SCROLL_ROOT_CLASS} max-lg:overscroll-y-contain`}
        >
          <div className="flex min-h-0 flex-1 flex-col">
            <main
              id="main-content"
              tabIndex={-1}
              className="flex min-h-0 flex-1 flex-col overflow-hidden outline-none"
            >
              {/*
                Padding on the scroll wrapper (not the scrollport) so sticky headers sit at top:0.
                lg: page body scrolls here unless a list page uses ListPageLayout + ScrollableListRegion fill.
              */}
              <div className="flex min-h-0 flex-1 flex-col max-lg:overflow-visible overflow-y-auto px-4 pb-4 pt-2 lg:overscroll-y-contain lg:p-6">
                {profileWarning ? (
                  <div
                    role="status"
                    className="mb-4 flex flex-col gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900 sm:flex-row sm:items-center dark:border-amber-900/50 dark:bg-amber-950/30 dark:text-amber-100"
                  >
                    <p className="min-w-0 flex-1">
                      {profileWarning.seconds != null
                        ? rateLimitWaitMessage(profileWarning.seconds, t, i18n.language)
                        : t(profileWarning.messageKey)}
                    </p>
                    <div className="flex shrink-0 gap-2">
                      <Button type="button" size="sm" variant="secondary" onClick={() => void refreshProfile()}>
                        {t('common.retry')}
                      </Button>
                      <Button type="button" size="sm" variant="ghost" onClick={dismissProfileWarning}>
                        {t('common.close')}
                      </Button>
                    </div>
                  </div>
                ) : null}
                {children}
              </div>
            </main>

            {/* Footer — pinned to bottom when content is short; follows content when tall */}
            <footer className="print:hidden mt-auto shrink-0 border-t border-gray-200 dark:border-gray-800 py-3 px-4 lg:px-6 pb-[max(0.75rem,env(safe-area-inset-bottom))] text-xs text-gray-400 dark:text-gray-500">
              <div className="grid grid-cols-1 items-center gap-y-2 sm:grid-cols-[1fr_auto_1fr] sm:gap-x-4 sm:gap-y-0">
                <div className="flex flex-wrap items-center justify-center gap-x-4 sm:justify-start">
                  <span className="font-mono">UI v{uiVersion}</span>
                  <span className="font-mono">Server v{serverVersion ?? '—'}</span>
                </div>
                <div className="text-center whitespace-nowrap">
                  {t('common.poweredBy')}{' '}
                  <LegalModalLink
                    modal="about"
                    className="font-semibold text-amber-600 dark:text-amber-400 hover:underline transition-colors"
                  >
                    Elidune
                  </LegalModalLink>
                </div>
                <div className="flex flex-wrap items-center justify-center gap-x-4 sm:justify-end">
                  <LegalModalLink
                    modal="about"
                    className="hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                  >
                    {t('nav.about')}
                  </LegalModalLink>
                  <LegalModalLink
                    modal="privacy"
                    className="hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                  >
                    {t('nav.privacy')}
                  </LegalModalLink>
                </div>
              </div>
            </footer>
          </div>
        </div>
      </div>
    </div>
  );
}

export default function Layout({ children }: LayoutProps) {
  return (
    <MobilePageTitleProvider>
      <LayoutShell>{children}</LayoutShell>
    </MobilePageTitleProvider>
  );
}

