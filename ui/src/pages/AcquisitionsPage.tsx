import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useSearchParams } from 'react-router-dom';
import AcquisitionsTabBar from '@/components/acquisitions/AcquisitionsTabBar';
import { useLibrary } from '@/contexts/LibraryContext';
import { AcquisitionsFundsPanel } from '@/pages/AcquisitionsFundsPage';
import { AcquisitionsOrdersPanel } from '@/pages/AcquisitionsOrdersPage';
import { AcquisitionsSuggestionsPanel } from '@/pages/AcquisitionsSuggestionsPage';
import { AcquisitionsVendorsPanel } from '@/pages/AcquisitionsVendorsPage';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { isFeatureEnabled } from '@/utils/featureAccess';
import { type AcquisitionsTab, isAcquisitionsTab } from '@/utils/acquisitionsTabs';

export default function AcquisitionsPage() {
  const { t } = useTranslation();
  const { libraryInfo } = useLibrary();
  const showSuggestions = isFeatureEnabled(libraryInfo?.enabledFeatures, 'purchaseSuggestions');
  const [searchParams, setSearchParams] = useSearchParams();
  const tabParam = searchParams.get('tab');
  const requested: AcquisitionsTab = isAcquisitionsTab(tabParam) ? tabParam : 'orders';
  const activeTab: AcquisitionsTab =
    requested === 'suggestions' && !showSuggestions ? 'orders' : requested;

  useEffect(() => {
    if (requested === 'suggestions' && !showSuggestions) {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          next.set('tab', 'orders');
          return next;
        },
        { replace: true },
      );
    }
  }, [requested, showSuggestions, setSearchParams]);

  const selectTab = (tab: AcquisitionsTab) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('tab', tab);
        return next;
      },
      { replace: true },
    );
  };

  return (
    <div className={`${APP_PAGE_CLASS} gap-6`}>
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('nav.sectionCataloging')}</h1>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('acquisitions.pageSubtitle')}</p>
      </div>

      <AcquisitionsTabBar activeTab={activeTab} onSelectTab={selectTab} />

      <div
        role="tabpanel"
        id={`acquisitions-panel-${activeTab}`}
        aria-labelledby={`acquisitions-tab-${activeTab}`}
      >
        {activeTab === 'orders' ? <AcquisitionsOrdersPanel /> : null}
        {activeTab === 'vendors' ? <AcquisitionsVendorsPanel /> : null}
        {activeTab === 'funds' ? <AcquisitionsFundsPanel /> : null}
        {activeTab === 'suggestions' ? <AcquisitionsSuggestionsPanel /> : null}
      </div>
    </div>
  );
}
