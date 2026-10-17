import { useTranslation } from 'react-i18next';
import type { AcquisitionsTab } from '@/utils/acquisitionsTabs';

const TABS: { id: AcquisitionsTab; labelKey: string }[] = [
  { id: 'orders', labelKey: 'nav.orders' },
  { id: 'vendors', labelKey: 'nav.vendors' },
  { id: 'funds', labelKey: 'nav.funds' },
  { id: 'suggestions', labelKey: 'nav.suggestions' },
];

type AcquisitionsTabBarProps = {
  activeTab: AcquisitionsTab;
  onSelectTab: (tab: AcquisitionsTab) => void;
};

export default function AcquisitionsTabBar({ activeTab, onSelectTab }: AcquisitionsTabBarProps) {
  const { t } = useTranslation();

  return (
    <div className="rounded-2xl border border-gray-200/80 dark:border-gray-800/80 bg-gray-50/50 dark:bg-gray-900/30 p-1.5 sm:p-2">
      <nav className="flex flex-wrap gap-1" role="tablist" aria-label={t('acquisitions.sectionNav')}>
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            id={`acquisitions-tab-${tab.id}`}
            aria-selected={activeTab === tab.id}
            aria-controls={`acquisitions-panel-${tab.id}`}
            onClick={() => onSelectTab(tab.id)}
            className={`rounded-xl px-3 py-2.5 text-sm font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-white dark:bg-gray-800 text-amber-700 dark:text-amber-400 shadow-sm border border-gray-200/70 dark:border-gray-700/70'
                : 'text-gray-600 dark:text-gray-400 hover:bg-white/60 dark:hover:bg-gray-800/40 hover:text-gray-900 dark:hover:text-gray-200 border border-transparent'
            }`}
          >
            {t(tab.labelKey)}
          </button>
        ))}
      </nav>
    </div>
  );
}
