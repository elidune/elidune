/**
 * Call-number suggestion helpers for physical copies (items).
 *
 * Priority (default selection):
 * 1. Genre shelf code + author mark → e.g. "ROM BAL" (cote)
 * 2. Dewey + author mark → e.g. "843.914 BAL"
 * 3. Legacy CATEGORY-YEAR-XXX fallback
 *
 * Genre shelf codes come from GET /genres (DB), not a hardcoded map.
 */

export type CallNumberAuthor = {
  lastname?: string | null;
  firstname?: string | null;
  function?: string | null;
};

/** Three-letter author mark from a free-text name (legacy word-split). */
export function authorMarkFromName(name?: string | null): string {
  const namePart = (name || '')
    .trim()
    .replace(/[^a-zA-Z\s]/g, '')
    .split(/\s+/)
    .filter(Boolean)
    .pop();
  return namePart ? namePart.slice(0, 3).toUpperCase() : 'XXX';
}

/**
 * Prefer an author with function `author`, else the first listed author.
 * Uses lastname when present; otherwise falls back to name-word logic.
 */
export function authorMarkFromAuthors(authors?: CallNumberAuthor[] | null): string {
  if (!authors?.length) return 'XXX';
  const preferred =
    authors.find((a) => (a.function || '').toLowerCase() === 'author') ?? authors[0];
  const lastname = preferred.lastname?.trim();
  if (lastname) {
    const letters = lastname.replace(/[^a-zA-Z]/g, '');
    if (letters) return letters.slice(0, 3).toUpperCase();
  }
  const combined = [preferred.lastname, preferred.firstname].filter(Boolean).join(' ');
  return authorMarkFromName(combined);
}

/** Resolve shelf code from a genre camelCase key using the catalog map. */
export function genreShelfCode(
  genre?: string | null,
  shelfCodes?: Record<string, string> | null,
): string | null {
  if (!genre || !shelfCodes) return null;
  const code = shelfCodes[genre];
  return code ?? null;
}

/**
 * Builds a suggested call number: [CATEGORY_CODE]-[YEAR]-[FIRST_3_LETTERS_OF_NAME]
 * Example: Zoology + 2024 + Smith -> ZOO-2024-SMI
 */
export function buildSuggestedCallNumber(options: {
  categoryCode?: string | null;
  year?: number | string | null;
  authorOrCollectorName?: string | null;
}): string {
  const category = (options.categoryCode || 'GEN').toString().toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 6) || 'GEN';
  const year = options.year
    ? typeof options.year === 'number'
      ? options.year
      : parseInt(String(options.year).trim().slice(0, 4), 10)
    : new Date().getFullYear();
  const validYear = Number.isFinite(year) && year >= 1000 && year <= 9999 ? year : new Date().getFullYear();
  const threeLetters = authorMarkFromName(options.authorOrCollectorName);
  return `${category}-${validYear}-${threeLetters}`;
}

export type SuggestedCallNumberBiblioInput = {
  dewey?: string | null;
  genre?: string | null;
  authors?: CallNumberAuthor[] | null;
  mediaType?: string | null;
  publicationDate?: string | null;
  /** Optional override when authors are not structured (Z39.50 free text). */
  authorOrCollectorName?: string | null;
  /** Genre code → shelf prefix from GET /genres. */
  genreShelfCodes?: Record<string, string> | null;
};

export type CallNumberOptionKind = 'dewey' | 'genre' | 'legacy';

export type CallNumberOption = {
  kind: CallNumberOptionKind;
  value: string;
};

/**
 * All constructible call-number formats for a bibliographic record
 * (order: genre/cote, Dewey, legacy). Genre / Dewey omitted when inputs missing;
 * legacy is always present.
 */
export function buildCallNumberOptions(input: SuggestedCallNumberBiblioInput): CallNumberOption[] {
  const mark =
    input.authors && input.authors.length > 0
      ? authorMarkFromAuthors(input.authors)
      : authorMarkFromName(input.authorOrCollectorName);

  const options: CallNumberOption[] = [];

  const shelf = genreShelfCode(input.genre ?? null, input.genreShelfCodes);
  if (shelf) {
    options.push({ kind: 'genre', value: `${shelf} ${mark}` });
  }

  const dewey = input.dewey?.trim();
  if (dewey) {
    options.push({ kind: 'dewey', value: `${dewey} ${mark}` });
  }

  const categoryCode = input.mediaType
    ? String(input.mediaType).toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 4) || 'GEN'
    : 'GEN';
  const year = input.publicationDate?.trim().slice(0, 4);
  options.push({
    kind: 'legacy',
    value: buildSuggestedCallNumber({
      categoryCode,
      year: year ? parseInt(year, 10) : undefined,
      authorOrCollectorName:
        input.authorOrCollectorName ??
        input.authors?.[0]?.lastname ??
        undefined,
    }),
  });

  return options;
}

/**
 * Preferred call number: first option from {@link buildCallNumberOptions}.
 */
export function buildSuggestedCallNumberFromBiblio(input: SuggestedCallNumberBiblioInput): string {
  const options = buildCallNumberOptions(input);
  return options[0]!.value;
}

/** Regex: alphanumeric, spaces, dots and dashes (e.g. E2 571.8 GAS). */
export const CALL_NUMBER_PATTERN = /^[A-Za-z0-9\s.-]*$/;

export function validateCallNumber(value: string): boolean {
  if (value === '') return true;
  return CALL_NUMBER_PATTERN.test(value);
}
