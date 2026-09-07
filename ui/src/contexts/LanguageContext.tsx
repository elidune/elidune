import { createContext, useContext, useEffect, useCallback, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SUPPORTED_LANGUAGES,
  LANGUAGE_NAMES,
  LANGUAGE_FLAGS,
  I18N_STORAGE_KEY,
  fromI18nLanguage,
  fromServerLanguage,
  toServerLanguage,
  type SupportedLanguage,
} from '@/locales';
import { useAuth } from './AuthContext';
import api from '@/services/api';

interface LanguageContextType {
  language: SupportedLanguage;
  setLanguage: (lang: SupportedLanguage) => Promise<void>;
  availableLanguages: typeof SUPPORTED_LANGUAGES;
  languageNames: typeof LANGUAGE_NAMES;
  languageFlags: typeof LANGUAGE_FLAGS;
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

function persistLanguage(lang: SupportedLanguage) {
  try {
    localStorage.setItem(I18N_STORAGE_KEY, lang);
  } catch {
    // Ignore quota / private-mode failures.
  }
}

function clearPersistedLanguage() {
  try {
    localStorage.removeItem(I18N_STORAGE_KEY);
  } catch {
    // Ignore storage failures.
  }
}

function browserLanguage(): SupportedLanguage {
  return fromI18nLanguage(typeof navigator !== 'undefined' ? navigator.language : undefined);
}

export function LanguageProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation();
  const { user, isAuthenticated, isLoading, refreshProfile } = useAuth();
  const language = fromI18nLanguage(i18n.resolvedLanguage ?? i18n.language);

  useEffect(() => {
    if (isLoading) return;

    if (isAuthenticated && user?.language) {
      const userLang = fromServerLanguage(user.language);
      if (!userLang) return;
      const currentLang = fromI18nLanguage(i18n.resolvedLanguage ?? i18n.language);
      if (SUPPORTED_LANGUAGES.includes(userLang) && userLang !== currentLang) {
        void i18n.changeLanguage(userLang);
      }
      persistLanguage(userLang);
      return;
    }

    if (!isAuthenticated) {
      clearPersistedLanguage();
      const nextLang = browserLanguage();
      const currentLang = fromI18nLanguage(i18n.resolvedLanguage ?? i18n.language);
      if (nextLang !== currentLang) {
        void i18n.changeLanguage(nextLang);
      }
    }
  }, [isLoading, isAuthenticated, user?.language, i18n]);

  const setLanguage = useCallback(async (lang: SupportedLanguage) => {
    if (!SUPPORTED_LANGUAGES.includes(lang)) return;

    await i18n.changeLanguage(lang);
    persistLanguage(lang);

    if (isAuthenticated) {
      try {
        await api.updateProfile({ language: toServerLanguage(lang) });
        await refreshProfile();
      } catch (error) {
        console.error('Failed to save language preference to server:', error);
      }
    }
  }, [i18n, isAuthenticated, refreshProfile]);

  return (
    <LanguageContext.Provider
      value={{
        language,
        setLanguage,
        availableLanguages: SUPPORTED_LANGUAGES,
        languageNames: LANGUAGE_NAMES,
        languageFlags: LANGUAGE_FLAGS,
      }}
    >
      {children}
    </LanguageContext.Provider>
  );
}

export function useLanguage() {
  const context = useContext(LanguageContext);
  if (context === undefined) {
    throw new Error('useLanguage must be used within a LanguageProvider');
  }
  return context;
}
