import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { BookOpen, ChevronRight, MapPin } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  Input,
  ListSkeleton,
  Pagination,
  QueryErrorBanner,
} from '@/components/common';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import {
  availabilityBadgeVariant,
  availabilityLabelKey,
  primaryCopyLocation,
  titleAvailabilityStatus,
  uniqueCopySites,
} from '@/utils/opacAvailability';
import type { MediaType } from '@/types';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';

const PAGE_SIZE = 20;

const MEDIA_FILTERS: Array<{ key: MediaType | ''; labelKey: string }> = [
  { key: '', labelKey: 'opac.filterAll' },
  { key: 'printedText', labelKey: 'opac.filterBooks' },
  { key: 'videoDvd', labelKey: 'opac.filterDvd' },
  { key: 'periodic', labelKey: 'opac.filterMagazines' },
  { key: 'comics', labelKey: 'opac.filterComics' },
];

export default function ReaderCatalogPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchInput, setSearchInput] = useState('');
  const [activeSearch, setActiveSearch] = useState('');
  const [mediaType, setMediaType] = useState<MediaType | ''>('');
  const [page, setPage] = useState(1);

  const [prevFilters, setPrevFilters] = useState({ activeSearch, mediaType });
  if (activeSearch !== prevFilters.activeSearch || mediaType !== prevFilters.mediaType) {
    setPrevFilters({ activeSearch, mediaType });
    setPage(1);
  }

  const catalogQuery = useQuery({
    queryKey: ['reader-catalog', activeSearch, mediaType, page],
    queryFn: () =>
      api.getOPACBiblios({
        freesearch: activeSearch || undefined,
        mediaType: mediaType || undefined,
        page,
        perPage: PAGE_SIZE,
      }),
    staleTime: 2 * 60 * 1000,
  });

  const biblios = catalogQuery.data?.items ?? [];
  const totalPages = Math.max(1, catalogQuery.data?.pageCount ?? 1);

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('nav.catalog')}</h1>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('opac.readerCatalogSubtitle')}</p>
      </div>

      <Card>
        <form
          className="flex flex-col gap-3 sm:flex-row"
          onSubmit={(e) => {
            e.preventDefault();
            setActiveSearch(searchInput.trim());
          }}
        >
          <Input
            type="text"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder={t('opac.searchPlaceholder')}
            className="flex-1"
          />
          <Button type="submit" variant="primary" className="shrink-0">
            {t('common.search')}
          </Button>
        </form>
        <div className="mt-3 flex flex-wrap gap-1.5">
          {MEDIA_FILTERS.map((f) => (
            <button
              key={f.key || 'all'}
              type="button"
              onClick={() => setMediaType(f.key)}
              className={`px-3 py-1.5 rounded-lg text-sm font-medium transition-colors ${
                mediaType === f.key
                  ? 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              {t(f.labelKey)}
            </button>
          ))}
        </div>
      </Card>

      {catalogQuery.isError && (
        <QueryErrorBanner
          message={getApiErrorMessage(catalogQuery.error, t)}
          onRetry={() => void catalogQuery.refetch()}
        />
      )}

      <Card padding="none">
        {catalogQuery.isLoading && !biblios.length ? (
          <ListSkeleton rows={8} />
        ) : biblios.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 px-4 text-gray-500 dark:text-gray-400">
            <BookOpen className="h-12 w-12 mb-3 opacity-30" aria-hidden />
            <p>{t('common.noResults')}</p>
          </div>
        ) : (
          <ul className="divide-y divide-gray-100 dark:divide-gray-800">
            {biblios.map((biblio) => {
              const status = titleAvailabilityStatus(biblio.items);
              const { callNumber } = primaryCopyLocation(biblio.items);
              const sites = uniqueCopySites(biblio.items);
              const authorName = biblio.author
                ? [biblio.author.firstname, biblio.author.lastname].filter(Boolean).join(' ')
                : null;

              return (
                <li key={biblio.id}>
                  <button
                    type="button"
                    onClick={() => navigate(`/biblios/${biblio.id}`)}
                    className="flex w-full items-center gap-3 p-4 text-left hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
                  >
                    <LoanMediaTypeBadge mediaType={biblio.mediaType} size="catalog" />
                    <div className="min-w-0 flex-1 space-y-1">
                      <p className="font-medium text-gray-900 dark:text-white truncate">
                        {biblio.title ?? t('items.notSpecified')}
                      </p>
                      {authorName && (
                        <p className="text-sm text-gray-500 dark:text-gray-400 truncate">{authorName}</p>
                      )}
                      <div className="flex flex-wrap items-center gap-2 pt-0.5">
                        <Badge variant={availabilityBadgeVariant(status)} size="sm">
                          {t(availabilityLabelKey(status))}
                        </Badge>
                        {callNumber && (
                          <span className="font-mono text-xs text-gray-700 dark:text-gray-300">
                            {t('items.callNumber')}: {callNumber}
                          </span>
                        )}
                        {sites.length > 0 && (
                          <span className="inline-flex items-center gap-1 text-xs text-gray-600 dark:text-gray-400">
                            <MapPin className="h-3 w-3 shrink-0" aria-hidden />
                            {sites.join(' · ')}
                          </span>
                        )}
                      </div>
                    </div>
                    <ChevronRight className="h-5 w-5 shrink-0 text-gray-400" aria-hidden />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
        {catalogQuery.data != null && catalogQuery.data.total > 0 && (
          <div className="border-t border-gray-200 dark:border-gray-800 p-4">
            <Pagination currentPage={page} totalPages={totalPages} onPageChange={setPage} />
          </div>
        )}
      </Card>
    </div>
  );
}
