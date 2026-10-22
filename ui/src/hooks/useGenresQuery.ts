import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import api from '@/services/api';
import type { GenreCatalogEntry } from '@/types';
import { GENRE_OPTIONS } from '@/utils/codeLabels';

export const GENRES_QUERY_KEY = ['genres'] as const;

export function useGenresQuery() {
  return useQuery<GenreCatalogEntry[]>({
    queryKey: GENRES_QUERY_KEY,
    queryFn: () => api.getGenres(),
    staleTime: 30 * 60 * 1000,
  });
}

/** Genre camelCase → shelf code map for call-number suggestions. */
export function useGenreShelfCodes(): Record<string, string> {
  const { data } = useGenresQuery();
  return useMemo(() => {
    const map: Record<string, string> = {};
    for (const row of data ?? []) {
      if (row.shelfCode) map[row.code] = row.shelfCode;
    }
    return map;
  }, [data]);
}

export type GenreSelectOption = { value: string; label: string };

/** Options for genre selects: API labels for UI language, i18n fallback. */
export function useGenreSelectOptions(): GenreSelectOption[] {
  const { t, i18n } = useTranslation();
  const { data } = useGenresQuery();
  const lang = (i18n.language || 'en').slice(0, 2);

  return useMemo(() => {
    if (data?.length) {
      return data
        .filter((row) => row.code !== 'unknown')
        .map((row) => ({
          value: row.code,
          label:
            row.labels[lang] ||
            row.labels.en ||
            Object.values(row.labels)[0] ||
            row.code,
        }));
    }
    return GENRE_OPTIONS.filter((o) => o.value !== 'unknown').map((o) => ({
      value: o.value,
      label: t(o.labelKey),
    }));
  }, [data, lang, t]);
}

/** Resolve a genre code to a display label (API, else i18n). */
export function useGenreLabel(): (code: string | null | undefined) => string {
  const { t, i18n } = useTranslation();
  const { data } = useGenresQuery();
  const lang = (i18n.language || 'en').slice(0, 2);

  return useMemo(() => {
    const byCode = new Map((data ?? []).map((row) => [row.code, row]));
    return (code: string | null | undefined) => {
      if (!code) return '';
      const row = byCode.get(code);
      if (row) {
        return (
          row.labels[lang] ||
          row.labels.en ||
          Object.values(row.labels)[0] ||
          code
        );
      }
      const fallback = GENRE_OPTIONS.find((o) => o.value === code);
      return fallback ? t(fallback.labelKey) : code;
    };
  }, [data, lang, t]);
}
