import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import enTranslation from './en/translation.json';
import frTranslation from './fr/translation.json';
import deTranslation from './de/translation.json';
import esTranslation from './es/translation.json';

export const SUPPORTED_LANGUAGES = ['en', 'fr', 'de', 'es'] as const;
export type SupportedLanguage = typeof SUPPORTED_LANGUAGES[number];

export const SERVER_LANGUAGES = [
  'unknown',
  'french',
  'english',
  'german',
  'spanish',
  'italian',
  'portuguese',
  'japanese',
  'chinese',
  'russian',
  'arabic',
  'dutch',
  'swedish',
  'norwegian',
  'danish',
  'finnish',
  'polish',
  'czech',
  'hungarian',
  'romanian',
  'turkish',
  'korean',
  'latin',
  'greek',
  'croatian',
  'hindi',
  'hebrew',
  'persian',
  'catalan',
  'thai',
  'vietnamese',
  'indonesian',
  'malay',
] as const;
export type ServerLanguage = typeof SERVER_LANGUAGES[number];

const SERVER_TO_SUPPORTED_LANGUAGE: Partial<Record<ServerLanguage, SupportedLanguage>> = {
  english: 'en',
  french: 'fr',
  german: 'de',
  spanish: 'es',
};

const SUPPORTED_TO_SERVER_LANGUAGE: Record<SupportedLanguage, string> = {
  en: 'english',
  fr: 'french',
  de: 'german',
  es: 'spanish',
};

export function fromServerLanguage(value: string | null | undefined): SupportedLanguage | null {
  if (!value) return null;
  const normalized = value.toLowerCase() as ServerLanguage;
  return SERVER_TO_SUPPORTED_LANGUAGE[normalized] ?? null;
}

export function toServerLanguage(value: SupportedLanguage): string {
  return SUPPORTED_TO_SERVER_LANGUAGE[value];
}

export const I18N_STORAGE_KEY = 'i18nextLng';

export function fromI18nLanguage(value: string | null | undefined): SupportedLanguage {
  if (!value) return 'en';
  const normalized = value.toLowerCase().split('-')[0] as SupportedLanguage;
  return SUPPORTED_LANGUAGES.includes(normalized) ? normalized : 'en';
}

/** True when a session token is already in localStorage (i18n init is before AuthContext). */
function hasStoredAuthToken(): boolean {
  try {
    return typeof localStorage !== 'undefined' && !!localStorage.getItem('auth_token');
  } catch {
    return false;
  }
}

const isAuthenticatedAtInit = hasStoredAuthToken();

if (!isAuthenticatedAtInit) {
  try {
    localStorage.removeItem(I18N_STORAGE_KEY);
  } catch {
    // Ignore quota / private-mode failures; detection still uses navigator.
  }
}

export const LANGUAGE_NAMES: Record<SupportedLanguage, string> = {
  en: 'English',
  fr: 'Français',
  de: 'Deutsch',
  es: 'Español',
};

export const LANGUAGE_FLAGS: Record<SupportedLanguage, string> = {
  en: '🇬🇧',
  fr: '🇫🇷',
  de: '🇩🇪',
  es: '🇪🇸',
};

const resources = {
  en: { translation: enTranslation },
  fr: { translation: frTranslation },
  de: { translation: deTranslation },
  es: { translation: esTranslation },
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'en',
    supportedLngs: [...SUPPORTED_LANGUAGES],
    // Map navigator values like `fr-FR` onto supported `fr` (not fallback `en`).
    nonExplicitSupportedLngs: true,
    load: 'languageOnly',

    interpolation: {
      escapeValue: false, // React already escapes
    },

    detection: {
      // Anonymous: browser locale only. Authenticated: last profile language (localStorage)
      // until LanguageContext applies user.language from the server.
      order: isAuthenticatedAtInit
        ? ['localStorage', 'navigator', 'htmlTag']
        : ['navigator', 'htmlTag'],
      lookupLocalStorage: I18N_STORAGE_KEY,
      caches: isAuthenticatedAtInit ? ['localStorage'] : [],
    },

    react: {
      useSuspense: false,
    },
  });

export default i18n;

