import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useLibrary } from '@/contexts/LibraryContext';
import LanguageSwitcher from './LanguageSwitcher';

interface PublicPageChromeProps {
  children: React.ReactNode;
}

/** Minimal chrome for unauthenticated shareable pages (events, etc.). */
export default function PublicPageChrome({ children }: PublicPageChromeProps) {
  const { t } = useTranslation();
  const { libraryName } = useLibrary();

  return (
    <div className="min-h-screen flex flex-col bg-gray-50 dark:bg-gray-950">
      <header className="shrink-0 border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900">
        <div className="mx-auto flex w-full max-w-5xl items-center gap-3 px-4 py-3">
          <Link to="/" className="flex min-w-0 items-center gap-2">
            <img src="/elidune_logo.png" alt="Elidune" className="h-9 w-9 shrink-0 object-contain" />
            <span className="truncate text-sm font-semibold text-gray-900 dark:text-white">
              {libraryName ?? 'Elidune'}
            </span>
          </Link>
          <div className="ml-auto flex items-center gap-3">
            <LanguageSwitcher id="public-language-switcher" className="w-36" />
            <Link
              to="/"
              className="text-sm font-medium text-amber-600 dark:text-amber-400 hover:underline whitespace-nowrap"
            >
              {t('auth.loginButton')}
            </Link>
          </div>
        </div>
      </header>
      <main className="flex min-h-0 flex-1 flex-col p-4 lg:p-6">{children}</main>
      <footer className="shrink-0 border-t border-gray-200 dark:border-gray-800 py-3 px-4 text-xs text-gray-400 dark:text-gray-500">
        <div className="mx-auto flex max-w-5xl flex-wrap items-center gap-x-4 gap-y-1">
          <span className="flex-1" />
          <Link to="/about" className="hover:text-gray-600 dark:hover:text-gray-300">
            {t('nav.about')}
          </Link>
          <Link to="/privacy" className="hover:text-gray-600 dark:hover:text-gray-300">
            {t('nav.privacy')}
          </Link>
        </div>
      </footer>
    </div>
  );
}
