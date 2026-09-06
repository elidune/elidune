import { useTranslation } from 'react-i18next';
import { useLanguage } from '@/contexts/LanguageContext';
import type { SupportedLanguage } from '@/locales';

interface LanguageSwitcherProps {
  /** Compact select for sidebar / public chrome. */
  className?: string;
  id?: string;
}

export default function LanguageSwitcher({ className = '', id }: LanguageSwitcherProps) {
  const { t } = useTranslation();
  const { language, setLanguage, availableLanguages, languageNames } = useLanguage();

  return (
    <div className={className}>
      <label htmlFor={id ?? 'language-switcher'} className="sr-only">
        {t('nav.language')}
      </label>
      <select
        id={id ?? 'language-switcher'}
        value={language}
        onChange={(e) => {
          void setLanguage(e.target.value as SupportedLanguage);
        }}
        className="w-full rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 px-2.5 py-1.5 text-xs font-medium text-gray-800 dark:text-gray-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500"
      >
        {availableLanguages.map((lang) => (
          <option key={lang} value={lang}>
            {languageNames[lang]}
          </option>
        ))}
      </select>
    </div>
  );
}
