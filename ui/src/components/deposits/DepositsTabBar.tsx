import { Building2, Package } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { SectionTabBar } from '@/components/common';
import type { DepositsTab } from '@/utils/depositsTabs';

type DepositsTabBarProps = {
  activeTab: DepositsTab;
  onSelectTab: (tab: DepositsTab) => void;
};

export default function DepositsTabBar({ activeTab, onSelectTab }: DepositsTabBarProps) {
  const { t } = useTranslation();

  return (
    <SectionTabBar
      ariaLabel={t('deposits.sectionNav')}
      activeTab={activeTab}
      onSelectTab={onSelectTab}
      tabs={[
        { id: 'batches', label: t('deposits.batches.title'), icon: Package },
        { id: 'lenders', label: t('deposits.lenders.title'), icon: Building2 },
      ]}
    />
  );
}
