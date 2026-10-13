/**
 * Bibliographic language as stored by the API (`french`, legacy codes, ISO tags)
 * shown in the active UI locale.
 */

const UNSPECIFIED = new Set(['unknown', 'langunknown', '0']);

/** English names and legacy aliases the API still emits, mapped to a BCP 47 tag. */
const LANGUAGE_ALIASES: Record<string, string> = {
  french: 'fr',
  english: 'en',
  german: 'de',
  spanish: 'es',
  italian: 'it',
  portuguese: 'pt',
  japanese: 'ja',
  chinese: 'zh',
  russian: 'ru',
  arabic: 'ar',
  dutch: 'nl',
  swedish: 'sv',
  norwegian: 'no',
  danish: 'da',
  finnish: 'fi',
  polish: 'pl',
  czech: 'cs',
  hungarian: 'hu',
  romanian: 'ro',
  turkish: 'tr',
  korean: 'ko',
  latin: 'la',
  greek: 'el',
  croatian: 'hr',
  hindi: 'hi',
  hebrew: 'he',
  persian: 'fa',
  catalan: 'ca',
  thai: 'th',
  vietnamese: 'vi',
  indonesian: 'id',
  malay: 'ms',
  // Legacy numeric ids and camelCase strings (server `Language::from`).
  '1': 'fr',
  '2': 'en',
  '3': 'de',
  '4': 'ja',
  '5': 'es',
  '6': 'pt',
  langfr: 'fr',
  langen: 'en',
  langde: 'de',
  langjp: 'ja',
  langes: 'es',
  langpo: 'pt',
  // Short codes that are not valid ISO 639 tags.
  jp: 'ja',
  po: 'pt',
};

function foldLanguageKey(value: string): string {
  return value.toLowerCase().replace(/[\s_]/g, '');
}

function capitalizeLabel(label: string, locale: string): string {
  const first = label.charAt(0);
  const upper = first.toLocaleUpperCase(locale);
  if (first === upper) return label;
  return upper + label.slice(1);
}

function localizedLanguageName(tag: string, uiLanguage: string): string | null {
  const locale = uiLanguage.trim() || 'fr';
  try {
    const label = new Intl.DisplayNames([locale], { type: 'language' }).of(tag);
    if (!label || label.toLowerCase() === tag.toLowerCase()) return null;
    return capitalizeLabel(label, locale);
  } catch {
    return null;
  }
}

/**
 * Localized language name. Unknown values are returned unchanged.
 * `unspecifiedLabel` is used for the API sentinel `unknown`.
 */
export function formatCatalogLanguage(
  value: string | number | null | undefined,
  uiLanguage: string,
  unspecifiedLabel?: string,
): string {
  if (value == null) return '';
  const raw = String(value).trim();
  if (!raw) return '';

  const folded = foldLanguageKey(raw);
  if (UNSPECIFIED.has(folded)) {
    const label = unspecifiedLabel?.trim();
    return label || raw;
  }

  const alias = LANGUAGE_ALIASES[folded];
  if (alias) {
    return localizedLanguageName(alias, uiLanguage) ?? raw;
  }

  return localizedLanguageName(raw, uiLanguage) ?? raw;
}
