import {
  fromServerLanguage,
  supportedLanguageCode,
  type SupportedLanguage,
} from '../locales/languages.ts';

/** Ultimate UI language when nothing else in the resolution chain is supported. */
export const UI_LANGUAGE_FALLBACK: SupportedLanguage = 'fr';

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
 * Read `language` from `GET /api/v1/library-info`.
 * Accepted values are the UI codes `fr`, `en`, `de`, and `es`.
 * Returns null when the field is missing, null, or blank so the caller uses French.
 */
export function libraryLanguageFromSettings(source: object | null | undefined): string | null {
  if (!source) return null;
  const value = (source as { language?: unknown }).language;
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
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
