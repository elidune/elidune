import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import enTranslation from './en/translation.json';
import frTranslation from './fr/translation.json';
import deTranslation from './de/translation.json';
import esTranslation from './es/translation.json';
import {
  I18N_STORAGE_KEY,
  PUBLIC_LANGUAGE_STORAGE_KEY,
  SUPPORTED_LANGUAGES,
} from './languages';
import { syncHtmlLang } from '../utils/uiLanguage';

export {
  fromI18nLanguage,
  fromServerLanguage,
  I18N_STORAGE_KEY,
  LANGUAGE_FLAGS,
  LANGUAGE_NAMES,
  PUBLIC_LANGUAGE_OPTIONS,
  PUBLIC_LANGUAGE_STORAGE_KEY,
  SERVER_LANGUAGES,
  SUPPORTED_LANGUAGES,
  supportedLanguageCode,
  toServerLanguage,
} from './languages';
export type { ServerLanguage, SupportedLanguage } from './languages';

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
    // Drop the signed-in language cache. Anonymous choice lives in a separate key.
    localStorage.removeItem(I18N_STORAGE_KEY);
  } catch {
    // Ignore quota / private-mode failures; detection still uses navigator.
  }
}

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
    // Unsupported tags (for example navigator `it`) must not become English.
    fallbackLng: 'fr',
    supportedLngs: [...SUPPORTED_LANGUAGES],
    // Map navigator values like `fr-FR` onto supported `fr` (not the fallback).
    nonExplicitSupportedLngs: true,
    load: 'languageOnly',

    interpolation: {
      escapeValue: false, // React already escapes
    },

    detection: {
      // Signed in: last account language until LanguageContext applies user.language.
      // Anonymous: explicit public choice, then the browser. Do not cache the
      // detected language — only the selector writes the public key.
      order: ['localStorage', 'navigator', 'htmlTag'],
      lookupLocalStorage: isAuthenticatedAtInit ? I18N_STORAGE_KEY : PUBLIC_LANGUAGE_STORAGE_KEY,
      caches: isAuthenticatedAtInit ? ['localStorage'] : [],
    },

    react: {
      useSuspense: false,
    },
  });

syncHtmlLang(i18n.resolvedLanguage ?? i18n.language);
i18n.on('languageChanged', (language) => {
  syncHtmlLang(language);
});

export default i18n;
