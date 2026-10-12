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

/** Keep `<html lang>` aligned with the active UI language. */
export function syncHtmlLang(
  language: string | null | undefined,
  root?: { lang: string } | null,
): void {
  const lang = supportedLanguageCode(language) ?? UI_LANGUAGE_FALLBACK;
  const target = root === undefined
    ? (typeof document !== 'undefined' ? document.documentElement : null)
    : root;
  if (target) target.lang = lang;
}

/** Empty select value for “not set”. The update body sends JSON `null`. */
export const LIBRARY_LANGUAGE_UNSET = '';

/**
 * Select value for the library default language.
 * Missing, null, blank, and unsupported codes stay unset.
 */
export function libraryLanguageChoice(value: string | null | undefined): string {
  return supportedLanguageCode(value) ?? LIBRARY_LANGUAGE_UNSET;
}

/**
 * PUT body for the library default language.
 * A supported UI code is sent as-is. “Not set” is `null`, which clears the stored value.
 */
export function libraryLanguageForUpdate(choice: string | null | undefined): string | null {
  return supportedLanguageCode(choice);
}

/** True when a 400 `validation_error` is about the library `language` field. */
export function isLibraryLanguageValidationError(error: {
  status?: number | null;
  code?: string | null;
  message?: string | null;
}): boolean {
  return error.status === 400
    && error.code === 'validation_error'
    && /\blanguage\b/i.test(error.message ?? '');
}

/**
 * Switch translations immediately. The public selector must not wait for a reload.
 */
export async function applyUiLanguage(
  i18n: { changeLanguage: (language: string) => Promise<unknown> },
  language: SupportedLanguage,
  root?: { lang: string } | null,
): Promise<SupportedLanguage> {
  await i18n.changeLanguage(language);
  syncHtmlLang(language, root);
  return language;
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
