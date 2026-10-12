import { useTranslation } from 'react-i18next';
import { PUBLIC_LANGUAGE_OPTIONS, SUPPORTED_LANGUAGES, type SupportedLanguage } from '@/locales';
import { useAuth } from '@/contexts/AuthContext';
import { useLanguage } from '@/contexts/LanguageContext';
import api from '@/services/api';

function isSupportedLanguage(value: string): value is SupportedLanguage {
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(value);
}

/**
 * Compact language control for anonymous visitors.
 * Hidden once an account session is active — the account preference owns the UI language.
 * Options use native names (Français, English, Deutsch, Español), with no flags.
 */
export default function PublicLanguageSelector() {
  const { t } = useTranslation();
  const { isAuthenticated, isLoading } = useAuth();
  const { language, setLanguage, languageNames } = useLanguage();

  if (isLoading || isAuthenticated || api.isAuthenticated()) return null;

  return (
    // In normal flow, above the page, so it never sits on the header card corner
    // or the decorative image. This keeps the narrow layout used at 420px.
    <div className="z-40 flex justify-end px-3 pt-3">
      <label htmlFor="public-language" className="sr-only">
        {t('language.selector')}
      </label>
      <select
        id="public-language"
        value={language}
        onChange={(event) => {
          const next = event.target.value;
          if (isSupportedLanguage(next)) void setLanguage(next);
        }}
        className="h-8 min-h-8 rounded-lg border border-gray-300 bg-white px-2 text-xs font-medium text-gray-900 shadow-sm focus:outline-none focus-visible:border-amber-500 focus-visible:ring-2 focus-visible:ring-amber-500/30 dark:border-gray-600 dark:bg-gray-900 dark:text-gray-100"
      >
        {PUBLIC_LANGUAGE_OPTIONS.map((code) => (
          <option key={code} value={code}>
            {languageNames[code]}
          </option>
        ))}
      </select>
    </div>
  );
}
