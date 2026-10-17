import { useTranslation } from 'react-i18next';
import PatronSuggestionsSection from '@/components/suggestions/PatronSuggestionsSection';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';

export default function MyPurchaseSuggestionsPage() {
  const { t } = useTranslation();

  return (
    <div className={APP_PAGE_CLASS}>
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
          {t('acquisitions.suggestions.patronTitle')}
        </h1>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          {t('acquisitions.suggestions.patronSubtitle')}
        </p>
      </div>
      <PatronSuggestionsSection />
    </div>
  );
}
