import { createContext, useContext, useEffect, useCallback, useRef, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  SUPPORTED_LANGUAGES,
  LANGUAGE_NAMES,
  LANGUAGE_FLAGS,
  I18N_STORAGE_KEY,
  PUBLIC_LANGUAGE_STORAGE_KEY,
  fromI18nLanguage,
  toServerLanguage,
  type SupportedLanguage,
} from '@/locales';
import {
  applyUiLanguage,
  libraryLanguageFromSettings,
  preferenceToUiLanguage,
  resolveUiLanguage,
} from '@/utils/uiLanguage';
import { useAuth } from './AuthContext';
import { useLibrary } from './LibraryContext';
import api from '@/services/api';

interface LanguageContextType {
  language: SupportedLanguage;
  setLanguage: (lang: SupportedLanguage) => Promise<void>;
  availableLanguages: typeof SUPPORTED_LANGUAGES;
  languageNames: typeof LANGUAGE_NAMES;
  languageFlags: typeof LANGUAGE_FLAGS;
}

const LanguageContext = createContext<LanguageContextType | undefined>(undefined);

function storageOrNull(): Storage | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage : null;
  } catch {
    return null;
  }
}

function persistAccountLanguage(lang: SupportedLanguage) {
  try {
    localStorage.setItem(I18N_STORAGE_KEY, lang);
  } catch {
    // Ignore quota / private-mode failures.
  }
}

function clearAccountLanguageCache() {
  try {
    localStorage.removeItem(I18N_STORAGE_KEY);
  } catch {
    // Ignore storage failures.
  }
}

function readExplicitPublicLanguage(): string | null {
  const storage = storageOrNull();
  if (!storage) return null;
  try {
    const value = storage.getItem(PUBLIC_LANGUAGE_STORAGE_KEY);
    return value?.trim() ? value.trim() : null;
  } catch {
    return null;
  }
}

function persistPublicLanguage(lang: SupportedLanguage) {
  const storage = storageOrNull();
  if (!storage) return;
  try {
    storage.setItem(PUBLIC_LANGUAGE_STORAGE_KEY, lang);
  } catch {
    // Ignore quota / private-mode failures.
  }
}

export function LanguageProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation();
  const { user, isAuthenticated, isLoading, refreshProfile } = useAuth();
  const { libraryInfo } = useLibrary();
  const language = fromI18nLanguage(i18n.resolvedLanguage ?? i18n.language);
  const applyingChoice = useRef(false);

  useEffect(() => {
    if (isLoading || applyingChoice.current) return;

    const next = resolveUiLanguage({
      accountLanguage: isAuthenticated ? user?.language : null,
      explicitChoice: readExplicitPublicLanguage(),
      browserLanguage: typeof navigator !== 'undefined' ? navigator.language : null,
      libraryLanguage: libraryLanguageFromSettings(libraryInfo),
    });

    if (isAuthenticated) {
      const account = preferenceToUiLanguage(user?.language);
      if (account) persistAccountLanguage(account);
    } else {
      clearAccountLanguageCache();
    }

    const current = fromI18nLanguage(i18n.resolvedLanguage ?? i18n.language);
    if (next !== current) {
      void i18n.changeLanguage(next);
    }
  }, [isLoading, isAuthenticated, user?.language, libraryInfo, i18n]);

  const setLanguage = useCallback(async (lang: SupportedLanguage) => {
    if (!SUPPORTED_LANGUAGES.includes(lang)) return;

    const next = resolveUiLanguage({
      accountLanguage: isAuthenticated ? lang : null,
      explicitChoice: lang,
      browserLanguage: typeof navigator !== 'undefined' ? navigator.language : null,
      libraryLanguage: libraryLanguageFromSettings(libraryInfo),
    });

    if (isAuthenticated) persistAccountLanguage(next);
    else persistPublicLanguage(next);

    applyingChoice.current = true;
    try {
      await applyUiLanguage(i18n, next);
    } finally {
      applyingChoice.current = false;
    }

    if (!isAuthenticated) return;

    try {
      await api.updateProfile({ language: toServerLanguage(next) });
      await refreshProfile();
    } catch (error) {
      console.error('Failed to save language preference to server:', error);
    }
  }, [i18n, isAuthenticated, libraryInfo, refreshProfile]);

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
