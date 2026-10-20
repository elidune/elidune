import { useTranslation } from 'react-i18next';
import { SectionTabBar } from '@/components/common';
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
    <SectionTabBar
      ariaLabel={t('acquisitions.sectionNav')}
      activeTab={activeTab}
      onSelectTab={onSelectTab}
      tabs={TABS.map((tab) => ({ id: tab.id, label: t(tab.labelKey) }))}
    />
  );
}
