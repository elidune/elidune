import { Lightbulb } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { PageHeading } from '@/components/common';
import PatronSuggestionsSection from '@/components/suggestions/PatronSuggestionsSection';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';

export default function MyPurchaseSuggestionsPage() {
  const { t } = useTranslation();

  return (
    <div className={APP_PAGE_CLASS}>
      <PageHeading
        icon={Lightbulb}
        title={t('acquisitions.suggestions.patronTitle')}
        subtitle={t('acquisitions.suggestions.patronSubtitle')}
      />
      <PatronSuggestionsSection />
    </div>
  );
}
