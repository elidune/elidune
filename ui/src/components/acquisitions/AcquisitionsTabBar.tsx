import { useMemo } from 'react';
import { Lightbulb, ShoppingCart, Store, Wallet } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { SectionTabBar } from '@/components/common';
import { useLibrary } from '@/contexts/LibraryContext';
import { isFeatureEnabled } from '@/utils/featureAccess';
import type { AcquisitionsTab } from '@/utils/acquisitionsTabs';

const TABS: {
  id: AcquisitionsTab;
  labelKey: string;
  icon: typeof ShoppingCart;
}[] = [
  { id: 'orders', labelKey: 'nav.orders', icon: ShoppingCart },
  { id: 'vendors', labelKey: 'nav.vendors', icon: Store },
  { id: 'funds', labelKey: 'nav.funds', icon: Wallet },
  { id: 'suggestions', labelKey: 'nav.suggestions', icon: Lightbulb },
];

type AcquisitionsTabBarProps = {
  activeTab: AcquisitionsTab;
  onSelectTab: (tab: AcquisitionsTab) => void;
};

export default function AcquisitionsTabBar({ activeTab, onSelectTab }: AcquisitionsTabBarProps) {
  const { t } = useTranslation();
  const { libraryInfo } = useLibrary();
  const showSuggestions = isFeatureEnabled(libraryInfo?.enabledFeatures, 'purchaseSuggestions');

  const tabs = useMemo(
    () =>
      TABS.filter((tab) => tab.id !== 'suggestions' || showSuggestions).map((tab) => ({
        id: tab.id,
        label: t(tab.labelKey),
        icon: tab.icon,
      })),
    [showSuggestions, t],
  );

  return (
    <SectionTabBar
      ariaLabel={t('acquisitions.sectionNav')}
      activeTab={activeTab}
      onSelectTab={onSelectTab}
      tabs={tabs}
    />
  );
}
