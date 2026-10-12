import {
  fromServerLanguage,
  supportedLanguageCode,
  type SupportedLanguage,
} from '../locales/languages.ts';

/** Ultimate UI language when nothing else in the resolution chain is supported. */
export const UI_LANGUAGE_FALLBACK: SupportedLanguage = 'fr';

/**
 * Fields that may carry the library UI language on a public settings payload.
 *
 * `GET /api/v1/library-info` is the public library endpoint. Its body is name,
 * address, phones, email, timezone, and updatedAt — no language, locale, or
 * default-locale field. The runtime `settings` table (email, logging, reminders,
 * audit, holds, circulation) is not public and has no UI locale either.
 * When none of these keys is present, callers fall back to French.
 */
const LIBRARY_LANGUAGE_FIELDS = [
  'language',
  'defaultLocale',
  'defaultLanguage',
  'locale',
  'uiLanguage',
] as const;

export interface ResolveUiLanguageInput {
  /** Account preference (`french` or `fr`). Wins when it maps to a UI language. */
  accountLanguage?: string | null;
  /** Explicit anonymous choice stored by the public selector. */
  explicitChoice?: string | null;
  /** `navigator.language` (for example `fr-FR` or `it`). */
  browserLanguage?: string | null;
  /** Library language from public settings, when the payload includes one. */
  libraryLanguage?: string | null;
}

/**
 * Map a server language name (`french`) or a UI code (`fr`) to a supported UI language.
 * Unsupported values (`italian`, `it`) are null so resolution can continue.
 */
export function preferenceToUiLanguage(value: string | null | undefined): SupportedLanguage | null {
  if (!value?.trim()) return null;
  return supportedLanguageCode(value) ?? fromServerLanguage(value);
}

/**
 * Read the library UI language from a public settings object.
 * Returns null when the payload has no configured language (the current API).
 */
export function libraryLanguageFromSettings(source: object | null | undefined): string | null {
  if (!source) return null;
  const record = source as Record<string, unknown>;
  for (const key of LIBRARY_LANGUAGE_FIELDS) {
    const value = record[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return null;
}

/**
 * Resolution order:
 * account preference > explicit public choice > supported browser language > library language > fr.
 * An unsupported browser language such as `it` does not become English.
 */
export function resolveUiLanguage(input: ResolveUiLanguageInput): SupportedLanguage {
  const account = preferenceToUiLanguage(input.accountLanguage);
  if (account) return account;

  const explicit = preferenceToUiLanguage(input.explicitChoice);
  if (explicit) return explicit;

  const browser = supportedLanguageCode(input.browserLanguage);
  if (browser) return browser;

  const library = preferenceToUiLanguage(input.libraryLanguage);
  if (library) return library;

  return UI_LANGUAGE_FALLBACK;
}
