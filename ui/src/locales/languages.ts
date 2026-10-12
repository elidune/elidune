export const SUPPORTED_LANGUAGES = ['en', 'fr', 'de', 'es'] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

/** Anonymous visitors' explicit choice. Separate from the signed-in i18next cache. */
export const PUBLIC_LANGUAGE_STORAGE_KEY = 'elidune_public_language';

export const I18N_STORAGE_KEY = 'i18nextLng';

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
export type ServerLanguage = (typeof SERVER_LANGUAGES)[number];

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
  const normalized = value.trim().toLowerCase() as ServerLanguage;
  return SERVER_TO_SUPPORTED_LANGUAGE[normalized] ?? null;
}

export function toServerLanguage(value: SupportedLanguage): string {
  return SUPPORTED_TO_SERVER_LANGUAGE[value];
}

/** UI code when `value` is a supported language tag (`fr`, `fr-FR`, `en_US`); otherwise null. */
export function supportedLanguageCode(value: string | null | undefined): SupportedLanguage | null {
  if (!value) return null;
  const normalized = value.trim().toLowerCase().split(/[-_]/)[0];
  return (SUPPORTED_LANGUAGES as readonly string[]).includes(normalized)
    ? (normalized as SupportedLanguage)
    : null;
}

/**
 * Active i18n language as a supported UI code.
 * Unsupported or missing values use French — never English — as the product default.
 */
export function fromI18nLanguage(value: string | null | undefined): SupportedLanguage {
  return supportedLanguageCode(value) ?? 'fr';
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

/** Public selector order: French first, then the other UI languages. Native names come from LANGUAGE_NAMES. */
export const PUBLIC_LANGUAGE_OPTIONS: readonly SupportedLanguage[] = ['fr', 'en', 'de', 'es'];
