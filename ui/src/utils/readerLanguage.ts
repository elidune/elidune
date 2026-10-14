import {
  supportedLanguageCode,
  type SupportedLanguage,
} from '../locales/languages.ts';
import { preferenceToUiLanguage } from './uiLanguage.ts';

/** Select value for “use the library language”. The write body omits `language`. */
export const READER_LANGUAGE_LIBRARY = '';

/**
 * Select value for a stored reader language.
 * UI codes (`de`) and server names (`german`) map onto the supported set.
 * Missing, null, and unsupported values stay on the library-language option.
 */
export function readerLanguageSelectValue(stored: string | null | undefined): string {
  return preferenceToUiLanguage(stored) ?? READER_LANGUAGE_LIBRARY;
}

/**
 * `language` for POST /users and PUT /users/{id}.
 * An explicit supported code (`fr`, `en`, `de`, `es`) is sent.
 * The library-language option adds no field, so a new reader stays NULL
 * and emails follow the library language.
 */
export function appendReaderLanguage<T extends Record<string, unknown>>(
  body: T,
  choice: string | null | undefined,
): T & { language?: SupportedLanguage } {
  const language = supportedLanguageCode(choice);
  if (!language) return body;
  return { ...body, language };
}
