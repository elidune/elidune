import { APP_LIST_SCROLL_ROOT_CLASS } from '@/components/common/ScrollableListRegion';
import type { MediaType } from '@/types';

export type CatalogSavedSearch = {
  searchQuery: string;
  mediaType: MediaType | '';
  audienceType: string;
  showFilters: boolean;
  includeWithoutActiveItems: boolean;
  serieId: string;
  serieName: string;
  collectionId: string;
  collectionName: string;
  scrollTop?: number;
};

const SESSION_KEY = 'elidune.catalog.restore';

export function readCatalogScrollTop(anchor: HTMLElement | null): number {
  const root = anchor?.closest(`.${APP_LIST_SCROLL_ROOT_CLASS}`);
  return root instanceof HTMLElement ? root.scrollTop : 0;
}

export function restoreCatalogScrollTop(anchor: HTMLElement | null, scrollTop: number): void {
  const root = anchor?.closest(`.${APP_LIST_SCROLL_ROOT_CLASS}`);
  if (!(root instanceof HTMLElement) || scrollTop <= 0) return;
  requestAnimationFrame(() => {
    root.scrollTop = scrollTop;
  });
}

export function buildCatalogSearchParams(search: CatalogSavedSearch): URLSearchParams {
  const params = new URLSearchParams();
  if (search.searchQuery) params.set('freesearch', search.searchQuery);
  if (search.mediaType) params.set('media_type', search.mediaType);
  if (search.audienceType) params.set('audience_type', search.audienceType);
  if (search.showFilters) params.set('show_filters', '1');
  if (search.includeWithoutActiveItems) params.set('include_without_items', '1');
  if (search.serieId) {
    params.set('serie_id', search.serieId);
    if (search.serieName) params.set('serie_name', search.serieName);
  }
  if (search.collectionId) {
    params.set('collection_id', search.collectionId);
    if (search.collectionName) params.set('collection_name', search.collectionName);
  }
  return params;
}

export function catalogListPath(search: CatalogSavedSearch): string {
  const qs = buildCatalogSearchParams(search).toString();
  return qs ? `/biblios?${qs}` : '/biblios';
}

export function writeCatalogRestoreSession(search: CatalogSavedSearch): void {
  try {
    sessionStorage.setItem(SESSION_KEY, JSON.stringify(search));
  } catch {
    // ignore quota / private mode
  }
}

export function readCatalogRestoreSession(): CatalogSavedSearch | null {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as CatalogSavedSearch;
    if (!parsed || typeof parsed !== 'object') return null;
    return parsed;
  } catch {
    return null;
  }
}

export function clearCatalogRestoreSession(): void {
  try {
    sessionStorage.removeItem(SESSION_KEY);
  } catch {
    // ignore
  }
}

export function catalogSavedSearchFromParams(params: URLSearchParams): CatalogSavedSearch {
  return {
    searchQuery: params.get('freesearch') ?? '',
    mediaType: (params.get('media_type') ?? '') as MediaType | '',
    audienceType: params.get('audience_type') ?? '',
    showFilters: params.get('show_filters') === '1',
    includeWithoutActiveItems: params.get('include_without_items') === '1',
    serieId: params.get('serie_id') ?? '',
    serieName: params.get('serie_name') ?? '',
    collectionId: params.get('collection_id') ?? '',
    collectionName: params.get('collection_name') ?? '',
  };
}
