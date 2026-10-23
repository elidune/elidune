import { PackageOpen } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Navigate, useSearchParams } from 'react-router-dom';
import { PageHeading, PageTabLeadGroup } from '@/components/common';
import DepositsTabBar from '@/components/deposits/DepositsTabBar';
import { DepositBatchesPanel } from '@/pages/DepositBatchesPage';
import { LendersPanel } from '@/pages/LendersPage';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { type DepositsTab, isDepositsTab } from '@/utils/depositsTabs';

export default function DepositsPage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const tabParam = searchParams.get('tab');
  const activeTab: DepositsTab = isDepositsTab(tabParam) ? tabParam : 'batches';

  const selectTab = (tab: DepositsTab) => {
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
      <PageTabLeadGroup>
        <PageHeading
          icon={PackageOpen}
          title={t('deposits.pageTitle')}
          subtitle={t('deposits.pageSubtitle')}
          subtitleClassName="mt-1 text-sm text-gray-600 dark:text-gray-300"
        />
        <DepositsTabBar activeTab={activeTab} onSelectTab={selectTab} />
      </PageTabLeadGroup>

      <div
        role="tabpanel"
        id={`deposits-panel-${activeTab}`}
        aria-labelledby={`deposits-tab-${activeTab}`}
      >
        {activeTab === 'batches' ? <DepositBatchesPanel /> : null}
        {activeTab === 'lenders' ? <LendersPanel /> : null}
      </div>
    </div>
  );
}

/** Legacy route `/deposits/lenders` → tabbed page. */
export function DepositsLendersRedirect() {
  return <Navigate to="/deposits?tab=lenders" replace />;
}
