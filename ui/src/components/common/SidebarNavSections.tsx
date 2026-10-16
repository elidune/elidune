import { useState, type ComponentType } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronUp } from 'lucide-react';
import { useMatchMedia } from '@/hooks/common/useMatchMedia';
import {
  SIDEBAR_NAV_COMPACT_MEDIA,
  isSidebarNavItemActive,
  navSectionCollapsedVisibleCount,
} from '@/utils/sidebarNav';

export type SidebarNavItem = {
  name: string;
  href: string;
  icon: ComponentType<{ className?: string }>;
};

export type SidebarNavSection = {
  id: string;
  label?: string;
  items: SidebarNavItem[];
};

type SidebarNavSectionsProps = {
  sections: SidebarNavSection[];
  onNavigate?: () => void;
};

export default function SidebarNavSections({ sections, onNavigate }: SidebarNavSectionsProps) {
  const { t } = useTranslation();
  const location = useLocation();
  const compactNav = useMatchMedia(SIDEBAR_NAV_COMPACT_MEDIA);
  const [expandedSectionIds, setExpandedSectionIds] = useState<Set<string>>(() => new Set());

  const toggleSection = (sectionId: string) => {
    setExpandedSectionIds((prev) => {
      const next = new Set(prev);
      if (next.has(sectionId)) next.delete(sectionId);
      else next.add(sectionId);
      return next;
    });
  };

  return (
    <>
      {sections.map((section) => {
        const expanded = !compactNav || expandedSectionIds.has(section.id);
        const collapsedVisible = navSectionCollapsedVisibleCount(section.items.length);
        const visibleItems = expanded
          ? section.items
          : section.items.filter(
              (item, index) =>
                index < collapsedVisible || isSidebarNavItemActive(item.href, location),
            );
        const hiddenCount = expanded ? 0 : section.items.length - visibleItems.length;

        return (
          <div key={section.id} className="space-y-1">
            {section.label && (
              <p className="px-3 pt-1 text-[11px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
                {section.label}
              </p>
            )}
            {visibleItems.map((item) => {
              const isActive = isSidebarNavItemActive(item.href, location);
              return (
                <Link
                  key={item.href}
                  to={item.href}
                  onClick={onNavigate}
                  className={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                    isActive
                      ? 'bg-amber-50 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                      : 'text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800'
                  }`}
                >
                  <item.icon className="h-5 w-5" />
                  {item.name}
                </Link>
              );
            })}
            {compactNav && hiddenCount > 0 && (
              <button
                type="button"
                onClick={() => toggleSection(section.id)}
                aria-expanded={false}
                className="flex w-full items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-800"
              >
                <ChevronDown className="h-4 w-4 shrink-0" aria-hidden="true" />
                {t('nav.showMore', { count: hiddenCount })}
              </button>
            )}
            {compactNav && expanded && section.items.length > collapsedVisible && (
              <button
                type="button"
                onClick={() => toggleSection(section.id)}
                aria-expanded={true}
                className="flex w-full items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium text-gray-500 hover:bg-gray-100 dark:text-gray-400 dark:hover:bg-gray-800"
              >
                <ChevronUp className="h-4 w-4 shrink-0" aria-hidden="true" />
                {t('nav.showLess')}
              </button>
            )}
          </div>
        );
      })}
    </>
  );
}
