import { Newspaper } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { SectionTabBar } from '@/components/common';

export type SerialsTab = 'subscriptions';

type SerialsTabBarProps = {
  activeTab: SerialsTab;
  onSelectTab: (tab: SerialsTab) => void;
};

export default function SerialsTabBar({ activeTab, onSelectTab }: SerialsTabBarProps) {
  const { t } = useTranslation();
  return (
    <SectionTabBar
      ariaLabel={t('serials.sectionNav')}
      activeTab={activeTab}
      onSelectTab={onSelectTab}
      tabs={[{ id: 'subscriptions', label: t('serials.tab.subscriptions'), icon: Newspaper }]}
    />
  );
}
