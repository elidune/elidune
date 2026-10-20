import { Library } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Navigate, useSearchParams } from 'react-router-dom';
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
      <div className="flex items-start gap-3">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-gray-100 dark:bg-gray-800">
          <Library className="h-6 w-6 text-gray-700 dark:text-gray-200" aria-hidden />
        </div>
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('deposits.pageTitle')}</h1>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">{t('deposits.pageSubtitle')}</p>
        </div>
      </div>

      <DepositsTabBar activeTab={activeTab} onSelectTab={selectTab} />

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
